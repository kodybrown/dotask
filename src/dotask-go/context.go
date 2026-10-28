// Package dotask supplies the execution context and native CLI protocol for Go
// tasks. It has no external modules and never installs tools or compiles tasks.
package dotask

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"runtime"

	"strings"
)

type BuildContext struct {
	data                 map[string]json.RawMessage
	parameters, settings map[string]any
	ctx                  context.Context
	stop                 context.CancelFunc
}

// ProcessFailure preserves the original child exit code, including when wrapped.
type ProcessFailure struct {
	Code  int
	Cause error
}

func (e *ProcessFailure) Error() string {
	return fmt.Sprintf("process failed with exit code %d: %v", e.Code, e.Cause)
}
func (e *ProcessFailure) Unwrap() error { return e.Cause }

// Run is the task entry point. The CLI owns cancellation and descendant cleanup;
// the context also lets cooperative Go tasks observe the interrupt directly.
func Run(task func(*BuildContext) error) {
	code := func() int {
		c, err := Load()
		if err == nil {
			defer c.Close()
			err = task(c)
		}
		if err == nil {
			return 0
		}
		fmt.Fprintln(os.Stderr, "dotask:", err)
		var failure *ProcessFailure
		if errors.As(err, &failure) {
			return failure.Code
		}
		if errors.Is(err, context.Canceled) {
			return 130
		}
		return 1
	}()
	if code != 0 {
		os.Exit(code)
	}
}
func Load() (*BuildContext, error) {
	path := os.Getenv("DOTASK_EXECUTION_CONTEXT")
	if path == "" {
		return nil, errors.New("run this task through dotask")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	c := &BuildContext{}
	if err = json.Unmarshal(data, &c.data); err != nil {
		return nil, err
	}
	for _, key := range []string{"RootDirectory", "TaskDirectory", "InvocationDirectory", "TargetFile", "TargetName", "CliExecutable", "SessionDirectory"} {
		var value string
		if err = json.Unmarshal(c.data[key], &value); err != nil || value == "" {
			return nil, fmt.Errorf("invalid execution context: missing %s", key)
		}
	}
	if err = decode(c.data["Parameters"], &c.parameters); err != nil {
		return nil, err
	}
	if err = decode(c.data["Settings"], &c.settings); err != nil {
		return nil, err
	}
	c.ctx, c.stop = signal.NotifyContext(context.Background(), os.Interrupt)
	return c, nil
}
func decode(raw []byte, value any) error {
	d := json.NewDecoder(bytes.NewReader(raw))
	d.UseNumber()
	return d.Decode(value)
}
func (c *BuildContext) Close() {
	if c.stop != nil {
		c.stop()
	}
}
func (c *BuildContext) Context() context.Context { return c.ctx }
func (c *BuildContext) text(key string) string {
	var value string
	_ = json.Unmarshal(c.data[key], &value)
	return value
}
func (c *BuildContext) CLIExecutable() string { return c.text("CliExecutable") }
func (c *BuildContext) Root() string          { return c.text("RootDirectory") }
func (c *BuildContext) TaskDirectory() string { return c.text("TaskDirectory") }
func (c *BuildContext) Invocation() string    { return c.text("InvocationDirectory") }
func (c *BuildContext) TaskFile() string      { return c.text("TargetFile") }
func (c *BuildContext) TaskName() string      { return c.text("TargetName") }
func (c *BuildContext) OS() string {
	if runtime.GOOS == "darwin" {
		return "macos"
	}
	return runtime.GOOS
}
func (c *BuildContext) Architecture() string {
	switch runtime.GOARCH {
	case "amd64":
		return "x64"
	case "386":
		return "x86"
	default:
		return runtime.GOARCH
	}
}

// Path resolves explicitly portable paths; ordinary process arguments are never
// rewritten. A Windows rooted path on Unix is rejected instead of rebased.
func (c *BuildContext) Path(value string) (string, error) {
	if strings.ContainsRune(value, 0) {
		return "", errors.New("a path cannot contain NUL")
	}
	if runtime.GOOS != "windows" && (strings.HasPrefix(value, `\\`) || len(value) >= 2 && value[1] == ':' && ((value[0] >= 'a' && value[0] <= 'z') || (value[0] >= 'A' && value[0] <= 'Z'))) {
		return "", fmt.Errorf("Windows rooted path cannot be resolved on this host: %s", value)
	}
	value = strings.ReplaceAll(strings.ReplaceAll(value, `\`, string(filepath.Separator)), "/", string(filepath.Separator))
	if !filepath.IsAbs(value) {
		value = filepath.Join(c.Root(), value)
	}
	return filepath.Abs(value)
}
func lookup(values map[string]any, name string) (any, error) {
	var current any = values
	for _, part := range strings.Split(name, ".") {
		mapping, ok := current.(map[string]any)
		if !ok {
			return nil, fmt.Errorf("missing value %q", name)
		}
		found := false
		for key, value := range mapping {
			if strings.EqualFold(key, part) {
				current = value
				found = true
				break
			}
		}
		if !found {
			return nil, fmt.Errorf("missing value %q", name)
		}
	}
	return current, nil
}
func (c *BuildContext) Parameter(name string) (any, error) { return lookup(c.parameters, name) }
func (c *BuildContext) Setting(name string) (any, error)   { return lookup(c.settings, name) }
func as[T any](get func(string) (any, error), name string) (T, error) {
	var result T
	value, err := get(name)
	if err != nil {
		return result, err
	}
	raw, err := json.Marshal(value)
	if err == nil {
		err = json.Unmarshal(raw, &result)
	}
	if err != nil {
		return result, fmt.Errorf("invalid type for %q: %w", name, err)
	}
	// JSON null must not silently become a zero-valued typed parameter/setting.
	if value == nil {
		return result, fmt.Errorf("value %q is null", name)
	}
	return result, nil
}
func ParameterAs[T any](c *BuildContext, name string) (T, error) { return as[T](c.Parameter, name) }
func SettingAs[T any](c *BuildContext, name string) (T, error)   { return as[T](c.Setting, name) }
func (c *BuildContext) String(name string) (string, error)       { return ParameterAs[string](c, name) }
func (c *BuildContext) Bool(name string) (bool, error)           { return ParameterAs[bool](c, name) }
func (c *BuildContext) Int(name string) (int64, error)           { return ParameterAs[int64](c, name) }
func (c *BuildContext) Number(name string) (float64, error)      { return ParameterAs[float64](c, name) }
func (c *BuildContext) SettingPath(name string) (string, error) {
	value, err := SettingAs[string](c, name)
	if err != nil {
		return "", err
	}
	if strings.TrimSpace(value) == "" {
		return "", fmt.Errorf("setting %q must name a path", name)
	}
	return c.Path(value)
}

// Typed integers never pass through float64, retaining exact values above 2^53.

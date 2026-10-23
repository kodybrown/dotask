package dotask

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

type OptionalResult struct {
	Exists   bool
	ExitCode *int
	Error    *string
}

// writeNew keeps private invocation files exclusive, matching the native SDK.
func writeNew(path string, value any) error {
	raw, err := json.Marshal(value)
	if err != nil {
		return err
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if err != nil {
		return err
	}
	_, err = file.Write(raw)
	closeErr := file.Close()
	if err != nil {
		return err
	}
	return closeErr
}
func (c *BuildContext) call(target string, parameters any, operation int, result any) error {
	if strings.TrimSpace(target) == "" {
		return fmt.Errorf("a task name is required")
	}
	if parameters == nil {
		parameters = map[string]any{}
	}
	raw, err := json.Marshal(parameters)
	if err != nil {
		return err
	}
	var mapping map[string]json.RawMessage
	if err = json.Unmarshal(raw, &mapping); err != nil || mapping == nil {
		return fmt.Errorf("task parameters must be an object")
	}
	directory, err := os.MkdirTemp(c.text("SessionDirectory"), "go-call-")
	if err != nil {
		return err
	}
	defer os.RemoveAll(directory)
	context := make(map[string]json.RawMessage, len(c.data))
	for key, value := range c.data {
		context[key] = value
	}
	context["SessionDirectory"], _ = json.Marshal(directory)
	request := filepath.Join(directory, "request.json")
	if err = writeNew(request, map[string]any{"Context": context, "Target": target, "Parameters": mapping, "Operation": operation}); err != nil {
		return err
	}
	var prefix []string
	if err = json.Unmarshal(c.data["CliArguments"], &prefix); err != nil {
		return err
	}
	args := append(prefix, "__exec", request)
	if err = c.Execute(c.Command(c.text("CliExecutable"), args...)); err != nil {
		return err
	}
	if operation == 0 {
		return nil
	}
	reply := request + ".result"
	if operation == 3 {
		reply = filepath.Join(directory, "installer-result.json")
	}
	data, err := os.ReadFile(reply)
	if err != nil {
		return fmt.Errorf("task %q returned no result: %w", target, err)
	}
	return json.Unmarshal(data, result)
}
func (c *BuildContext) ExecTarget(target string, parameters any) error {
	return c.call(target, parameters, 0, nil)
}
func (c *BuildContext) TargetExists(target string) (bool, error) {
	var reply OptionalResult
	err := c.call(target, nil, 1, &reply)
	return reply.Exists, err
}

// Only absence has Exists=false. Ambiguity and task failure remain explicit.
func (c *BuildContext) ExecIfExists(target string, parameters any) (OptionalResult, error) {
	var reply OptionalResult
	err := c.call(target, parameters, 2, &reply)
	return reply, err
}
func (c *BuildContext) CreateInstaller(target string, parameters any) (*InstallerArtifact, error) {
	var artifact InstallerArtifact
	if err := c.call(target, parameters, 3, &artifact); err != nil {
		return nil, err
	}
	if err := c.ValidateInstaller(&artifact); err != nil {
		return nil, err
	}
	return &artifact, nil
}

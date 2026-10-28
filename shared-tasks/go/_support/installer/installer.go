// Package installer implements shared native Go packaging. Project-owned YAML
// selects the package, version source, smoke check and explicit payload inventory.
package installer

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"

	dotask "dotask.local/sdk"
)

type settings struct {
	InstallerCommand         string   `json:"installer-command"`
	InstallerForbiddenFiles  []string `json:"installer-forbidden-files"`
	Package                  string   `json:"package"`
	Executable               string   `json:"executable"`
	VersionFile              string   `json:"version-file"`
	VersionSymbol            string   `json:"version-symbol"`
	VersionOutput            string   `json:"version-output"`
	VersionArguments         []string `json:"version-arguments"`
	InstallerFiles           []string `json:"installer-files"`
	CleanEnvironmentPrefixes []string `json:"clean-environment-prefixes"`
}

func config(c *dotask.BuildContext) (settings, error) {
	s, err := dotask.SettingAs[settings](c, "go")
	if err != nil {
		return s, err
	}
	for _, item := range []struct{ name, value string }{
		{"package", s.Package}, {"executable", s.Executable}, {"version-file", s.VersionFile},
		{"version-symbol", s.VersionSymbol}, {"version-output", s.VersionOutput},
	} {
		if strings.TrimSpace(item.value) == "" {
			return s, fmt.Errorf("settings.go.%s is required", item.name)
		}
	}
	if err = relativeFile(s.Executable); err != nil || strings.ContainsAny(s.Executable, "/\\") {
		return s, errors.New("settings.go.executable must be a plain filename")
	}
	if (runtime.GOOS == "windows") != strings.EqualFold(filepath.Ext(s.Executable), ".exe") {
		return s, errors.New("settings.go.executable must match the native platform (.exe on Windows)")
	}
	if c.Architecture() != "x64" && c.Architecture() != "arm64" {
		return s, errors.New("native installer tasks support x64 and arm64")
	}
	if !strings.Contains(s.VersionOutput, "{version}") {
		return s, errors.New("settings.go.version-output must contain {version}")
	}
	if s.VersionArguments == nil {
		s.VersionArguments = []string{"--version"}
	}
	for _, prefix := range s.CleanEnvironmentPrefixes {
		if strings.TrimSpace(prefix) == "" || strings.ContainsAny(prefix, "=\x00") {
			return s, errors.New("clean-environment-prefixes must contain nonempty environment name prefixes")
		}
	}
	return s, nil
}

// Read only a single top-level literal. Never execute project source or rewrite
// generated version files, and reject duplicate/computed declarations explicitly.
func version(c *dotask.BuildContext, s settings) (string, error) {
	path, err := c.Path(s.VersionFile)
	if err != nil {
		return "", err
	}
	source, err := parser.ParseFile(token.NewFileSet(), path, nil, 0)
	if err != nil {
		return "", err
	}
	var values []ast.Expr
	for _, decl := range source.Decls {
		group, ok := decl.(*ast.GenDecl)
		if !ok || group.Tok != token.CONST && group.Tok != token.VAR {
			continue
		}
		for _, spec := range group.Specs {
			value := spec.(*ast.ValueSpec)
			for i, name := range value.Names {
				if name.Name == s.VersionSymbol {
					if len(value.Values) != len(value.Names) {
						return "", fmt.Errorf("%s must have its own string literal", s.VersionSymbol)
					}
					values = append(values, value.Values[i])
				}
			}
		}
	}
	if len(values) != 1 {
		return "", fmt.Errorf("%s must define exactly one top-level %s string literal", s.VersionFile, s.VersionSymbol)
	}
	literal, ok := values[0].(*ast.BasicLit)
	if !ok || literal.Kind != token.STRING {
		return "", fmt.Errorf("%s must be a string literal", s.VersionSymbol)
	}
	value, err := strconv.Unquote(literal.Value)
	if err != nil {
		return "", err
	}
	value = strings.TrimPrefix(value, "v")
	if strings.TrimSpace(value) == "" {
		return "", errors.New("application version cannot be empty")
	}
	return value, nil
}

func Create(c *dotask.BuildContext) error {
	s, err := config(c)
	if err != nil {
		return err
	}
	value, err := version(c, s)
	if err != nil {
		return err
	}
	if err = checkHelper(c); err != nil {
		return err
	}
	temporary, err := filepath.EvalSymlinks(os.TempDir())
	if err != nil {
		return err
	}
	preparation, err := os.MkdirTemp(temporary, "dotask-go-installer-*")
	if err != nil {
		return err
	}
	completed := false
	defer func() {
		if !completed {
			fmt.Fprintln(os.Stderr, "Installer preparation retained:", preparation)
			return
		}
		// Only delete the private immediate child created above; resolve its final
		// identity again rather than trusting a caller-supplied or mutable path.
		resolved, err := filepath.EvalSymlinks(preparation)
		if err == nil && filepath.Dir(resolved) == temporary && strings.HasPrefix(filepath.Base(resolved), "dotask-go-installer-") {
			if err = os.RemoveAll(resolved); err != nil {
				fmt.Fprintln(os.Stderr, "Installer preparation cleanup:", err)
			}
		}
	}()
	payload := filepath.Join(preparation, "app")
	if err = preparePayload(c, s, payload, value); err != nil {
		return err
	}
	parameters := map[string]any{"app-version": value, "input-payload": payload}
	for _, option := range []string{"output", "builder", "installer", "launcher-template"} {
		raw, exists := c.Parameter(option)
		if exists != nil {
			continue
		}
		text, ok := raw.(string)
		if !ok || strings.TrimSpace(text) == "" {
			return fmt.Errorf("%s must be a nonempty string", option)
		}
		if option != "builder" {
			text, err = c.Path(text)
			if err != nil {
				return err
			}
		}
		parameters[option] = text
	}
	// The native helper owns result transport and records only fully successful
	// packaging. Returning the same artifact preserves nested caller composition.
	artifact, err := c.CreateInstaller("_/doinstall/create-installer.task", parameters)
	if err != nil {
		return err
	}
	if err = c.SetInstallerResult(artifact); err != nil {
		return err
	}
	completed = true
	fmt.Println("Installer created:", artifact.FilePath)
	return nil
}

func relativeFile(name string) error {
	if name == "" || strings.ContainsAny(name, "\\:\x00") || filepath.IsAbs(name) {
		return fmt.Errorf("payload file must be a portable relative path: %q", name)
	}
	for _, part := range strings.Split(name, "/") {
		if part == "" || part == "." || part == ".." {
			return fmt.Errorf("payload file must stay beneath its root: %q", name)
		}
	}
	return nil
}

func noLinks(root, name string) (string, error) {
	if err := relativeFile(name); err != nil {
		return "", err
	}
	path := root
	for _, part := range strings.Split(name, "/") {
		path = filepath.Join(path, part)
		info, err := os.Lstat(path)
		if err != nil {
			return "", err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return "", fmt.Errorf("payload source cannot be a link: %s", path)
		}
	}
	info, err := os.Stat(path)
	if err != nil {
		return "", err
	}
	if !info.Mode().IsRegular() {
		return "", fmt.Errorf("payload source must be a regular file: %s", path)
	}
	return path, nil
}

// Clear inherited Go controls before selecting the installed native compiler.
// GOTOOLCHAIN=local forbids toolchain downloads even if go.mod asks for newer Go.
// Cache/module downloads stay outside source; ordinary app dependencies may use
// the existing Go module cache/proxy (the SDK itself has no external modules).
func goCommand(c *dotask.BuildContext, args ...string) *exec.Cmd {
	command := c.Command("go", args...)
	for _, entry := range os.Environ() {
		name, _, _ := strings.Cut(entry, "=")
		if !strings.HasPrefix(strings.ToUpper(name), "GO") && !strings.EqualFold(name, "CGO_ENABLED") {
			command.Env = append(command.Env, entry)
		}
	}
	command.Env = append(command.Env, "GOENV=off", "GOTOOLCHAIN=local", "GOWORK=off", "GO111MODULE=on", "CGO_ENABLED=0", "GOOS="+runtime.GOOS, "GOARCH="+runtime.GOARCH)
	return command
}

func preparePayload(c *dotask.BuildContext, s settings, stage, value string) error {
	var err error
	// Resolve the existing parent so a source-local symlink cannot redirect an
	// apparently external staging directory back into the consumer checkout.
	parent, err := filepath.EvalSymlinks(filepath.Dir(stage))
	if err != nil {
		return err
	}
	stage = filepath.Join(parent, filepath.Base(stage))
	root, err := filepath.EvalSymlinks(c.Root())
	if err != nil {
		return err
	}
	relative, err := filepath.Rel(root, stage)
	if err == nil && relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		return errors.New("installer staging directory must be outside the project source tree")
	}
	if _, err = os.Lstat(stage); !os.IsNotExist(err) {
		return fmt.Errorf("installer staging directory must be fresh: %s", stage)
	}
	if strings.TrimSpace(value) == "" {
		return errors.New("app-version cannot be empty")
	}
	// Validate the entire inventory before building or copying. Case-insensitive
	// collisions are refused so the same recipe is portable across native hosts.
	seen := map[string]bool{strings.ToLower(s.Executable): true}
	sources := make([]string, len(s.InstallerFiles))
	for i, name := range s.InstallerFiles {
		if seen[strings.ToLower(name)] {
			return fmt.Errorf("duplicate payload destination: %s", name)
		}
		seen[strings.ToLower(name)] = true
		sources[i], err = noLinks(root, name)
		if err != nil {
			return err
		}
	}
	for _, operation := range []string{"test", "vet"} {
		if err = c.Execute(goCommand(c, operation, "-mod=readonly", "./...")); err != nil {
			return err
		}
	}
	if err = os.Mkdir(stage, 0700); err != nil {
		return err
	}
	executable := filepath.Join(stage, s.Executable)
	if err = c.Execute(goCommand(c, "build", "-mod=readonly", "-trimpath", "-buildvcs=false", "-o", executable, s.Package)); err != nil {
		return err
	}
	data, err := os.ReadFile(executable)
	if err != nil {
		return err
	}
	for _, path := range []string{c.Root(), os.Getenv("USERPROFILE"), os.Getenv("HOME")} {
		for _, spelling := range []string{strings.ReplaceAll(path, "\\", "/"), strings.ReplaceAll(path, "/", "\\")} {
			if spelling != "" && bytes.Contains(data, []byte(spelling)) {
				return errors.New("compiled executable contains a local checkout/profile path")
			}
		}
	}
	command := c.Command(executable, s.VersionArguments...)
	command.Dir = stage
	for _, entry := range os.Environ() {
		name, _, _ := strings.Cut(entry, "=")
		keep := true
		for _, prefix := range s.CleanEnvironmentPrefixes {
			if strings.HasPrefix(strings.ToUpper(name), strings.ToUpper(prefix)) {
				keep = false
			}
		}
		if keep {
			command.Env = append(command.Env, entry)
		}
	}
	actual, err := c.Capture(command)
	if err != nil {
		return err
	}
	expected := strings.ReplaceAll(s.VersionOutput, "{version}", value)
	if strings.TrimSpace(actual) != expected {
		return fmt.Errorf("built executable version does not match installer metadata: expected %q, got %q", expected, strings.TrimSpace(actual))
	}
	for i, name := range s.InstallerFiles {
		destination := filepath.Join(stage, filepath.FromSlash(name))
		if err = os.MkdirAll(filepath.Dir(destination), 0700); err != nil {
			return err
		}
		if err = copyFile(sources[i], destination); err != nil {
			return err
		}
	}
	fmt.Println("Verified installer payload:", expected)
	return nil
}

func copyFile(source, destination string) error {
	input, err := os.Open(source)
	if err != nil {
		return err
	}
	defer input.Close()
	output, err := os.OpenFile(destination, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0644)
	if err != nil {
		return err
	}
	_, err = io.Copy(output, input)
	closeErr := output.Close()
	if err != nil {
		return err
	}
	return closeErr
}

func checkHelper(c *dotask.BuildContext) error {
	executable, ok := any(c).(interface{ CLIExecutable() string })
	if !ok {
		return errors.New("Go installer creation requires an updated bundled Go SDK and matching precompiled helper with input-payload support; no application build was started")
	}
	helper := filepath.Join(filepath.Dir(executable.CLIExecutable()), "dotask-doinstall-create-installer")
	if runtime.GOOS == "windows" {
		helper += ".exe"
	}
	command := c.Command(helper, "--capabilities")
	// Older helpers ignore arguments. Remove task context so probing one can
	// never accidentally execute the project's installer recipe.
	command.Env = []string{}
	for _, entry := range os.Environ() {
		name, _, _ := strings.Cut(entry, "=")
		if !strings.EqualFold(name, "DOTASK_EXECUTION_CONTEXT") {
			command.Env = append(command.Env, entry)
		}
	}
	output, err := c.Capture(command)
	var capability struct{ Schema, InputPayload int }
	if err != nil || json.Unmarshal([]byte(output), &capability) != nil || capability.Schema != 1 || capability.InputPayload != 1 {
		return errors.New("Go installer creation requires matching updated precompiled dotask/helper and bundled Go SDK with input-payload support; no application build was started")
	}
	return nil
}

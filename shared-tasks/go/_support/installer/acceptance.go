package installer

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"time"
	"unicode/utf8"

	dotask "dotask.local/sdk"
)

type registryValue struct {
	present bool
	kind    uint32
	data    []byte
}

// Test exercises a previously assembled installer, never the active installation.
// Failed fixtures and process logs survive; cleanup is limited to a verified
// immediate child of the canonical temporary directory created by this call.
func Test(c *dotask.BuildContext) error {
	if c.OS() != "windows" {
		return errors.New("installer acceptance currently requires native Windows; persistent PATH verification is unavailable on this host")
	}
	s, err := config(c)
	if err != nil {
		return err
	}
	path, err := acceptanceInstaller(c)
	if err != nil {
		return err
	}
	path, err = filepath.EvalSymlinks(path)
	if err != nil {
		return err
	}
	info, err := os.Stat(path)
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() {
		return errors.New("installer must be an existing regular file")
	}
	fmt.Println("Testing installer:", path)
	timeout, err := c.Int("timeout-seconds")
	if err != nil {
		return err
	}
	if timeout < 1 || timeout > 86400 {
		return errors.New("timeout-seconds must be between 1 and 86400")
	}
	expected, err := c.String("app-version")
	if err != nil {
		expected, err = version(c, s)
	}
	if err != nil {
		return err
	}
	if strings.TrimSpace(expected) == "" {
		return errors.New("app-version cannot be empty")
	}
	before, err := persistentPaths()
	if err != nil {
		return err
	}
	temporary, err := filepath.EvalSymlinks(os.TempDir())
	if err != nil {
		return err
	}
	root, err := os.MkdirTemp(temporary, "dotask-installer-test-* space 日本語")
	if err != nil {
		return err
	}
	cleaned := false
	defer func() {
		if !cleaned {
			fmt.Fprintln(os.Stderr, "Failed acceptance fixture retained:", root)
		}
	}()
	runner := acceptance{c: c, settings: s, root: root, timeout: time.Duration(timeout) * time.Second, expected: strings.ReplaceAll(s.VersionOutput, "{version}", expected)}
	outcome := runner.check(path)
	after, pathErr := persistentPaths()
	if pathErr == nil && !samePaths(before, after) {
		pathErr = errors.New("installer acceptance changed persistent user/machine PATH")
	}
	if outcome != nil || pathErr != nil {
		return errors.Join(outcome, pathErr)
	}
	resolved, err := filepath.EvalSymlinks(root)
	if err != nil {
		return err
	}
	name := filepath.Base(resolved)
	if filepath.Dir(resolved) != temporary || !strings.HasPrefix(name, "dotask-installer-test-") || !strings.HasSuffix(name, " space 日本語") {
		return fmt.Errorf("unexpected cleanup target: %s", resolved)
	}
	if err = cleanupFixture(c.Context(), resolved); err != nil {
		return err
	}
	cleaned = true
	fmt.Println("PASS: user/machine PATH unchanged; temporary acceptance files cleaned up.")
	return nil
}

func samePaths(a, b []registryValue) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i].present != b[i].present || a[i].kind != b[i].kind || !bytes.Equal(a[i].data, b[i].data) {
			return false
		}
	}
	return true
}

type acceptance struct {
	c          *dotask.BuildContext
	settings   settings
	root       string
	timeout    time.Duration
	expected   string
	invocation int
}

func (a *acceptance) run(executable string, args ...string) (string, error) {
	a.invocation++
	stdout := filepath.Join(a.root, fmt.Sprintf("process-%d.stdout", a.invocation))
	stderr := filepath.Join(a.root, fmt.Sprintf("process-%d.stderr", a.invocation))
	output, err := os.Create(stdout)
	if err != nil {
		return "", err
	}
	defer output.Close()
	diagnostic, err := os.Create(stderr)
	if err != nil {
		return "", err
	}
	defer diagnostic.Close()
	ctx, cancel := context.WithTimeout(a.c.Context(), a.timeout)
	defer cancel()
	command := exec.CommandContext(ctx, executable, args...)
	command.Dir = a.root
	// File-backed logs cannot deadlock on full pipes and stay with failed fixtures.
	// The native dotask runner owns descendant cleanup on timeout/task cancellation.
	command.Stdout = output
	command.Stderr = diagnostic
	for _, entry := range os.Environ() {
		name, _, _ := strings.Cut(entry, "=")
		keep := !strings.EqualFold(name, "APPDATA") && !strings.EqualFold(name, "LOCALAPPDATA")
		for _, prefix := range a.settings.CleanEnvironmentPrefixes {
			if strings.HasPrefix(strings.ToUpper(name), strings.ToUpper(prefix)) {
				keep = false
			}
		}
		if keep {
			command.Env = append(command.Env, entry)
		}
	}
	command.Env = append(command.Env, "APPDATA="+filepath.Join(a.root, "roaming"), "LOCALAPPDATA="+filepath.Join(a.root, "local"))
	err = runContained(command)
	if ctx.Err() != nil {
		return "", fmt.Errorf("process %s stopped (logs under %s): %w", executable, a.root, ctx.Err())
	}
	if err != nil {
		var exit *exec.ExitError
		if errors.As(err, &exit) {
			code := exit.ExitCode()
			if code < 0 {
				code = 130
			}
			err = &dotask.ProcessFailure{Code: code, Cause: err}
		}
		return "", fmt.Errorf("process %s failed; logs under %s: %w", executable, a.root, err)
	}
	data, err := os.ReadFile(stdout)
	if err != nil {
		return "", err
	}
	if !utf8.Valid(data) {
		return "", fmt.Errorf("process stdout is not UTF-8; see %s", stdout)
	}
	return string(data), nil
}

func (a *acceptance) install(installer, extraction string, validate bool) error {
	args := []string{"--extract-to", filepath.Join(a.root, extraction), "--non-interactive", "--install-dir", filepath.Join(a.root, "application"), "--set", "add-to-path=false", "--set", "additional-command=false"}
	if validate {
		args = append(args, "--validate")
	}
	_, err := a.run(installer, args...)
	return err
}

func (a *acceptance) check(installer string) error {
	application := filepath.Join(a.root, "application")
	if err := a.install(installer, "validation extraction", true); err != nil {
		return err
	}
	if _, err := os.Lstat(application); !os.IsNotExist(err) {
		return errors.New("validation created an installation or its absence could not be verified")
	}
	if err := a.install(installer, "initial extraction", false); err != nil {
		return err
	}
	commandName := a.settings.InstallerCommand
	if commandName == "" {
		commandName = a.settings.Executable
	}
	if err := relativeFile(commandName); err != nil || strings.ContainsAny(commandName, "/\\") {
		return errors.New("settings.go.installer-command must be a plain filename")
	}
	executable := filepath.Join(application, commandName)
	actual, err := a.run(executable, a.settings.VersionArguments...)
	if err != nil {
		return err
	}
	if strings.TrimSpace(actual) != a.expected {
		return fmt.Errorf("unexpected installed version: expected %q, got %q", a.expected, strings.TrimSpace(actual))
	}
	builds, err := os.ReadDir(filepath.Join(application, "app"))
	if err != nil {
		return err
	}
	if len(builds) != 1 || !builds[0].IsDir() || builds[0].Type()&os.ModeSymlink != 0 {
		return errors.New("expected exactly one installed application build")
	}
	build := filepath.Join(application, "app", builds[0].Name())
	expected := append([]string{a.settings.Executable}, a.settings.InstallerFiles...)
	for _, name := range expected {
		if err = relativeFile(name); err != nil {
			return err
		}
	}
	var payload []string
	err = filepath.WalkDir(application, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.Type()&os.ModeSymlink != 0 {
			return fmt.Errorf("unexpected link in acceptance installation: %s", path)
		}
		if entry.IsDir() {
			return nil
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if !info.Mode().IsRegular() {
			return fmt.Errorf("unexpected non-file in installation: %s", path)
		}
		for _, forbidden := range a.settings.InstallerForbiddenFiles {
			if strings.EqualFold(entry.Name(), forbidden) {
				return fmt.Errorf("installer copied forbidden file: %s", forbidden)
			}
		}
		relative, err := filepath.Rel(build, path)
		if err != nil {
			return err
		}
		if relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
			return nil
		}
		relative = filepath.ToSlash(relative)
		if relative != "installation.yaml" && relative != "installer.yaml" {
			payload = append(payload, relative)
		}
		return nil
	})
	if err != nil {
		return err
	}
	sort.Strings(payload)
	sort.Strings(expected)
	if !equalStrings(payload, expected) {
		return fmt.Errorf("unexpected installed payload: got %q; expected %q", payload, expected)
	}
	if err = a.install(installer, "reinstall extraction", false); err != nil {
		return err
	}
	actual, err = a.run(executable, a.settings.VersionArguments...)
	if err != nil {
		return err
	}
	if strings.TrimSpace(actual) != a.expected {
		return errors.New("reinstall changed the application version")
	}
	if _, err = a.run(filepath.Join(application, "installer", "installer.exe"), "uninstall", "--non-interactive", "--leave-settings"); err != nil {
		return err
	}
	if _, err = os.Lstat(executable); !os.IsNotExist(err) {
		return errors.New("uninstall left the application command behind or its absence could not be verified")
	}
	fmt.Printf("PASS: validation, install, exact payload, installed version (%s), reinstall, and uninstall.\n", a.expected)
	return nil
}

func equalStrings(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func cleanupFixture(ctx context.Context, root string) error {
	// A just-exited Windows launcher/uninstaller may retain its executable mapping
	// briefly. Retry sharing violations without forcing permissions or deleting
	// anything beyond the already verified temporary fixture root.
	deadline := time.Now().Add(3 * time.Second)
	for {
		err := os.RemoveAll(root)
		if err == nil {
			return nil
		}
		if !retryableCleanup(err) || time.Now().After(deadline) {
			return err
		}
		select {
		case <-ctx.Done():
			return errors.Join(err, ctx.Err())
		case <-time.After(50 * time.Millisecond):
		}
	}
}

func acceptanceInstaller(c *dotask.BuildContext) (string, error) {
	if _, err := c.Parameter("installer"); err == nil {
		raw, err := c.String("installer")
		if err != nil {
			return "", err
		}
		if strings.TrimSpace(raw) == "" {
			return "", errors.New("installer override must be a nonempty path")
		}
		return c.Path(raw)
	}
	// Reuse native discovery so successful-build hashes, platform checks and
	// invalid-record refusal stay identical to the installation workflow. Never
	// guess filenames or rebuild an installer merely to test the last success.
	artifact, err := c.FindInstaller()
	if err != nil {
		return "", err
	}
	if artifact == nil {
		return "", errors.New("no successful installer is recorded for this project/platform; create an installer first or pass --installer")
	}
	return artifact.FilePath, nil
}

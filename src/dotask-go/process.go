package dotask

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"unicode/utf8"
)

// Command uses exact argument tokens and the project root. Callers may set Dir,
// Env and streams explicitly. Descendants stay in the native CLI's owned group.
func (c *BuildContext) Command(executable string, args ...string) *exec.Cmd {
	command := exec.CommandContext(c.ctx, executable, args...)
	command.Dir = c.Root()
	return command
}
func failure(err error) error {
	if err == nil {
		return nil
	}
	var exit *exec.ExitError
	if errors.As(err, &exit) {
		code := exit.ExitCode()
		if code < 0 {
			code = 130
		}
		return &ProcessFailure{Code: code, Cause: err}
	}
	return err
}
func (c *BuildContext) Execute(command *exec.Cmd) error {
	if command.Stdin == nil {
		command.Stdin = os.Stdin
	}
	if command.Stdout == nil {
		command.Stdout = os.Stdout
	}
	if command.Stderr == nil {
		command.Stderr = os.Stderr
	}
	return failure(command.Run())
}

// Capture reads UTF-8 stdout, forwards stderr and preserves failures. It never
// parses a shell command line or changes caller-supplied argument boundaries.
func (c *BuildContext) Capture(command *exec.Cmd) (string, error) {
	var stdout, stderr bytes.Buffer
	command.Stdout = &stdout
	command.Stderr = &stderr
	err := failure(command.Run())
	if !utf8.Valid(stdout.Bytes()) || !utf8.Valid(stderr.Bytes()) {
		if err != nil {
			return "", fmt.Errorf("process output is not UTF-8: %w", err)
		}
		return "", errors.New("process output is not UTF-8")
	}
	if err != nil {
		_, _ = os.Stderr.Write(stdout.Bytes())
	}
	_, _ = os.Stderr.Write(stderr.Bytes())
	return stdout.String(), err
}

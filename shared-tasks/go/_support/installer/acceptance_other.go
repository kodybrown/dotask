//go:build !windows

package installer

import (
	"errors"
	"os/exec"
)

func runContained(command *exec.Cmd) error { return command.Run() }
func persistentPaths() ([]registryValue, error) {
	return nil, errors.New("persistent PATH verification requires Windows")
}

func retryableCleanup(err error) bool { return false }

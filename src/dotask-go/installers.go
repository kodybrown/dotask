package dotask

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// InstallerArtifact uses the same numeric kind/OS/architecture wire contract as
// Rust and C#. Returning an artifact and recording a successful build are separate.
type InstallerArtifact struct {
	FilePath         string
	Kind             int
	OS               int
	Architecture     int
	DefaultArguments []string
}

// UnmarshalJSON rejects omitted/null discriminators instead of allowing Go zero
// values to accidentally identify an executable installer on Windows.
func (a *InstallerArtifact) UnmarshalJSON(raw []byte) error {
	type wire InstallerArtifact
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(raw, &fields); err != nil {
		return err
	}
	for _, key := range []string{"FilePath", "Kind", "OS", "Architecture"} {
		value, ok := fields[key]
		if !ok || string(value) == "null" {
			return fmt.Errorf("missing installer %s", key)
		}
	}
	var value wire
	if err := json.Unmarshal(raw, &value); err != nil {
		return err
	}
	*a = InstallerArtifact(value)
	return nil
}
func (c *BuildContext) host() (int, int) {
	osID, arch := 3, -1
	switch c.OS() {
	case "windows":
		osID = 0
	case "linux":
		osID = 1
	case "macos":
		osID = 2
	}
	switch c.Architecture() {
	case "x64":
		arch = 1
	case "arm64":
		arch = 3
	}
	return osID, arch
}
func (c *BuildContext) InstallerArtifact(file string) (*InstallerArtifact, error) {
	osID, arch := c.host()
	artifact := &InstallerArtifact{FilePath: file, OS: osID, Architecture: arch, DefaultArguments: []string{}}
	return artifact, c.ValidateInstaller(artifact)
}
func (c *BuildContext) ValidateInstaller(artifact *InstallerArtifact) error {
	if artifact == nil {
		return errors.New("missing installer artifact")
	}
	osID, arch := c.host()
	info, err := os.Stat(artifact.FilePath)
	if err != nil || !filepath.IsAbs(artifact.FilePath) || !info.Mode().IsRegular() || artifact.OS != osID || artifact.Architecture != arch || artifact.Kind < 0 || artifact.Kind > 3 {
		return errors.New("installer must be an existing absolute file for the current OS and architecture")
	}
	extension := filepath.Ext(artifact.FilePath)
	if osID == 0 && artifact.Kind == 0 && !strings.EqualFold(extension, ".exe") {
		return errors.New("Windows executable installers must be .exe files")
	}
	if artifact.Kind == 1 && (osID != 0 || !strings.EqualFold(extension, ".msi")) || artifact.Kind == 2 && osID == 0 {
		return errors.New("installer kind is incompatible with this platform")
	}
	for _, arg := range artifact.DefaultArguments {
		if strings.ContainsRune(arg, 0) {
			return errors.New("installer arguments cannot contain NUL")
		}
	}
	return nil
}
func (c *BuildContext) SetInstallerResult(artifact *InstallerArtifact) error {
	if err := c.ValidateInstaller(artifact); err != nil {
		return err
	}
	return writeNew(filepath.Join(c.text("SessionDirectory"), "installer-result.json"), artifact)
}

// Native operations reuse the existing Rust implementation, preserving record
// locks, atomic replacement, unknown-state refusal, hashes and Windows activation
// behavior without another language runtime or a second implementation of policy.
func (c *BuildContext) installerOperation(operation string, artifact *InstallerArtifact, arguments []string) (*InstallerArtifact, error) {
	directory, err := os.MkdirTemp(c.text("SessionDirectory"), "go-installer-")
	if err != nil {
		return nil, err
	}
	defer os.RemoveAll(directory)
	request := filepath.Join(directory, "request.json")
	if err = writeNew(request, map[string]any{"Operation": operation, "Artifact": artifact, "Arguments": arguments}); err != nil {
		return nil, err
	}
	if err = c.Execute(c.Command(c.text("CliExecutable"), "__installer", request)); err != nil {
		return nil, err
	}
	if operation != "find" {
		return nil, nil
	}
	data, err := os.ReadFile(filepath.Join(directory, "request.result"))
	if err != nil {
		return nil, err
	}
	var result *InstallerArtifact
	if err = json.Unmarshal(data, &result); err != nil {
		return nil, fmt.Errorf("invalid installer result: %w", err)
	}
	return result, nil
}
func (c *BuildContext) RecordInstaller(artifact *InstallerArtifact) error {
	if err := c.ValidateInstaller(artifact); err != nil {
		return err
	}
	_, err := c.installerOperation("record", artifact, nil)
	return err
}
func (c *BuildContext) FindInstaller() (*InstallerArtifact, error) {
	return c.installerOperation("find", nil, nil)
}

// RunInstaller uses artifact defaults when arguments is nil; an empty slice
// intentionally supplies no arguments. Native Windows activation honors manifests.
func (c *BuildContext) RunInstaller(artifact *InstallerArtifact, arguments []string) error {
	if err := c.ValidateInstaller(artifact); err != nil {
		return err
	}
	_, err := c.installerOperation("run", artifact, arguments)
	return err
}

package version

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

// A nil original denotes a new configuration file. Version source/mirror
// updates always carry the exact original bytes for concurrency checks.
type fileChange struct {
	path              string
	original, updated []byte
}

func replaceFile(root string, change fileChange) error {
	path, err := checkedPath(root, change.path, change.original == nil)
	if err != nil {
		return err
	}
	mode := os.FileMode(0600)
	if change.original != nil {
		info, err := os.Stat(path)
		if err != nil {
			return err
		}
		mode = info.Mode().Perm()
	}
	temporary, err := os.CreateTemp(filepath.Dir(path), ".dotask-version-*.tmp")
	if err != nil {
		return err
	}
	defer os.Remove(temporary.Name())
	if err := temporary.Chmod(mode); err != nil {
		temporary.Close()
		return err
	}
	if _, err := temporary.Write(change.updated); err != nil {
		temporary.Close()
		return err
	}
	if err := temporary.Close(); err != nil {
		return err
	}
	current, err := os.ReadFile(path)
	if change.original == nil {
		if !errors.Is(err, os.ErrNotExist) {
			return fmt.Errorf("%s appeared during initialization; review its existing settings", path)
		}
		// Linking publishes a new file atomically and refuses a concurrent file.
		return os.Link(temporary.Name(), path)
	}
	if err != nil {
		return err
	}
	if !bytes.Equal(current, change.original) {
		return fmt.Errorf("%s changed during version preparation", path)
	}
	if _, err := checkedPath(root, path, false); err != nil {
		return err
	}
	return os.Rename(temporary.Name(), path)
}

func commitChanges(changes []fileChange, write func(fileChange) error) error {
	var committed []fileChange
	for _, change := range changes {
		if bytes.Equal(change.original, change.updated) {
			continue
		}
		if err := write(change); err != nil {
			for i := len(committed) - 1; i >= 0; i-- {
				previous := committed[i]
				rollback := fileChange{previous.path, previous.updated, previous.original}
				if rollbackErr := write(rollback); rollbackErr != nil {
					err = errors.Join(err, fmt.Errorf("restoring %s failed; review the version files: %w", previous.path, rollbackErr))
				}
			}
			return err
		}
		committed = append(committed, change)
	}
	return nil
}

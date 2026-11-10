package version

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"

	dotask "dotask.local/sdk"
)

func jsonVersion(source []byte) (literal, error) {
	data := bytes.TrimPrefix(source, []byte{0xef, 0xbb, 0xbf})
	bom := len(source) - len(data)
	if !json.Valid(data) {
		return literal{}, errors.New("version mirror must contain valid JSON")
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	opening, _ := decoder.Token()
	if opening != json.Delim('{') {
		return literal{}, errors.New("version mirror must be a JSON object")
	}
	var found *literal
	for decoder.More() {
		key, _ := decoder.Token()
		var value json.RawMessage
		if err := decoder.Decode(&value); err != nil {
			return literal{}, err
		}
		if key != "version" {
			continue
		}
		if found != nil {
			return literal{}, errors.New("version mirror contains duplicate top-level version fields")
		}
		var text string
		if len(value) == 0 || value[0] != '"' || json.Unmarshal(value, &text) != nil {
			return literal{}, errors.New("version mirror needs a top-level version string")
		}
		end := bom + int(decoder.InputOffset())
		found = &literal{start: end - len(value), end: end, value: text}
	}
	if found == nil {
		return literal{}, errors.New("version mirror needs an existing top-level version string")
	}
	return *found, nil
}

func jsonChange(c *dotask.BuildContext, name, current, next string) (fileChange, error) {
	path, err := sourceFile(c, name)
	if err != nil {
		return fileChange{}, err
	}
	original, err := os.ReadFile(path)
	if err != nil {
		return fileChange{}, err
	}
	span, err := jsonVersion(original)
	if err != nil {
		return fileChange{}, fmt.Errorf("%s: %w", name, err)
	}
	if span.value != current {
		return fileChange{}, fmt.Errorf("%s version %q differs from Go source version %q; align them before updating", name, span.value, current)
	}
	replacement, err := json.Marshal(next)
	if err != nil {
		return fileChange{}, err
	}
	updated := append([]byte{}, original[:span.start]...)
	updated = append(updated, replacement...)
	updated = append(updated, original[span.end:]...)
	if next == current {
		updated = original
	}
	return fileChange{path, original, updated}, nil
}

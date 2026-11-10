package version

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	dotask "dotask.local/sdk"
)

func relativeVersionFile(c *dotask.BuildContext, name string) (string, error) {
	file, err := sourceFile(c, name)
	if err != nil {
		return "", err
	}
	root, err := filepath.EvalSymlinks(c.Root())
	if err != nil {
		return "", err
	}
	relative, err := filepath.Rel(root, file)
	return filepath.ToSlash(relative), err
}

func initializeSettings(c *dotask.BuildContext, name, symbol, jsonName, mode string, dryRun bool) error {
	relative, err := relativeVersionFile(c, name)
	if err != nil {
		return err
	}
	values := map[string]string{"version-file": relative, "version-symbol": symbol}
	if jsonName != "" {
		relative, err := relativeVersionFile(c, jsonName)
		if err != nil {
			return err
		}
		values["version-json-file"] = relative
	}
	if _, err := c.Parameter("patch-mode"); err == nil {
		values["version-patch"] = mode
	}
	for key, want := range values {
		if _, err := c.Setting("go." + key); err != nil {
			continue
		}
		existing, err := dotask.SettingAs[string](c, "go."+key)
		if err != nil {
			return err
		}
		matches := existing == want
		if key == "version-file" || key == "version-json-file" {
			existingPath, err := sourceFile(c, existing)
			if err != nil {
				return err
			}
			wantedPath, err := sourceFile(c, want)
			if err != nil {
				return err
			}
			existingInfo, err := os.Stat(existingPath)
			if err != nil {
				return err
			}
			wantedInfo, err := os.Stat(wantedPath)
			if err != nil {
				return err
			}
			matches = os.SameFile(existingInfo, wantedInfo)
		}
		if !matches {
			return fmt.Errorf("settings.go.%s is already %q; edit .dotasks.yaml explicitly to change it", key, existing)
		}
		delete(values, key)
	}
	if len(values) == 0 {
		fmt.Println("Go version settings are already configured.")
		return nil
	}
	path := filepath.Join(c.Root(), ".dotasks.yaml")
	if _, err := checkedPath(c.Root(), path, true); err != nil {
		return err
	}
	original, err := os.ReadFile(path)
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	updated, err := fillVersionSettings(original, values)
	if err != nil {
		return err
	}
	fmt.Printf("%s: save Go version settings\n", path)
	if dryRun {
		fmt.Println(string(updated))
		return nil
	}
	return replaceFile(c.Root(), fileChange{path, original, updated})
}

func lineIndent(line string) int {
	return len(line) - len(strings.TrimLeft(line, " "))
}

// Insert into block mappings or expand an empty {} mapping. Refuse nonempty
// flow/alias mappings instead of rewriting unrelated YAML or losing comments.
func ensureMapping(lines []string, start, end, indent int, key, eol string) ([]string, int, int, error) {
	pattern := regexp.MustCompile(`(?i)^([ ]*)(?:` + key + `|"` + key + `"|'` + key + `'):[ \t]*(.*?)(\r?\n)?$`)
	for i := start; i < end; i++ {
		match := pattern.FindStringSubmatchIndex(lines[i])
		if match == nil || match[3]-match[2] != indent {
			continue
		}
		value := lines[i][match[4]:match[5]]
		if strings.HasPrefix(value, "{}") {
			tail := strings.TrimLeft(value[2:], " \t")
			if tail != "" && !strings.HasPrefix(tail, "#") {
				return nil, 0, 0, fmt.Errorf("use a block-style %s mapping in .dotasks.yaml before --init", key)
			}
			prefix := lines[i][:match[4]]
			if tail == "" {
				prefix = strings.TrimRight(prefix, " \t")
			}
			lines[i] = prefix + tail + lines[i][match[5]:]
		} else if value != "" && !strings.HasPrefix(value, "#") {
			return nil, 0, 0, fmt.Errorf("use a block-style %s mapping in .dotasks.yaml before --init", key)
		}
		return lines, i, indent, nil
	}
	// The effective configuration may contain a quoted/flow mapping that is not
	// represented by a standalone key. Root flow objects need manual editing.
	if start == 0 {
		for _, line := range lines {
			trimmed := strings.TrimSpace(line)
			if strings.HasPrefix(trimmed, "{") || trimmed == "..." {
				return nil, 0, 0, errors.New("use block-style settings/go mappings in .dotasks.yaml before --init")
			}
		}
		// Append a missing root key so document markers and header comments stay
		// at the beginning of the file.
		start = end
		for start > 0 && strings.TrimSpace(lines[start-1]) == "" {
			start--
		}
	}
	newLine := strings.Repeat(" ", indent) + key + ":" + eol
	lines = insertConfigLines(lines, start, []string{newLine}, eol)
	return lines, start, indent, nil
}

func insertConfigLines(lines []string, at int, inserted []string, eol string) []string {
	if at > 0 && !strings.HasSuffix(lines[at-1], "\n") {
		lines[at-1] += eol
	}
	result := append([]string{}, lines[:at]...)
	result = append(result, inserted...)
	return append(result, lines[at:]...)
}

func childIndent(lines []string, parent, indent int) (int, int) {
	child := indent + 2
	found := false
	end := len(lines)
	for i := parent + 1; i < len(lines); i++ {
		text := strings.TrimSpace(lines[i])
		if text == "" || strings.HasPrefix(text, "#") {
			continue
		}
		current := lineIndent(lines[i])
		if current <= indent {
			end = i
			break
		}
		if !found {
			child = current
			found = true
		}
	}
	return child, end
}

func fillVersionSettings(original []byte, values map[string]string) ([]byte, error) {
	bom := bytes.HasPrefix(original, []byte{0xef, 0xbb, 0xbf})
	body := string(bytes.TrimPrefix(original, []byte{0xef, 0xbb, 0xbf}))
	if body == "" {
		body = "version: 1\n"
	}
	complexKeys := regexp.MustCompile(`(?m)^[ \t]*(?:\?[ \t]|<<[ \t]*:|"[^"\r\n]*\\[^"\r\n]*"[ \t]*:)`)
	if complexKeys.MatchString(body) {
		return nil, errors.New("use simple block-style settings/go mappings without merge or escaped keys in .dotasks.yaml before --init")
	}
	eol := "\n"
	if strings.Contains(body, "\r\n") {
		eol = "\r\n"
	}
	lines := strings.SplitAfter(body, "\n")
	lines, settings, settingsIndent, err := ensureMapping(lines, 0, len(lines), 0, "settings", eol)
	if err != nil {
		return nil, err
	}
	goIndent, end := childIndent(lines, settings, settingsIndent)
	lines, goLine, _, err := ensureMapping(lines, settings+1, end, goIndent, "go", eol)
	if err != nil {
		return nil, err
	}
	valueIndent, valueEnd := childIndent(lines, goLine, goIndent)
	var inserted []string
	for _, key := range []string{"version-file", "version-symbol", "version-json-file", "version-patch"} {
		if value, ok := values[key]; ok {
			pattern := regexp.MustCompile(`(?i)^ {` + fmt.Sprint(valueIndent) + `}(?:` + key + `|"` + key + `"|'` + key + `'):`)
			for _, line := range lines[goLine+1 : valueEnd] {
				if pattern.MatchString(line) {
					return nil, fmt.Errorf("settings.go.%s already exists in .dotasks.yaml; rerun --init to reload the current settings", key)
				}
			}
			encoded, err := json.Marshal(value)
			if err != nil {
				return nil, err
			}
			inserted = append(inserted, strings.Repeat(" ", valueIndent)+key+": "+string(encoded)+eol)
		}
	}
	lines = insertConfigLines(lines, goLine+1, inserted, eol)
	result := strings.Join(lines, "")
	if !strings.HasSuffix(body, "\n") {
		result = strings.TrimSuffix(result, eol)
	}
	if bom {
		result = "\xef\xbb\xbf" + result
	}
	return []byte(result), nil
}

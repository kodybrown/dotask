package version

import (
	"bytes"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	dotask "dotask.local/sdk"
)

func versionContext(t *testing.T, root string, parameters, settings map[string]any) *dotask.BuildContext {
	t.Helper()
	defaults := map[string]any{"init": false, "dry-run": false, "increment": false, "increment-minor": false, "increment-major": false, "increment-preview": false}
	for key, value := range parameters {
		defaults[key] = value
	}
	data := map[string]any{
		"RootDirectory": root, "TaskDirectory": root, "InvocationDirectory": root,
		"TargetFile": "update-version.go", "TargetName": "update-version", "CliExecutable": "dotask",
		"SessionDirectory": root, "Parameters": defaults, "Settings": settings,
	}
	encoded, err := json.Marshal(data)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "context.json")
	if err := os.WriteFile(path, encoded, 0600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("DOTASK_EXECUTION_CONTEXT", path)
	c, err := dotask.Load()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(c.Close)
	return c
}

func TestJSONVersionSpansAndInvalidMirrors(t *testing.T) {
	source := []byte("\xef\xbb\xbf{\r\n  \"nested\": {\"version\":\"ignore\"},\r\n  \"ver\\u0073ion\" : \"1.\\u0032.3\",\r\n  \"other\": [1,2]\r\n}")
	span, err := jsonVersion(source)
	if err != nil || span.value != "1.2.3" || string(source[span.start:span.end]) != `"1.\u0032.3"` {
		t.Fatalf("span: %+v, %v", span, err)
	}
	for _, input := range []string{`[]`, `null`, `{}`, `{"nested":{"version":"1.2.3"}}`, `{"version":null}`, `{"version":3}`, `{"version":"1.2.3","version":"1.2.3"}`, `{"version":"1.2.3",}`} {
		if _, err := jsonVersion([]byte(input)); err == nil {
			t.Errorf("accepted %s", input)
		}
	}
	root := t.TempDir()
	path := filepath.Join(root, "mygo.json")
	if err := os.WriteFile(path, source, 0600); err != nil {
		t.Fatal(err)
	}
	c := versionContext(t, root, nil, nil)
	change, err := jsonChange(c, "mygo.json", "1.2.3", "1.3.0")
	if err != nil {
		t.Fatal(err)
	}
	want := bytes.Replace(source, []byte(`"1.\u0032.3"`), []byte(`"1.3.0"`), 1)
	if !bytes.Equal(change.updated, want) {
		t.Fatalf("changed unrelated JSON bytes: %q", change.updated)
	}
	unchanged, err := jsonChange(c, "mygo.json", "1.2.3", "1.2.3")
	if err != nil || !bytes.Equal(unchanged.updated, source) {
		t.Fatalf("no-op changed escaped JSON: %v", err)
	}
	if _, err := jsonChange(c, "mygo.json", "2.0.0", "2.0.1"); err == nil {
		t.Fatal("accepted a mismatched mirror")
	}
}

func TestFillVersionSettingsPreservesYAML(t *testing.T) {
	values := map[string]string{"version-file": "cmd/app.go", "version-symbol": "Release", "version-json-file": "mygo.json"}
	for _, test := range []struct{ original, want string }{
		{"", "version: 1\nsettings:\n  go:\n    version-file: \"cmd/app.go\"\n    version-symbol: \"Release\"\n    version-json-file: \"mygo.json\"\n"},
		{"# header\n---\nversion: 1\nname: App", "# header\n---\nversion: 1\nname: App\nsettings:\n  go:\n    version-file: \"cmd/app.go\"\n    version-symbol: \"Release\"\n    version-json-file: \"mygo.json\""},
		{"\xef\xbb\xbfversion: 1\r\nsettings: {} # keep\r\nname: App\r\n", "\xef\xbb\xbfversion: 1\r\nsettings: # keep\r\n  go:\r\n    version-file: \"cmd/app.go\"\r\n    version-symbol: \"Release\"\r\n    version-json-file: \"mygo.json\"\r\nname: App\r\n"},
		{"version: 1\nSettings:\n    # keep\n    other: yes\n    Go: {}\n", "version: 1\nSettings:\n    # keep\n    other: yes\n    Go:\n      version-file: \"cmd/app.go\"\n      version-symbol: \"Release\"\n      version-json-file: \"mygo.json\"\n"},
	} {
		got, err := fillVersionSettings([]byte(test.original), values)
		if err != nil || string(got) != test.want {
			t.Errorf("input %q: got %q (%v), want %q", test.original, got, err, test.want)
		}
	}
	for _, source := range []string{"version: 1\nsettings: {go: {}}\n", "version: 1\nsettings:\n  go: &options {}\n", "version: 1\nsettings:\n  <<: *defaults\n", "version: 1\n? settings\n: {}\n", "version: 1\n\"\\u0073ettings\": {}\n", `{"version":1,"settings":{}}`} {
		if _, err := fillVersionSettings([]byte(source), values); err == nil {
			t.Errorf("accepted complex flow/alias mapping: %q", source)
		}
	}
	if _, err := fillVersionSettings([]byte("version: 1\nsettings:\n  go:\n    version-file: other.go\n"), values); err == nil {
		t.Fatal("added a duplicate key after a concurrent settings change")
	}
}

func TestInitializeSettingsDoesNotOverwriteOrIncrement(t *testing.T) {
	root := t.TempDir()
	config := filepath.Join(root, ".dotasks.yaml")
	source := filepath.Join(root, "entry.go")
	original := []byte("package main\nconst Release = \"1.2.3\"\n")
	if err := os.WriteFile(source, original, 0600); err != nil {
		t.Fatal(err)
	}
	parameters := map[string]any{"init": true, "file": "entry.go", "symbol": "Release", "dry-run": true}
	c := versionContext(t, root, parameters, nil)
	if err := Update(c); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(config); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("dry-run created config")
	}
	parameters["dry-run"] = false
	c = versionContext(t, root, parameters, nil)
	if err := Update(c); err != nil {
		t.Fatal(err)
	}
	saved, err := os.ReadFile(config)
	if err != nil || !bytes.Contains(saved, []byte(`version-symbol: "Release"`)) {
		t.Fatalf("missing settings: %q, %v", saved, err)
	}
	settings := map[string]any{"go": map[string]any{"version-file": "entry.go", "version-symbol": "Release"}}
	c = versionContext(t, root, parameters, settings)
	if err := Update(c); err != nil {
		t.Fatal(err)
	}
	parameters["symbol"] = "Other"
	if err := initializeSettings(versionContext(t, root, parameters, settings), "entry.go", "Other", "", "increment", false); err == nil {
		t.Fatal("overwrote an existing setting")
	}
	after, _ := os.ReadFile(config)
	actualSource, _ := os.ReadFile(source)
	if !bytes.Equal(after, saved) || !bytes.Equal(actualSource, original) {
		t.Fatal("initialization changed existing settings or source")
	}
	parameters["increment-minor"] = true
	if err := Update(versionContext(t, root, parameters, settings)); err == nil || !strings.Contains(err.Error(), "--init saves settings only") {
		t.Fatalf("accepted init with increment: %v", err)
	}
}

func TestFailedSecondWriteRollsBackAndPreservesConcurrentEdits(t *testing.T) {
	root := t.TempDir()
	first, second := filepath.Join(root, "version.go"), filepath.Join(root, "mygo.json")
	for _, path := range []string{first, second} {
		if err := os.WriteFile(path, []byte("old"), 0600); err != nil {
			t.Fatal(err)
		}
	}
	changes := []fileChange{{first, []byte("old"), []byte("new")}, {second, []byte("old"), []byte("new")}}
	failure := errors.New("mirror locked")
	err := commitChanges(changes, func(change fileChange) error {
		if change.path == second {
			return failure
		}
		return replaceFile(root, change)
	})
	if !errors.Is(err, failure) {
		t.Fatalf("lost failure: %v", err)
	}
	got, _ := os.ReadFile(first)
	if string(got) != "old" {
		t.Fatal("source was not restored")
	}
	err = commitChanges(changes, func(change fileChange) error {
		if change.path == second {
			if err := os.WriteFile(first, []byte("concurrent"), 0600); err != nil {
				t.Fatal(err)
			}
			return failure
		}
		return replaceFile(root, change)
	})
	got, _ = os.ReadFile(first)
	if err == nil || !strings.Contains(err.Error(), "restoring") || string(got) != "concurrent" {
		t.Fatalf("overwrote concurrent edit or lost recovery error: %q, %v", got, err)
	}
	matches, _ := filepath.Glob(filepath.Join(root, ".dotask-version-*.tmp"))
	if len(matches) != 0 {
		t.Fatalf("leaked temporary files: %v", matches)
	}
}

func TestSameDayMirrorRefreshPreservesEscapedLiteralBytes(t *testing.T) {
	root := t.TempDir()
	current, err := prepare("1.0.0", "date", operation{}, time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	escaped := strings.Replace(current, "1", `\u0031`, 1)
	files := map[string]string{
		"entry.go":  "package main\nconst Release = \"" + escaped + "\"\n",
		"mygo.json": "{\"version\":\"" + escaped + "\"}\n",
	}
	for name, contents := range files {
		if err := os.WriteFile(filepath.Join(root, name), []byte(contents), 0600); err != nil {
			t.Fatal(err)
		}
	}
	settings := map[string]any{"go": map[string]any{"version-file": "entry.go", "version-symbol": "Release", "version-json-file": "mygo.json", "version-patch": "date"}}
	if err := Update(versionContext(t, root, nil, settings)); err != nil {
		t.Fatal(err)
	}
	for name, contents := range files {
		got, _ := os.ReadFile(filepath.Join(root, name))
		if string(got) != contents {
			t.Errorf("same-day refresh rewrote %s", name)
		}
	}
}

func TestUnconfiguredJSONFileIsLeftAlone(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "entry.go")
	mirror := filepath.Join(root, "mygo.json")
	if err := os.WriteFile(source, []byte("package main\nconst Release = \"1.2.3\"\n"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(mirror, []byte("unrelated contents"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := Update(versionContext(t, root, map[string]any{"file": "entry.go", "symbol": "Release"}, nil)); err != nil {
		t.Fatal(err)
	}
	got, _ := os.ReadFile(source)
	untouched, _ := os.ReadFile(mirror)
	if !bytes.Contains(got, []byte(`"1.2.4"`)) || string(untouched) != "unrelated contents" {
		t.Fatalf("unexpected source/mirror update: %q, %q", got, untouched)
	}
}

func TestVersionUpdatesThroughLinkedProjectRoot(t *testing.T) {
	parent := t.TempDir()
	root := filepath.Join(parent, "actual")
	alias := filepath.Join(parent, "alias")
	if err := os.Mkdir(root, 0700); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(root, alias); err != nil {
		t.Skipf("directory symlinks unavailable: %v", err)
	}
	if err := os.WriteFile(filepath.Join(root, "entry.go"), []byte("package main\nconst Release = \"1.2.3\"\n"), 0600); err != nil {
		t.Fatal(err)
	}
	setup := map[string]any{"init": true, "file": filepath.Join(root, "entry.go"), "symbol": "Release"}
	if err := Update(versionContext(t, alias, setup, nil)); err != nil {
		t.Fatal(err)
	}
	config, _ := os.ReadFile(filepath.Join(root, ".dotasks.yaml"))
	if !bytes.Contains(config, []byte(`version-file: "entry.go"`)) {
		t.Fatalf("setup stored a nonportable path through a linked root: %q", config)
	}
	if err := Update(versionContext(t, alias, map[string]any{"file": "entry.go", "symbol": "Release"}, nil)); err != nil {
		t.Fatal(err)
	}
	got, _ := os.ReadFile(filepath.Join(root, "entry.go"))
	if !bytes.Contains(got, []byte(`"1.2.4"`)) {
		t.Fatalf("linked-root source was not updated: %q", got)
	}
	if _, err := checkedPath(alias, filepath.Join(parent, "outside.go"), true); err == nil {
		t.Fatal("accepted a path outside the linked project")
	}
}

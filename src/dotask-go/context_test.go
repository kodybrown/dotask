package dotask

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestTypedValuesAndPaths(t *testing.T) {
	c := &BuildContext{parameters: map[string]any{"count": json.Number("9007199254740993"), "flag": true, "bad": "12"}, settings: map[string]any{"App": map[string]any{"name": "日本語"}}, data: map[string]json.RawMessage{}, ctx: context.Background()}
	root := t.TempDir()
	c.data["RootDirectory"], _ = json.Marshal(root)
	if n, err := c.Int("COUNT"); err != nil || n != 9007199254740993 {
		t.Fatalf("integer lost precision: %d %v", n, err)
	}
	if _, err := c.Int("bad"); err == nil {
		t.Fatal("string accepted as integer")
	}
	if flag, err := c.Bool("flag"); err != nil || !flag {
		t.Fatal(flag, err)
	}
	if text, err := SettingAs[string](c, "app.NAME"); err != nil || text != "日本語" {
		t.Fatal(text, err)
	}
	if path, err := c.Path(`nested\space 日本語/file`); err != nil || path != filepath.Join(root, "nested", "space 日本語", "file") {
		t.Fatal(path, err)
	}
	if _, err := c.Path("bad\x00path"); err == nil {
		t.Fatal("NUL path accepted")
	}
}
func TestProcessFailureAndExclusiveResult(t *testing.T) {
	var failure *ProcessFailure
	if !errors.As(errors.Join(errors.New("outer"), &ProcessFailure{Code: 37, Cause: errors.New("child")}), &failure) || failure.Code != 37 {
		t.Fatal("lost exit code")
	}
	file := filepath.Join(t.TempDir(), "result.json")
	if err := writeNew(file, map[string]string{"value": "日本語"}); err != nil {
		t.Fatal(err)
	}
	before, _ := os.ReadFile(file)
	if err := writeNew(file, map[string]string{"value": "changed"}); err == nil {
		t.Fatal("overwrote result")
	}
	after, _ := os.ReadFile(file)
	if string(before) != string(after) {
		t.Fatal("result changed")
	}
}

func TestInstallerWireRequiresExplicitDiscriminators(t *testing.T) {
	for _, raw := range []string{`{"FilePath":"x","OS":0,"Architecture":1}`, `{"FilePath":"x","Kind":null,"OS":0,"Architecture":1}`, `null`} {
		var artifact InstallerArtifact
		if err := json.Unmarshal([]byte(raw), &artifact); err == nil {
			t.Fatalf("accepted incomplete artifact: %s", raw)
		}
	}
	var artifact InstallerArtifact
	if err := json.Unmarshal([]byte(`{"FilePath":"x","Kind":0,"OS":0,"Architecture":1,"DefaultArguments":[]}`), &artifact); err != nil {
		t.Fatal(err)
	}
}

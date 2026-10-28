// Package version prepares one explicit source literal. Numbering is separate
// from compiler/module versions and from builds, Git tags and publication.
package version

import (
	"bytes"
	"errors"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"time"
	_ "time/tzdata" // Make America/Denver available without an OS timezone database.

	dotask "dotask.local/sdk"
)

type operation struct{ minor, major, preview, refresh bool }

var numeric = regexp.MustCompile(`^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$`)
var identifiers = regexp.MustCompile(`^[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*$`)

func digits(value string) bool {
	if value == "" {
		return false
	}
	for _, r := range value {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}

func prepare(current, mode string, op operation, now time.Time) (string, error) {
	count := 0
	for _, flag := range []bool{op.minor, op.major, op.preview, op.refresh} {
		if flag {
			count++
		}
	}
	if count > 1 {
		return "", errors.New("choose only one version increment")
	}
	if mode != "increment" && mode != "date" {
		return "", errors.New("patch mode must be increment or date")
	}
	prefix := ""
	if strings.HasPrefix(current, "v") {
		prefix = "v"
		current = current[1:]
	}
	body, metadata, hasMetadata := strings.Cut(current, "+")
	if hasMetadata && !identifiers.MatchString(metadata) {
		return "", errors.New("invalid version build metadata")
	}
	body, prerelease, hasPrerelease := strings.Cut(body, "-")
	if hasPrerelease && !identifiers.MatchString(prerelease) {
		return "", errors.New("invalid version prerelease")
	}
	if !numeric.MatchString(body) {
		return "", errors.New("SemVer requires exactly three numeric components without leading zeroes")
	}
	parts := strings.Split(body, ".")
	components := make([]uint64, 3)
	for i, part := range parts {
		value, err := strconv.ParseUint(part, 10, 64)
		if err != nil {
			return "", errors.New("version numeric component exceeds uint64")
		}
		components[i] = value
	}
	for _, part := range strings.Split(prerelease, ".") {
		if digits(part) && len(part) > 1 && part[0] == '0' {
			return "", errors.New("numeric prerelease identifiers cannot have leading zeroes")
		}
	}
	preview := uint64(0)
	if hasPrerelease && strings.HasPrefix(prerelease, "preview.") {
		counter := strings.TrimPrefix(prerelease, "preview.")
		if !digits(counter) || len(counter) > 1 && counter[0] == '0' {
			return "", errors.New("expected preview.N with a canonical nonnegative integer")
		}
		value, err := strconv.ParseUint(counter, 10, 31)
		if err != nil {
			return "", errors.New("preview counter must not exceed Int32.MaxValue")
		}
		preview = value
	} else if hasPrerelease && op.preview {
		return "", errors.New("preview increment requires a stable version or preview.N prerelease")
	}
	increment := func(value uint64) (uint64, error) {
		if value == ^uint64(0) {
			return 0, errors.New("version increment exceeds uint64")
		}
		return value + 1, nil
	}
	var err error
	if op.major {
		components[0], err = increment(components[0])
		components[1] = 0
		components[2] = 0
	} else if op.minor {
		components[1], err = increment(components[1])
		components[2] = 0
	} else if mode == "increment" && !op.preview {
		components[2], err = increment(components[2])
	}
	if err != nil {
		return "", err
	}
	if mode == "date" {
		zone, err := time.LoadLocation("America/Denver")
		if err != nil {
			return "", err
		}
		// YYMMDD is a numeric SemVer patch, not a fourth component. Convert to an
		// integer so early-century dates never introduce forbidden leading zeroes.
		patch, err := strconv.ParseUint(now.In(zone).Format("060102"), 10, 64)
		if err != nil {
			return "", err
		}
		if patch < components[2] {
			return "", errors.New("current patch is newer than the Utah date; refusing to decrease the version")
		}
		components[2] = patch
	}
	next := fmt.Sprintf("%d.%d.%d", components[0], components[1], components[2])
	if op.major || op.minor {
		prerelease = ""
	} else if op.preview {
		if preview == 2147483647 {
			return "", errors.New("preview increment exceeds Int32.MaxValue")
		}
		prerelease = fmt.Sprintf("preview.%d", preview+1)
	}
	if prerelease != "" {
		next += "-" + prerelease
	}
	if hasMetadata {
		next += "+" + metadata
	}
	return prefix + next, nil
}

type literal struct {
	start, end int
	value      string
	raw        bool
}

func locate(source []byte, symbol string) (literal, error) {
	set := token.NewFileSet()
	file, err := parser.ParseFile(set, "version.go", source, 0)
	if err != nil {
		return literal{}, err
	}
	var expressions []ast.Expr
	for _, decl := range file.Decls {
		group, ok := decl.(*ast.GenDecl)
		if !ok || group.Tok != token.CONST && group.Tok != token.VAR {
			continue
		}
		for _, spec := range group.Specs {
			value := spec.(*ast.ValueSpec)
			for i, name := range value.Names {
				if name.Name == symbol {
					if len(value.Values) != len(value.Names) {
						return literal{}, fmt.Errorf("%s must have its own string literal", symbol)
					}
					expressions = append(expressions, value.Values[i])
				}
			}
		}
	}
	if len(expressions) != 1 {
		return literal{}, fmt.Errorf("expected exactly one top-level %s string literal", symbol)
	}
	value, ok := expressions[0].(*ast.BasicLit)
	if !ok || value.Kind != token.STRING {
		return literal{}, fmt.Errorf("%s must be a string literal", symbol)
	}
	start := set.PositionFor(value.Pos(), false).Offset
	// AST literal lengths can omit CR bytes in raw strings. Locate the physical
	// token in the original bytes instead of trusting a normalized AST span.
	end := start + 1
	if source[start] == '`' {
		offset := bytes.IndexByte(source[end:], '`')
		if offset < 0 {
			return literal{}, errors.New("unterminated raw literal")
		}
		end += offset + 1
	} else {
		for end < len(source) {
			if source[end] == '\\' {
				end += 2
				continue
			}
			end++
			if source[end-1] == '"' {
				break
			}
		}
	}
	if end > len(source) {
		return literal{}, errors.New("cannot safely locate source literal")
	}
	decoded, err := strconv.Unquote(string(source[start:end]))
	if err != nil {
		return literal{}, err
	}
	return literal{start: start, end: end, value: decoded, raw: source[start] == '`'}, nil
}

func sourceFile(c *dotask.BuildContext, name string) (string, error) {
	path, err := c.Path(name)
	if err != nil {
		return "", err
	}
	relative, err := filepath.Rel(c.Root(), path)
	if err != nil || relative == "." || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		return "", errors.New("version source must be beneath the project root")
	}
	root, err := filepath.EvalSymlinks(c.Root())
	if err != nil {
		return "", err
	}
	for _, part := range strings.Split(relative, string(filepath.Separator)) {
		root = filepath.Join(root, part)
		info, err := os.Lstat(root)
		if err != nil {
			return "", err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return "", errors.New("version source cannot contain links")
		}
	}
	info, err := os.Stat(root)
	if err != nil {
		return "", err
	}
	if !info.Mode().IsRegular() {
		return "", errors.New("version source must be a regular file")
	}
	return root, nil
}

func option(c *dotask.BuildContext, name, setting string, defaultValue string) (string, error) {
	if _, err := c.Parameter(name); err == nil {
		return c.String(name)
	}
	if _, err := c.Setting(setting); err == nil {
		return dotask.SettingAs[string](c, setting)
	}
	if defaultValue != "" {
		return defaultValue, nil
	}
	return "", fmt.Errorf("%s or settings.%s is required", name, setting)
}

func Update(c *dotask.BuildContext) error {
	var op operation
	for name, field := range map[string]*bool{"increment": &op.refresh, "increment-minor": &op.minor, "increment-major": &op.major, "increment-preview": &op.preview} {
		value, err := c.Bool(name)
		if err != nil {
			return err
		}
		*field = value
	}
	dryRun, err := c.Bool("dry-run")
	if err != nil {
		return err
	}
	name, err := option(c, "file", "go.version-file", "")
	if err != nil {
		return err
	}
	symbol, err := option(c, "symbol", "go.version-symbol", "")
	if err != nil {
		return err
	}
	if !token.IsIdentifier(symbol) || token.Lookup(symbol).IsKeyword() {
		return errors.New("version symbol must be a Go identifier")
	}
	mode, err := option(c, "patch-mode", "go.version-patch", "increment")
	if err != nil {
		return err
	}
	path, err := sourceFile(c, name)
	if err != nil {
		return err
	}
	original, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	span, err := locate(original, symbol)
	if err != nil {
		return err
	}
	next, err := prepare(span.value, mode, op, time.Now().UTC())
	if err != nil {
		return err
	}
	fmt.Printf("%s (%s): %s -> %s\n", path, symbol, span.value, next)
	if dryRun || next == span.value {
		return nil
	}
	replacement := strconv.Quote(next)
	if span.raw {
		replacement = "`" + next + "`"
	}
	updated := make([]byte, 0, len(original)+len(replacement))
	updated = append(updated, original[:span.start]...)
	updated = append(updated, replacement...)
	updated = append(updated, original[span.end:]...)
	// Atomic replacement preserves all bytes outside the one literal, including
	// comments, encoding/BOM, EOLs and unrelated version/build metadata fields.
	info, err := os.Stat(path)
	if err != nil {
		return err
	}
	temporary, err := os.CreateTemp(filepath.Dir(path), ".dotask-version-*.tmp")
	if err != nil {
		return err
	}
	defer os.Remove(temporary.Name())
	if err = temporary.Chmod(info.Mode().Perm()); err != nil {
		temporary.Close()
		return err
	}
	if _, err = temporary.Write(updated); err != nil {
		temporary.Close()
		return err
	}
	if err = temporary.Close(); err != nil {
		return err
	}
	current, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	if !bytes.Equal(current, original) {
		return errors.New("version source changed during preparation; no update written")
	}
	if _, err = sourceFile(c, name); err != nil {
		return err
	}
	return os.Rename(temporary.Name(), path)
}

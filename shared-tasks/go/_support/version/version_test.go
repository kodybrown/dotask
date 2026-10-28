package version

import (
	"bytes"
	"strings"
	"testing"
	"time"
)

func TestPrepareSemVer(t *testing.T) {
	now := time.Date(2026, 10, 28, 1, 0, 0, 0, time.UTC)
	for _, test := range []struct {
		current string
		op      operation
		want    string
	}{
		{"v1.2.3", operation{}, "v1.2.4"},
		{"1.2.3-preview.4+build.007", operation{}, "1.2.4-preview.4+build.007"},
		{"1.2.3", operation{preview: true}, "1.2.3-preview.1"},
		{"1.2.3-preview.4", operation{preview: true}, "1.2.3-preview.5"},
		{"1.2.3-preview.4+commit", operation{minor: true}, "1.3.0+commit"},
		{"1.2.3-preview.4", operation{major: true}, "2.0.0"},
		{"1.2.3-rc.1", operation{}, "1.2.4-rc.1"},
	} {
		got, err := prepare(test.current, "increment", test.op, now)
		if err != nil || got != test.want {
			t.Errorf("%s: got %q (%v), want %q", test.current, got, err, test.want)
		}
	}
}

func TestDatePatchUsesUtahAndPreservesSemVer(t *testing.T) {
	for _, test := range []struct {
		instant, current string
		op               operation
		want             string
	}{
		{"2026-10-28T01:00:00Z", "v1.5.261023", operation{}, "v1.5.261027"},
		{"2026-01-01T01:00:00Z", "1.2.3", operation{}, "1.2.251231"},
		{"2026-03-08T08:59:00Z", "1.2.3-preview.2", operation{}, "1.2.260308-preview.2"},
		{"2026-03-08T09:00:00Z", "1.2.260308-preview.2", operation{preview: true}, "1.2.260308-preview.3"},
		{"2026-11-01T07:15:00Z", "1.2.261101-preview.2", operation{}, "1.2.261101-preview.2"},
		{"2026-11-01T08:15:00Z", "1.2.261101-preview.2", operation{}, "1.2.261101-preview.2"},
		{"2026-10-28T01:00:00Z", "1.2.3-preview.7", operation{minor: true}, "1.3.261027"},
		{"2026-10-28T01:00:00Z", "1.2.3-preview.7", operation{major: true}, "2.0.261027"},
		{"2001-01-01T12:00:00Z", "1.2.0", operation{}, "1.2.10101"},
	} {
		now, err := time.Parse(time.RFC3339, test.instant)
		if err != nil {
			t.Fatal(err)
		}
		got, err := prepare(test.current, "date", test.op, now)
		if err != nil || got != test.want {
			t.Errorf("%s: got %q (%v), want %q", test.instant, got, err, test.want)
		}
	}
}
func TestInvalidVersionsAndConflictingIncrements(t *testing.T) {
	for _, test := range []struct {
		version, scheme string
		op              operation
	}{
		{"1.2.3", "increment", operation{minor: true, major: true}},
		{"1.2.3", "date", operation{refresh: true, preview: true}},
		{"1.2.3.4", "increment", operation{}},
		{"01.2.3", "increment", operation{}},
		{"1.2.3-preview.01", "increment", operation{}},
		{"1.2.3-preview.2147483647", "increment", operation{preview: true}},
		{"1.2.18446744073709551615", "increment", operation{}},
		{"1.2.261228", "date", operation{}},
		{"1.2.3+", "increment", operation{}},
		{"1.2.3", "unknown", operation{}},
	} {
		if got, err := prepare(test.version, test.scheme, test.op, time.Date(2026, 10, 28, 1, 0, 0, 0, time.UTC)); err == nil {
			t.Errorf("accepted %s/%s: %s", test.scheme, test.version, got)
		}
	}
}

func TestLiteralSpansPreserveBOMCommentsAndLineEndings(t *testing.T) {
	for _, quoted := range []string{`"v1.2.3"`, "`v1.2.3`", `"v1.\u0032.3"`} {
		original := []byte("\xef\xbb\xbf// 日本語 header\r\npackage main\r\n// const Version = \"ignored\"\r\nvar (\r\n  Version = " + quoted + " // keep\r\n  Other = \"unchanged\"\r\n)\r\n")
		span, err := locate(original, "Version")
		if err != nil {
			t.Fatal(err)
		}
		if span.value != "v1.2.3" {
			t.Fatal(span.value)
		}
		if !bytes.Equal(original[span.start:span.end], []byte(quoted)) {
			t.Fatal("span changed literal bytes")
		}
		updated := append([]byte{}, original[:span.start]...)
		updated = append(updated, []byte(`"v1.2.4"`)...)
		updated = append(updated, original[span.end:]...)
		if string(updated) != strings.Replace(string(original), quoted, `"v1.2.4"`, 1) {
			t.Fatal("unrelated bytes changed")
		}
	}
	for _, source := range []string{
		"package main\nvar Version = makeVersion()\n",
		"package main\nconst (Version = \"1.2.3\"; Version = \"1.2.4\")\n",
		"package main\nvar Other = \"1.2.3\"\n",
		"package main\nvar Version, Other = multi()\n",
	} {
		if _, err := locate([]byte(source), "Version"); err == nil {
			t.Errorf("accepted unsafe source: %s", source)
		}
	}
}

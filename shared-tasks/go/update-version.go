// dotask: 1
// description: Prepare a Go source version explicitly, using SemVer with numeric or Utah date-based patches. Never build or commit.
// options:
//   - {name: file, type: path, description: Go source file; otherwise settings.go.version-file.}
//   - {name: symbol, description: Top-level string const/var; otherwise settings.go.version-symbol.}
//   - {name: patch-mode, choices: [increment, date], description: Patch policy; otherwise go.version-patch or increment.}
//   - {name: increment, type: bool, default: false, description: Explicit patch increment or Utah date refresh (also the default).}
//   - {name: increment-preview, type: bool, default: false, description: "Increment preview.N, starting at preview.1 for a stable version."}
//   - {name: increment-minor, type: bool, default: false}
//   - {name: increment-major, type: bool, default: false}
//   - {name: dry-run, type: bool, default: false}
//
// requires: [{kind: file, value: go/_support/version/version.go}]
// end-dotask
package main

import (
	dotask "dotask.local/sdk"
	"dotask.local/task/_support/version"
)

func main() { dotask.Run(version.Update) }

// dotask: 1
// description: Prepare a configured Go source version and optional JSON mirror, or initialize the version settings.
// options:
//   - {name: file, type: path, description: Go source file; otherwise settings.go.version-file.}
//   - {name: symbol, description: Top-level string const/var; otherwise settings.go.version-symbol.}
//   - {name: json-file, type: path, description: Optional JSON file with a top-level version string; otherwise settings.go.version-json-file.}
//   - {name: init, type: bool, default: false, description: Save missing version settings in .dotasks.yaml without incrementing.}
//   - {name: patch-mode, choices: [increment, date], description: Patch policy; otherwise go.version-patch or increment.}
//   - {name: increment, type: bool, default: false, description: Explicit patch increment or Utah date refresh (also the default).}
//   - {name: increment-preview, type: bool, default: false, description: "Increment preview.N, starting at preview.1 for a stable version."}
//   - {name: increment-minor, type: bool, default: false}
//   - {name: increment-major, type: bool, default: false}
//   - {name: dry-run, type: bool, default: false}
//
// requires: [{kind: file, value: go/_support/version/version.go}, {kind: file, value: go/_support/version/files.go}, {kind: file, value: go/_support/version/mirror.go}, {kind: file, value: go/_support/version/config.go}]
// end-dotask
package main

import (
	dotask "dotask.local/sdk"
	"dotask.local/task/_support/version"
)

func main() { dotask.Run(version.Update) }

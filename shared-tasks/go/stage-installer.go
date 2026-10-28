// dotask: 1
// description: Test, vet, build and verify a native Go application's exact installer payload outside source.
// options:
//   - {name: stage-dir, type: path, required: true, description: Fresh external installer staging directory.}
//   - {name: app-version, required: true, description: Expected application version without a leading v.}
//
// requires: [{kind: tool, value: go}, {kind: file, value: go/_support/installer/installer.go}]
// end-dotask
package main

import (
	dotask "dotask.local/sdk"
	"dotask.local/task/_support/installer"
)

func main() { dotask.Run(installer.Stage) }

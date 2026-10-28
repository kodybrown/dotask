// dotask: 1
// description: Test, vet, build and verify a Go application, then create its installer with the precompiled native helper.
// options:
//   - {name: output, type: path, description: Alternate installer output parent directory.}
//   - {name: builder, description: Prebuilt doinstall-builder command or executable path.}
//   - {name: installer, type: path, description: Actual matching doinstall runtime executable.}
//   - {name: launcher-template, type: path, description: Actual matching doinstall launcher executable.}
//
// requires: [{kind: task, value: doinstall/create-installer.task}, {kind: task, value: doinstall/assemble-self-extracting.task}, {kind: file, value: go/_support/installer/installer.go}]
// end-dotask
package main

import (
	dotask "dotask.local/sdk"
	"dotask.local/task/_support/installer"
)

func main() { dotask.Run(installer.Create) }

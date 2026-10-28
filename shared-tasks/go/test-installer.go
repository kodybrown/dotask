// dotask: 1
// description: Validate, install, inspect, reinstall and uninstall a native Go installer in temporary roots without changing persistent PATH.
// options:
//   - {name: installer, type: path, required: true, description: Existing self-extracting DoInstall installer to test; never builds it.}
//   - {name: app-version, description: Expected application version; otherwise the configured Go version literal.}
//   - {name: timeout-seconds, type: int, default: 30, description: Positive timeout for each installer or application process.}
//
// requires: [{kind: file, value: go/_support/installer/installer.go}, {kind: file, value: go/_support/installer/acceptance.go}, {kind: file, value: go/_support/installer/acceptance_windows.go}, {kind: file, value: go/_support/installer/acceptance_other.go}]
// end-dotask
package main

import (
	dotask "dotask.local/sdk"
	"dotask.local/task/_support/installer"
)

func main() { dotask.Run(installer.Test) }

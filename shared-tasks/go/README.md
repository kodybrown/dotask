# Go application tasks

Refresh with `dotask --save go/create-installer.go`, then install with
`dotask --add go/create-installer.go`. Its dependencies are the native
`doinstall` creator/assembly adapters; there is no public Go staging task.

The shared Go creator owns the full preparation sequence: read the version,
validate the inventory, run tests/vet, build the native executable with trimmed
paths, check private paths and the version, prepare the exact files privately,
then ask the precompiled helper to gather/package/record/return the installer.
Project entry points such as create-myapp-installer stay thin.

Use installed stable Go 1.22+, a matching updated prebuilt dotask bundle with
`sdk/go` and `dotask-doinstall-create-installer`, and matching prebuilt DoInstall
tools. The helper must report schema 1 / InputPayload 1 from `--capabilities`.
The creator probes the helper beside its actual CLI executable without task
context, so probing an old helper cannot execute a recipe accidentally. Missing
SDK/helper support fails before the application's build. Upgrade the bundle
explicitly; tasks never install tools or update an active installation.

Creation requires no Rust/Cargo or .NET. It never installs the result, increments
versions or downloads a Go toolchain. Application dependencies may use Go's
normal external module cache/proxy; module files are readonly.

Configure `.dotasks.yaml`, for example:

```yaml
version: 1
settings:
  go:
    package: ./cmd/myapp
    executable: myapp.exe
    version-file: cmd/myapp/version.go
    version-symbol: Version
    version-output: 'myapp v{version}'
    installer-files: [README.md, LICENSE.md]
    clean-environment-prefixes: [MYAPP_]
  installer:
    config: .tasks/installer.yaml
    output: artifacts/installers
```

`version-symbol` must occur exactly once as a top-level const/var string literal
in `version-file`. A leading `v` is removed for installer metadata. Parsing uses
the Go syntax tree, supports raw/escaped strings and comments, and never executes
or rewrites source. `version-output` contains `{version}` and must equal the
executable's trimmed stdout. `version-arguments` defaults to `[--version]`; an
explicit empty array supplies no arguments.

`executable` is a plain native filename (`.exe` on Windows). `installer-files`
lists explicit portable project-relative files, copied at the same paths.
No recursive globs, links, directories, traversal or duplicate destinations are
accepted. An omitted/empty inventory prepares only the executable. Clean prefixes
remove application/site environment from the version smoke check. Tests/vet/build
use the project root. The native build supports x64/arm64 with Cgo off, readonly
modules, trimpath, no VCS stamp, controlled Go settings and GOTOOLCHAIN=local.

Private preparation is outside source and is removed after success; failures
print its retained location. No consumer go.mod/go.sum or generated task files
are created. The precompiled helper receives this directory as input-payload;
the source-only installer YAML remains authoritative for gathered file selection:

```yaml
build-steps:
  - gather:
      - from: '${input-payload}'
        files: [myapp.exe, README.md, LICENSE.md]
  - run: _/doinstall/assemble-self-extracting.task
    with:
      builder: '${builder}'
      installer: '${installer}'
      launcher-template: '${launcher-template}'
      config: '${config}'
      output: '${package}'
      result-file: '${result-file}'
```

Run `dotask _/go/create-installer.go`, or call it through the Go SDK's
`CreateInstaller` and return `SetInstallerResult` from a thin project entry.
It accepts output/builder/installer/launcher-template overrides. The native helper
discovers companion tools, preserves nested context, uses the builder's actual
artifact path and records only fully successful builds. Keep gather.files aligned
with the prepared executable/files and expected acceptance inventory. See
[installer authoring](../../docs/INSTALLATION.md#self-extracting-build-and-gather-workflow)
and the [Go SDK contract](../../docs/TARGETS.md#go-tasks).

To preview unpublished sources, set DOTASK_ONLINE_TASKS to the absolute dotask
shared-tasks directory before save/add. When replacing an installed old creator,
remove its tracked go/create-installer and go/stage-installer entries, then save/
add the new creator and sync the native adapter/Go acceptance. Never edit managed
files or hashes by hand. Update source YAML to gather input-payload rather than
calling stage-installer. Review and preserve local edits before replacement.

## Installer acceptance

Refresh/add `go/test-installer.go` through the shared-task manager, then run:

```powershell
dotask test-installer
```

With no local wrapper needed, the unique `test-installer` shortcut resolves to
this shared Go task. It tests an existing native DoInstall installer without
building it. Omit `--installer` to use the SDK's configured
`installer.installer_file` resolution, or otherwise the last successful build
record in `artifacts/.dotask/installers/<os>-<arch>.json`. These machine/platform
build results are separate from `.dotasks.yaml` project configuration. The
native resolver checks recorded artifact existence, platform and SHA-256; a
missing/changed/invalid record is reported without guessing paths or rebuilding.
`--installer PATH` explicitly selects another artifact and bypasses record
selection without rewriting that record. The selected path is printed.
Acceptance currently requires Windows x64/arm64; runtime verification here is
Windows x64. Other hosts fail explicitly before attempting installation.
It uses the same `settings.go` executable, payload files, version literal,
version arguments/output and clean environment prefixes as staging. The expected
version defaults to the configured source literal; `--app-version` overrides it
for testing an older artifact. Optional settings:

```yaml
settings:
  go:
    installer-command: myapp.exe
    installer-forbidden-files: [.myapp, .env]
```

`installer-command` defaults to `go.executable` and selects the installed command
shim when its name differs from the payload executable. Forbidden entries are
case-insensitive basenames checked recursively throughout the installation.
The task verifies validation creates no installation, performs a temporary
install, checks the exact recursive payload and version, reinstalls, checks the
version again, then uninstalls and verifies the command is gone. It disables
PATH/additional-command options, redirects application data into the fixture and
compares real unexpanded user/machine PATH registry values and types afterwards.

Each process has a positive `--timeout-seconds` (default 30, maximum 86400).
Each Windows invocation starts suspended in its own kill-on-close job before
executing. Cancellation/timeout stops its descendants and waits for their exit. Failures report and
retain temporary fixtures with numbered stdout/stderr files; successful fixtures
are removed only after verifying their exact parent/name beneath canonical TEMP.
Windows executable sharing violations get bounded cleanup retries; permissions
are never forced. This does not touch an active installation or persistent PATH.

## Explicit SemVer preparation

Refresh/add `go/update-version.go`. With no local wrapper needed,
`dotask update-version` resolves to that shared task. It edits the one explicitly
configured `go.version-file` / `go.version-symbol` top-level string const/var,
the same source used by installer creation. No filename, symbol, generated
package or additional historical version store is assumed.

[Go module releases use three-part SemVer](https://go.dev/doc/modules/version-numbers),
usually represented by tags such as `v1.5.261023`. A date is allowed as the numeric
patch; Go gives it no special meaning. The module's own release version is not
stored in `go.mod`. CLI binaries can also use
[linker-injected variables](https://pkg.go.dev/cmd/link) or
[Go build information](https://pkg.go.dev/runtime/debug#ReadBuildInfo). This task
uses an explicit source literal and does not introduce those build mechanisms,
create tags, change module paths, build, restore or publish.

The built-in patch policies are `increment` (default) and `date`. Configure
`go.version-patch: date`, or pass `--patch-mode date`, for `major.minor.yyMMdd` in
America/Denver, including DST. Both policies produce valid three-part SemVer.
This is reusable shared behavior, not a project-specific date generator.
For example, `v1.5.261023` refreshes to `v1.5.261027` on October 27 in Utah.

| Invocation | Increment policy | Date policy |
| --- | --- | --- |
| no increment flag / `--increment` | Increment patch; preserve prerelease | Refresh Utah date patch; preserve prerelease |
| `--increment-preview` | Preserve numeric version; start/increment preview.N | Refresh date patch; start/increment preview.N |
| `--increment-minor` | Increment minor; reset patch; clear prerelease | Increment minor; refresh date patch; clear prerelease |
| `--increment-major` | Increment major; reset minor/patch; clear prerelease | Increment major; reset minor; refresh date patch; clear prerelease |
| `--dry-run` | Print without writing | Print without writing |

The increment/preview/minor/major/dry-run flags follow the updated C# task's
explicit preparation workflow, using a Go-compatible three-part version instead
of C#'s four-part calendar version. Flags are mutually exclusive. A leading `v`
and `+build.metadata` are preserved. Preview counters have the C# nonnegative
Int32 range. Other valid SemVer prereleases survive a normal patch update;
`--increment-preview` requires stable/preview.N input.

A same-day date refresh is a byte-preserving no-op; increment preview explicitly
for multiple preparations on that date. Older date patches are refused rather
than decreasing an existing version. Numeric components are checked uint64
values. YYMMDD is serialized as an integer, dropping forbidden leading zeroes
for early-century dates. Embedded standard-library timezone data avoids any
external OS timezone database dependency.

`--file` and `--symbol` override the configured source. Only the selected literal
changes, preserving raw/double quote style, UTF-8 BOM, comments, whitespace, EOLs,
unrelated declarations and file permissions. Computed/ambiguous values, links,
paths outside the project and invalid versions are rejected. Updates compare the
original bytes before atomic replacement. Module files are never modified.
Build/test/installer tasks never call update-version automatically.

```yaml
settings:
  go:
    version-file: cmd/myapp/version.go
    version-symbol: Version
    version-patch: date
```

```go
package main

const Version = "v1.5.261023"
```

```powershell
dotask update-version --dry-run
dotask update-version --increment-preview
```

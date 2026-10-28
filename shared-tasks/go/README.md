# Go application tasks

Refresh with `dotask --save go/create-installer.go`, then install with
`dotask --add go/create-installer.go`. Its dependencies include
`go/stage-installer.go` and the native `doinstall` creator/assembly adapters.
Use Go 1.22+, prebuilt dotask with `sdk/go` and its companion
`dotask-doinstall-create-installer`, and matching prebuilt doinstall tools.
Creation uses no Rust/Cargo or .NET. It never installs the result, increments
versions, or downloads a Go toolchain. The project's own Go dependencies may
use Go's normal external module cache/proxy; module files are readonly.

Configure the native application in `.dotasks.yaml`, for example:

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
or rewrites version source. `version-output` contains `{version}`; its expansion
must equal the executable's trimmed stdout. `version-arguments` defaults to
`[--version]`; an explicit empty array supplies no arguments.

`executable` is a plain native filename (`.exe` on Windows). `installer-files`
contains explicit portable project-relative file paths, copied at the same paths.
No recursive globs, links, directories, traversal or duplicate destinations are
accepted. An omitted/empty inventory distributes only the executable. Configure
`clean-environment-prefixes` to remove site/application settings from the version
smoke check. Tests/vet/build keep the project root as their working directory.

The shared tasks build for the current OS on x64/arm64. They test and vet `./...`,
then build `go.package` with readonly modules, trimpath, no VCS stamp and Cgo off.
Inherited Go controls are cleared, and `GOTOOLCHAIN=local` prevents downloads.
A newer compiler required by the application's module must already be installed.
The fresh stage must be outside source; its parent must exist. Failed staging
remains available through the creator's diagnostics. Executables containing
checkout/home paths or failing the configured version check are refused.

In the source-only installer YAML, use these build steps, plus your normal
application/runtime schema:

```yaml
build-steps:
  - run: _/go/stage-installer.go
    with:
      stage-dir: '${staging}/app'
      app-version: '${version}'
    gather:
      - from: '${staging}/app'
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

Run `dotask _/go/create-installer.go`, or call it with the Go SDK's
`CreateInstaller` from a thin project task and return `SetInstallerResult`.
It accepts `output`, `builder`, `installer` and `launcher-template` overrides.
The native helper discovers builder companion tools, preserves nested context,
uses the builder's actual artifact path and records only fully successful builds.
Keep `gather.files` aligned with your exact configured payload inventory.
See [installer authoring](../../docs/INSTALLATION.md#self-extracting-build-and-gather-workflow)
and the [Go SDK contract](../../docs/TARGETS.md#go-tasks).

To preview unpublished shared changes locally, set `DOTASK_ONLINE_TASKS` to the
absolute dotask `shared-tasks` directory before `--add`. Replace a previously
tracked Rust creator with `--remove _/doinstall/create-installer` before adding
the Go group; never edit its managed files or tracking hashes manually.

## Installer acceptance

Refresh/add `go/test-installer.go` through the shared-task manager, then run:

```powershell
dotask _/go/test-installer.go --installer '<existing self-extracting installer>'
```

This Go task tests an existing native DoInstall installer without building it.
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

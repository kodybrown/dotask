# Application installers

`dotask install` creates and runs a fresh installer for console and GUI applications.
The shared adapters live under `_/doinstall/`, with generic launching under
`_/installer/`; they support arbitrary application payloads. Application builds
remain project-specific.
Custom `create-installer` targets and other installer technologies remain supported.

Dotask and doinstall tooling are installed separately. Dotask's application
payload contains its command and SDK helpers; packaging does not download or
install doinstall tools automatically. Its installer package carries the runtime
as `installer`, which is also retained for uninstall, as with other applications.
Consumer projects supply their own doinstall runtime/builder pair. The native CLI
has no doinstall-specific behavior; optional shared tasks invoke its ordinary
command-line interface, just like adapters for other external utilities.

## Where installer configuration comes from

The source `.tasks/installer.yaml` separates three responsibilities:

- `application` declares application identity and banner metadata.
- `build-steps` runs producer tasks, gathers their expected files, assembles the
  installer directory, and optionally runs archive tasks.
- `runtime` defines destination behavior: `interactive`, `inputs`, `defaults`,
  `profiles`, `values`, `commands`, `shortcuts`, and application-owned `settings`.

The creator removes `build-steps`, supplies the resolved version/build, platform,
architecture, and payload location, and preserves the nested `runtime` mapping
in the distributed `installer.yaml`. The standalone installer reads that file
beside its executable, or the file selected with `--config`. It never discovers
`.dotasks.yaml`, runs build tasks, or accepts source-only `build-steps`.
Old root-level runtime fields are rejected; there is no compatibility loader.

In dotask's source project, `.dotasks.yaml` points `settings.installer.config`
to `.tasks/installer.yaml`. Top-level project name/description are not copied
automatically. Application version policy lives in `settings.app`:

```yaml
settings:
  app:
    version: '0.1.yyDDD.HHmm'
    git-hash: short           # none, short (7 characters), or full
    dirty-suffix: true
  installer:
    config: .tasks/installer.yaml
    output: artifacts/installers
```

Version tokens use UTC: `yyyy`, `yy`, `MM` (month), `dd` (day), `DDD` (day of
year), `HH` or `hh` (24-hour clock), and `mm` (minute). Other characters are
literal. For example, `0.1.yyMM.ddhh` retains the requested calendar shape,
and `0.1.yyDDD.HHmm` includes ordinal day and minute. An existing destination
version directory stops creation with an error; the creator never advances the
clock or overwrites an earlier build.

A source configuration can compose ordinary tasks:

```yaml
schema: 1
application:
  id: example
  name: Example App
build-steps:
  - run: build
    with:
      stage-dir: '${staging}/app'
    gather:
      - from: '${staging}/app'
        files: ['example${exe-extension}', 'sdk/**']
        to: .
  - run: _/doinstall/assemble
    with:
      builder: '${builder}'
      installer: '${installer}'
      config: '${config}'
      output: '${package}'
  - run: _/archive/create-zip
    enabled: false
    with:
      source: '${package}'
      output: '${package}.zip'
runtime:
  interactive: true
  commands:
    - name: example
      executable: 'example${exe-extension}'
  shortcuts: []
  inputs: {}
  defaults: {}
```

Build steps run in order. A task failure stops the sequence; missing expected
files and payload collisions are errors. `gather.files` accepts exact portable
relative file paths and directory selections ending in `/**`, preserving their
structure beneath `to` (payload root by default). Producers need no output
registration: their parameters and the expected-file declarations establish the
relationship. Configure SDK file selections in `.tasks/sdks.yaml` for the reusable
`build-sdks` task; installer payload selections belong in `build-steps`.

Available build values are `${staging}`, `${payload}`, `${config}`, `${package}`,
`${version}`, `${build-info}`, `${exe-extension}`, and `${settings.NAME}`.
The doinstall creator also supplies `${builder}` (the `--builder` option,
`settings.doinstall.builder`, or `doinstall-builder` on PATH) and `${installer}`
(the `--installer` option or `settings.doinstall.installer`, resolved from the
project root). Supply the actual runtime executable, not a command launcher shim.
`build-info` is JSON metadata for a producer that embeds the same application
version/stamp. Runtime input placeholders, such as `${shortcut-name}`, remain for
the destination installer. Only command/shortcut executable paths resolve build
placeholders inside `runtime`.

| Installer YAML | Purpose |
| --- | --- |
| `application.name`, `description`, `copyright` | Console banner metadata |
| `runtime.shortcuts[].start-menu`, `desktop`, `local` | Permit shortcut locations |
| `runtime.defaults.*.start-menu-shortcuts`, `desktop-shortcuts`, `local-shortcuts` | Default selection of permitted shortcuts |
| `runtime.defaults.*.additional-command` | Default for a second command/shim, initially false |
| `runtime.defaults.*.add-to-path` | Default for Windows user PATH addition |
| `runtime.interactive`, `inputs`, `defaults`, `values` | Prompt mode, labels, defaults, and supplied values |

An additional-command default of false sets the initial No answer. The engine
has no separate permission flag to forbid that additional shim. Dotask declares
no shortcuts, so it offers no Start Menu or desktop shortcut questions.

## Shared tasks

Install the optional adapters using the tool:

```sh
dotask --add "_/doinstall/*" "_/installer/install"
```

Review conflicts rather than deleting locally modified tasks. Remove obsolete
tracked installer tasks explicitly so short names do not become ambiguous.
An exact project `install` can wrap `_/installer/install`. Updating the CLI
alone does not update committed task copies. Until published, use the
[local catalog override](SHARED-TASKS.md#official-catalog-and-unpublished-preview).

`_/doinstall/assemble.task` invokes a prebuilt builder directly, with no C# or
Rust task compilation. For an already-resolved configuration and payload:

```powershell
dotask --add "_/doinstall/assemble.task"
dotask doinstall/assemble --installer C:/Tools/doinstall/doinstall.exe --config .tasks/resolved-installer.yaml --output artifacts/example-installer
```

The builder defaults to `doinstall-builder` on PATH; `--builder` accepts an
explicit executable path. `--installer` must point to the matching actual
runtime executable because a PATH command may be a launcher shim. Both tools
must match the host OS/architecture. The adapter takes resolved doinstall YAML,
not dotask's source-only `build-steps`, and does not return an `InstallerArtifact`.
Missing tools and builder failures stop execution; no fallback build or download
occurs. Configure recurring paths in `targets._/doinstall/assemble.defaults`.

Replace old tracked `assemble.rs` copies explicitly with `--remove` followed by
`--add`; shared sync does not silently change their implementation. Review local
edits first. The build/gather creator, source-build helper, retained-uninstaller
adapter and technology-neutral install launcher remain Rust tasks and require
Rust/Cargo when executed. Converting those dynamic workflows is separate work.

The shared creator accepts `installer.config` (`--config`) and
`installer.output` (`--output`), with optional `--app-version` and `--build-stamp`
overrides. Paths are project-relative or absolute. A project `create-installer`
can call the shared creator and forward its `InstallerArtifact`. The shared
`install` task requires that creator, waits for it to succeed, then runs the
returned artifact with its exact argument tokens.

Packages live beneath
`<output>/<os>-<architecture>/<app-id>-<version>[-<revision>][-dirty]/` and contain:

```text
installer.exe             # installer on Linux/macOS
installer.yaml            # resolved application and nested runtime settings
payload/
```

Distribute the entire directory. It contains one installer executable without a
companion language runtime. The destination does not need dotask, Rust, or an SDK
to install; application runtime requirements depend on the payload. Build-machine
assembly belongs to the separate `doinstall-builder` executable, which is
not distributed. The shipped installer has no `package` command.

ZIP and 7z creation are ordinary optional `build-steps` using
`_/archive/create-zip` and `_/archive/create-7z`. Each uses a configurable `7z`
executable and includes the installer directory as the archive's top-level
folder. Enable one or both, or substitute a project task for another format.
Archive flags do not belong to the destination installer.

Supply binaries matching the host OS/architecture; cross-platform release
production is not automated. Native acceptance is recorded separately in
[VERIFICATION.md](VERIFICATION.md).

## Installer runtime configuration

Doinstall is an independent project. Its authoritative YAML, prompts, shortcuts,
ownership, receipt, retention, and uninstall contract lives in
[the doinstall repository](https://github.com/kodybrown/doinstall/blob/main/docs/INSTALLATION.md).
Dotask keeps generic artifact launching and shared tool adapters; it has no
installer runtime, builder, shim implementation, or bundled shim assets.

The project can use prebuilt doinstall and doinstall-builder tools with the
`_/doinstall/assemble` wrapper's explicit paths. For local development, set
`settings.doinstall.source` to an external checkout and explicitly run
`_/doinstall/build`. That wrapper delegates to the external project's Cargo
helper and can stage both tools. It does not add the external package to dotask's
Cargo workspace. No source download or tool installation happens implicitly.
Consumers do not need `settings.doinstall.source`. Keep the builder/runtime pair
from the same doinstall build; this adapter requires its schema-1 resolved YAML
and `--installer`, `--config`, `--output` interface. Doinstall owns that interface;
dotask owns the optional task files and their add/sync lifecycle.

## Build and install dotask itself

For the complete checkout-local workflow, run:

```powershell
cd C:\Users\kodyb\Projects\dotask
.\build.cmd create-dotask-installer
```

Unix uses `./build.sh create-dotask-installer`. This local Rust task builds and
stages the configured doinstall checkout, runs its full verification gate, runs
dotask's full gate against the staged pair, then calls `create-installer` with
those exact binaries. Any failure stops later steps. Tools are temporary and stay
outside source and deliverables; the task returns an `InstallerArtifact` and
does not launch it. No environment or project settings are rewritten. Select a
different checkout with `--source`; otherwise the build helper uses
`DOINSTALL_SOURCE` or `settings.doinstall.source` (`../doinstall` here).
`--app-version` and `--build-stamp` forward to packaging. This task is local to
dotask and is not distributed through the shared catalog.

To use an already-prepared tool pair instead, keep using `create-installer`:

The local Rust `create-installer` task delegates to
`_/doinstall/create-installer`. Its `.tasks/installer.yaml` build steps
run `build` to prepare the native app and SDKs, gather the declared files, and call
`_/doinstall/assemble`. Optional archive steps follow. There is no local
`pack` task; `_/dotnet/pack` remains a NuGet task for consuming C# projects.
Tool selection comes from `--builder` / `--installer` or `settings.doinstall`
as described above. Missing runtime paths fail before building dotask. Creating
its installer never automatically builds doinstall. The source-build helper is
an explicit development convenience, not a consumer prerequisite.

`build-sdks` publishes the maintained C# helper using evaluated MSBuild
`PublishDir`, then copies the Rust SDK selections from `.tasks/sdks.yaml`.
Compiler output stays external. Only the final independent installer directory
is created beneath `settings.installer.output`. Destination machines need
Rust/Cargo to execute Rust tasks and the .NET SDK to execute C# tasks; neither is
needed for metadata, help, initialization, completion, or installer execution.
No language runtime, SDK, Roslyn, or managed support host is bundled.

```powershell
.\build.cmd                         # Bootstrap and list available tasks.
$env:DOINSTALL_TOOL_DIR = 'C:/Tools/doinstall' # Prebuilt, host-matching pair.
.\build.cmd verify                  # Includes external adapter integration.
.\build.cmd create-installer --builder "$env:DOINSTALL_TOOL_DIR/doinstall-builder.exe" --installer "$env:DOINSTALL_TOOL_DIR/doinstall.exe"
# Use fresh, isolated locations for acceptance:
# Run the generated installer.exe with --non-interactive --set add-to-path=false
# --install-dir C:/Temp/dotask-acceptance/apps/dotask --bin-dir C:/Temp/dotask-acceptance/bin
```

For repeated `dotask install` calls, configure `settings.doinstall.builder` and
`settings.doinstall.installer` in `.dotasks.yaml`; the generic install target does
not forward tool-selection options. Unix uses actual tool paths without `.exe`.
`verify` and `test-sdks` accept `--doinstall-tools PATH`, which takes precedence
over `DOINSTALL_TOOL_DIR`. Both require a prebuilt tool pair and fail when it is
absent. If developing against source, explicitly run
`build.cmd _/doinstall/build --stage-dir C:/Tools/doinstall` first. CI performs
that preparation as a separate step from verification and packaging.

Unix uses `./build.sh`. Both launchers forward arbitrary arguments to a temporary
native runner; no task whitelist is maintained. The Rust source SDK is available
to bootstrapped Rust tasks. The launcher prepares the C# helper through
`build-sdks` only when a C# task is executed. `build` calls `build-sdks` explicitly
for a distributable native app. Doinstall owns installer Rust tests, formatting,
clippy, and shim verification. Rust 1.95+, rustfmt, and clippy are required for
source verification. Native Linux/macOS acceptance remains pending.

## Installer library contract

`InstallerArtifact` contains an absolute `FilePath`, `Kind`, `OS`, `Architecture`,
and optional `DefaultArguments`. `BuildContext.SetInstallerResultAsync` resolves
relative artifact paths against the project root. The standalone
`InstallerRunner` requires an absolute path. The artifact must exist and match
the host OS and OS architecture; unknown kinds and invalid arguments fail.

| Kind | Launch behavior |
| --- | --- |
| `Executable` | Execute the artifact; Windows requires `.exe`, Unix requires execute permissions |
| `Msi` | Windows system `msiexec.exe /i <artifact>`; `.msi` required |
| `ShellScript` | Linux/macOS `/bin/sh <artifact>`; script must be POSIX sh compatible |
| `DotNetAssembly` | `dotnet <artifact>`; suitable runtime must be available |

Package formats without a built-in kind require a project-authored installer
wrapper. There is no universal package generator, privilege escalation, desktop
shortcut generation, or shell command inference in the shared task.

- `CreateInstallerAsync(target = "create-installer", parameters = null, cancellationToken)`
  builds and returns the artifact using normal target resolution and parameter binding.
- `SetInstallerResultAsync(artifact, cancellationToken)` returns one artifact.
- `RunInstallerAsync(artifact, arguments = null, cancellationToken)` runs it;
  null uses defaults, while an explicit list replaces them.
- `InstallerRunner.ParseArguments(json)` decodes argument tokens for task options.
- `InstallerRunner.RunAsync` also works outside an ambient task context.

Launches use the artifact’s parent as the working directory and wait for the
launched process. Windows executable installers use shell activation, honoring
the installer’s elevation manifest without forcing `runas`; UAC cancellation is a
launch failure. Other kinds inherit the environment and standard streams. Nonzero exit codes
propagate as failures, including installer cancellation. MSI 1641 and 3010 are
successful restart-required outcomes and are reported explicitly. Task cancellation
attempts to stop the process tree; an elevated Windows installer may remain
running if access is denied, which is reported. Installer rollback is the
installer’s responsibility.
The shared task reports the observed exit code, not independently verified
installation success. An installer that detaches must supply a wrapper which
waits for completion if that guarantee is needed.

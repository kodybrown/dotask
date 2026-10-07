# Application installers

`dotask install` creates and runs a fresh installer for console and GUI applications.
The shared tasks live under `_/simple-installer/`; their Rust implementation does not
restrict application payloads to .NET. Application builds remain project-specific.
Custom `create-installer` targets and other installer technologies remain supported.

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
  - run: _/simple-installer/assemble
    with:
      builder: tools/simple-installer-builder.exe
      installer: tools/dotask-installer.exe
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

Install the Rust group using the tool:

```sh
dotask --add "_/simple-installer/*"
```

Review conflicts rather than deleting locally modified tasks. Remove obsolete
tracked installer tasks explicitly so short names do not become ambiguous.
An exact project `install` can wrap `_/simple-installer/install`. Updating the CLI
alone does not update committed task copies. Until published, use the
[local catalog override](SHARED-TASKS.md#official-catalog-and-unpublished-preview).

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
assembly belongs to the separate `simple-installer-builder` executable, which is
not distributed. The shipped installer has no `package` command.

ZIP and 7z creation are ordinary optional `build-steps` using
`_/archive/create-zip` and `_/archive/create-7z`. Each uses a configurable `7z`
executable and includes the installer directory as the archive's top-level
folder. Enable one or both, or substitute a project task for another format.
Archive flags do not belong to the destination installer.

Supply binaries matching the host OS/architecture; cross-platform release
production is not automated. Native acceptance is recorded separately in
[VERIFICATION.md](VERIFICATION.md).

## Installer YAML schema 1

This example packages an already-built Windows application. Use `linux` or `macos`
and appropriate executable paths on those systems. Architectures are `x64` and
`arm64`. Paths inside the payload use `/` and must not escape the payload.

```yaml
schema: 1
application:
  id: example
  name: Example App
  version: 1.2.0
  build:
    stamp: 26279-0612       # UTC YYDDD-HHMM; supplied by the application build
    commit: abcdef71234567890123456789012345678901234  # optional
    dirty: false
  author: Example Author
  copyright: Copyright (C) 2026 Example Author
  description: A desktop application.
platform: windows
architecture: x64
payload: ./published
runtime:
  interactive: true
  commands:
    - name: example
      executable: example.exe
  shortcuts:
    - name: '${shortcut-name}'
      executable: example.exe
      arguments: []
      terminal: false
      desktop: true
      start-menu: true
      local: true
  inputs:
    channel:
      type: choice
      choices: [stable, preview]
      required: true
      default: stable
      prompt: Choose a channel
  defaults:
    common:
      additional-command: false
      confirm-install: true
      desktop-shortcuts: false
      local-shortcuts: true
    windows:
      add-to-path: true
      start-menu-shortcuts: true
      start-menu-nested: false
      shortcut-name: Example App
  profiles:
    omarchy:
      values:
        desktop-shortcuts: false
  values: {}
  settings:
    - '${local-app-data}/Example/settings'
```

The engine finds same-basename YAML beside itself, independent of working directory.
`--config FILE` overrides that path. Payload paths resolve relative to the YAML;
packaging rewrites the payload reference to its packaged directory or ZIP.
ZIP traversal, duplicate paths, and links are rejected. Links inside directory
payloads are also rejected. Executable permissions are preserved on Unix.
`installer.yaml` and `installation.yaml` are reserved at each payload's root.

Inputs support `string`, `path`, `boolean`, and `choice`, with `required`, `prompt`,
`default`, and `choices`. Values are resolved in this order, highest first:

1. CLI overrides (`--set NAME=VALUE`).
2. Explicit YAML `values`.
3. Selected profile `values`.
4. OS `defaults`.
5. `defaults.common`, input defaults, then built-in location defaults.

Profiles are selected by `--profile NAME`, or optional `detect` strings matching
`XDG_CURRENT_DESKTOP` components. Multiple matches fail and require an explicit
profile. Omarchy can be selected explicitly; distribution detection is not inferred
from a desktop identifier. Unknown fields/inputs and invalid types/choices fail.
Templates support `${home}`, `${local-app-data}`, `${config-home}`, and named inputs.
Unresolved references fail. Built-in path inputs must resolve to absolute paths.

## Unattended and interactive execution

The installer is console-only. Install and uninstall default to prompting;
`interactive: false` in `installer.yaml` selects unattended execution.
`--interactive` and `--non-interactive` override that setting and are mutually
exclusive. Interactive execution requires a terminal; redirected execution fails
with guidance to use `--non-interactive`, rather than silently installing.
Package creation, help, and `--validate` never prompt. Missing required values,
invalid configuration, and ownership conflicts stop with a diagnostic and nonzero
exit code. Interactive answers replace defaults; CLI-supplied inputs skip their
individual prompts. Automation does not require a second confirmation flag.

Interactive output begins with an application banner, optional copyright and
description, then indented prompts. `Installation Settings` separates the chosen
locations from the final confirmation. `Installation Summary` precedes results,
errors, or cancellation; errors use a separate label and indented detail lines.
Uninstall uses the corresponding Uninstallation sections. Separator lines span
the current visible console width minus one column, rechecking width after
resizes. Unicode display widths and narrow windows are respected; unavailable
width information falls back to 80 columns. Unattended output remains plain.

The install prompts ask for the complete application directory, an optional
additional command (default No), its directory when enabled, and on Windows
whether to add the selected command directory to the user PATH (default Yes).
The additional directory is derived from the chosen install directory's parent,
including interactive changes, unless YAML or `--bin-dir` supplies another path.
If no additional command is selected, the application directory is the PATH
candidate. A directory already on persistent user/system PATH produces an
informational message and skips the PATH question and write. A final location
summary precedes the install confirmation. Boolean answers accept Y/N, yes/no,
and true/false; Enter accepts the shown default. Ctrl+C, end of input, or declining
confirmation prints `Canceled` and exits 130. Cancellation before confirmation
creates no installation. Interrupted activation retains its recovery journal.

```sh
./installer --non-interactive --install-dir /tmp/example/apps/example --bin-dir /tmp/example/bin
./installer --interactive
./installer --non-interactive --set desktop-shortcuts=false --prune-old-versions
./installer --validate
./installer uninstall --install-dir /tmp/example/apps/example
```

Built-in inputs are `install-dir`, `bin-dir`, `desktop-dir`, `start-menu-dir`,
`desktop-shortcuts`, `start-menu-shortcuts`, `local-shortcuts`, and
`prune-old-versions`, plus `additional-command`, `add-to-path`, and
`confirm-install`, `shortcut-name`, and `start-menu-nested`. These defaults and their prompt labels can be overridden using
`defaults`, `values`, and `inputs` in installer YAML. `confirm-install` controls
the final confirmation's default, not unattended execution. Boolean flags enable
the corresponding option; use
`--set NAME=false` to disable it. Shortcut and pruning defaults are false unless
YAML changes them. `--validate` checks package/configuration without installing.
Failures return 1, cancellation returns 130, and success returns 0. Detailed errors identify failed operations;
output can be captured by the invoking automation. There is no separate log-file
or dry-run installation-plan interface yet.

The install task's `--installer-args` is a JSON string array. It replaces the
artifact's default argument tokens; `[]` clears them. It does not forward build
options implicitly. Configure creator defaults on the creator's full target name.

```sh
dotask install --installer-args '["--non-interactive", "--set", "add-to-path=false"]'
```

## Locations, launchers, and version retention

### Package and installed build names

Versions belong to the application being packaged, never the installer engine
or dotask tool running the creator. `application.version` is an arbitrary
nonempty version string: SemVer, calendar versions, and custom labels are
supported. Common filename-safe characters, including SemVer `+` metadata, stay
readable; unsafe characters and literal percent signs are percent-encoded only
in directory names. The original version remains in metadata and display output.
Encoded version/build components must fit the filesystem-safe name limits.

Supply `application.build.stamp` as UTC `YYDDD-HHMM` to match the application's
embedded build information. Git `commit` is optional; when supplied it is stored
in full and displayed in names using its first seven characters. `dirty: true`
adds `-dirty` and requires a commit. When package creation has no build metadata,
the engine captures a packaging minute without a Git revision and saves it in
the package. Installation always uses that saved stamp. Direct installation of
an un-packaged config without build metadata retains a content-addressed identity.

`package --output-parent DIRECTORY` creates a directory named
`<app-id>-<app-version>-<stamp>[-<git7>][-dirty]`, with the engine, YAML, and payload
directly inside. The installed build under `app/` uses the same name without the
`<app-id>-` prefix. `--output DIRECTORY` remains an explicit complete package-path
override. `--result-file FILE` writes the absolute installer `FilePath` as JSON;
creator tasks read that result instead of parsing console output.

Identical named packages can be reused. Matching names with different payload or
configuration are rejected without overwriting the package or installed build.
The internal full-content fingerprint remains an integrity check; no fingerprint
is appended to build-metadata-based names. Previously owned build directories
retain their names and receipts; no directory renaming or adoption is performed.

Default application roots are `%LOCALAPPDATA%/Programs/<id>` on Windows and
`~/.local/lib/<id>` on Linux/macOS. `--install-dir` selects the complete application
root, not its parent. A stable command is always created inside that root. An
additional command is optional (`additional-command: false` by default); its
suggested directory is the application root's parent. `BIN` is not consulted.
`--bin-dir` enables the additional command unless `--set additional-command=false`
explicitly disables it. Its location must be outside the application root;
an ancestor such as `%LOCALAPPDATA%/Programs` is permitted. Updates may enable,
disable, or move the additional command. The receipt's launcher inventory owns
the previous files independently of saved preference values. The installer locks
the previous and requested directories, verifies existing owned files, creates
the selected launchers, and removes only obsolete owned launchers. An unowned new
destination or modified old launcher stops the update before activation. A pending
activation must be retried with the same package and launcher destinations.

On Windows, `add-to-path` defaults to true and applies to the additional command
directory when enabled, or the application directory otherwise. The installer
appends only a missing directory to persistent **user** PATH, preserves existing
entries and the registry string type, and notifies Windows of the change. Existing
terminals keep their environment; reopen them to use the new command. Uninstall
removes only owned shims/sidecars and retains PATH entries, including shared parent
directories that may contain other applications' commands. Automatic PATH changes
are unavailable on Linux/macOS; add the selected directory to your shell's PATH.
Explicit directory aliases are resolved before operating; managed child links
and junctions are rejected.

```text
example/
  installer/
    installer.exe
    installer.yaml
    installation.yaml
  app/
    1.2.0-<build-id>/
      installer.yaml
      installation.yaml
      <application files>
  example.exe
  example.shim
  Example App.lnk          # when local shortcuts are enabled
```

Unix root commands are symlinks. Windows root commands use the existing native
shim plus a `.shim` sidecar. These provide a stable launch path while the real
executable remains in its build directory. External commands/shortcuts point
directly to that same build, not through the root launcher. Installation refreshes
both sets. No fixed-location payload mode is implemented.

Each build has a configuration snapshot and immutable inventory receipt. The
management receipt records actual locations, active/previous builds, launcher
ownership, settings locations, and hashes. Build identity includes payload files,
Unix permissions, and package configuration; timestamps are not identity.
Repeating an identical install does not change the previously active build.

`--prune-old-versions` prunes after successful activation, keeping the new active
build and the last active build. Modified or unsafe builds are retained with a
diagnostic. Older builds otherwise remain. Rollback and `--set-version` are deferred:
retention does not imply data/settings downgrade compatibility.

## Shortcuts

Console and GUI applications can both declare shortcuts. Each shortcut's
`desktop`, `start-menu`, and `local` booleans permit those locations; corresponding
input values select whether to create them. Requesting a location with no allowed
shortcut fails. Optional `icon` and `working-directory` are payload-relative.

Declare `name: '${shortcut-name}'` to offer a configurable shortcut name, which
defaults to `application.name`. Static names remain fixed; a package may declare
multiple distinct shortcuts. `--shortcut-name NAME` or `--set shortcut-name=NAME`
overrides the named input. Names omit the extension and may contain spaces or
Unicode. Empty/oversized names, control characters, path separators, invalid
Windows characters, reserved device names, and trailing dots/spaces are rejected.
An invalid interactive name reports the error and asks again. Template-expanded
names are validated before a shortcut is staged or installed.

On Windows, enabling a Start Menu shortcut prompts for its configurable name and
whether to nest it in a folder of the same name (default No). Set
`start-menu-nested: true` in Windows defaults, use `--start-menu-nested`, or pass
`--set start-menu-nested=true` to select `name/name.lnk`; otherwise the location
is `name.lnk`. The base defaults to the current user's Start Menu Programs folder.
Desktop/local shortcuts remain flat. Updates may rename shortcuts or change
their nesting; obsolete owned links are removed. Receipts record menu folders
created by the installer, which are removed only when empty. Preexisting folders
and folders containing unrelated files are preserved.

Linux application-menu `.desktop` entries remain flat. Windows nesting is rejected
on other platforms, since a desktop-file subdirectory does not itself define a
visible Linux submenu. Linux/macOS acceptance remains deferred. Dotask's own
console package declares no shortcuts, so it offers no Start Menu question or
entry; the shortcut options are for applications packaged by this engine.

Windows uses native `.lnk` files and application entry points determine console
behavior. Linux uses `.desktop` files with `Terminal` metadata; desktop visibility
and trust still depend on the desktop environment. Defaults use `~/Desktop` and
`$XDG_DATA_HOME/applications` (falling back to `~/.local/share/applications`);
localized/custom desktop locations can use `desktop-dir`.
macOS GUI aliases target app bundles and use their launch defaults/icon; aliases
with custom arguments, working directories, or icons are rejected. Terminal
shortcuts use `.command` files with explicit arguments and working directories.
macOS has no Start menu: requesting it fails. Desktop aliases may require Finder
automation permission; native macOS acceptance remains pending.

## Uninstall, settings, and ownership

Run the retained executable, or use the shared task without rebuilding:

```sh
/path/to/example/installer/installer uninstall
dotask uninstall --install-dir /path/to/example
```

On Windows, invoke `installer.exe`. Uninstall uses installed build snapshots and
ownership receipts; the original payload/package is unnecessary. Settings are
preserved by default. `--leave-settings` makes that explicit; `--remove-settings`
removes declared application-owned settings paths. They are mutually exclusive.
The shared task exposes `--remove-settings` and `--non-interactive`; otherwise
the retained installer's YAML controls prompt mode.
Application documents and unrelated files must never be declared as settings.
Settings remain in their ordinary locations, not linked into version directories.

Unowned destinations are never adopted. Modified owned launchers/builds block
uninstall before deletion; resolve the diagnostic rather than deleting receipts.
Unrecorded application files are preserved and reported. On Windows, close running
applications before uninstall; locked payloads fail preflight. The engine does not
force-close applications or manage services. A running Windows uninstaller may
move itself to a printed temporary directory, which can be deleted after it exits.

Operations are locked by installation and command directory. Activation keeps a
`pending.yaml` record for forward completion: retry an interrupted installation
with the same package and locations. Unknown modifications still fail. Uninstall
records progress for retry after partial removal. This is not user-requested
rollback, nor a guarantee that arbitrary failures restore the previous state.

Only the current YAML installation format is supported. There is no import,
migration, or backward compatibility with older installation formats. A directory
without a valid YAML receipt is never adopted, even if it contains dotask files.
Use a fresh installation root and unused command destinations, or manually remove
the previous installation and its launchers before using those locations again.
The new installer does not uninstall installations created by the previous engine.
There is no Windows Installed Apps registration or Inno Setup dependency.

## Build and install dotask itself

The local Rust `create-installer` task delegates to
`_/simple-installer/create-installer`. Its `.tasks/installer.yaml` build steps
run `build-installer` to prepare the installer and build-machine builder, run
`build` to prepare the native app and SDKs, gather the declared files, and call
`_/simple-installer/assemble`. Optional archive steps follow. There is no local
`pack` task; `_/dotnet/pack` remains a NuGet task for consuming C# projects.

`build-sdks` publishes the maintained C# helper using evaluated MSBuild
`PublishDir`, then copies the Rust SDK selections from `.tasks/sdks.yaml`.
Compiler output stays external. Only the final independent installer directory
is created beneath `settings.installer.output`. Destination machines need
Rust/Cargo to execute Rust tasks and the .NET SDK to execute C# tasks; neither is
needed for metadata, help, initialization, completion, or installer execution.
No language runtime, SDK, Roslyn, or managed support host is bundled.

```powershell
.\build.cmd                         # Bootstrap and list available tasks.
.\build.cmd verify                  # Explicit repository verification.
.\build.cmd create-installer
# Use fresh, isolated locations for acceptance:
.\build.cmd install --installer-args '["--non-interactive","--set","add-to-path=false","--install-dir","C:/Temp/dotask-acceptance/apps/dotask","--bin-dir","C:/Temp/dotask-acceptance/bin"]'
```

Unix uses `./build.sh`. Both launchers forward arbitrary arguments to a temporary
native runner; no task whitelist is maintained. The Rust source SDK is available
to bootstrapped Rust tasks. The launcher prepares the C# helper through
`build-sdks` only when a C# task is executed. `build` calls `build-sdks` explicitly
for a distributable native app. `build-installer --verify` runs installer Rust
tests, formatting, and clippy. Rust 1.95+, rustfmt, and clippy are required for
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

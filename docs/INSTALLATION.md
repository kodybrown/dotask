# Application installers

`dotask install` creates and runs a fresh installer for console and GUI applications.
The shared tasks live under `_/dotask-installer/`; their C# implementation does not
restrict application payloads to .NET. Application builds remain project-specific.
Custom `create-installer` targets and other installer technologies remain supported.

## Shared tasks

Use the tool to install the new group and remove the old tracked task:

```sh
dotask --add "_/dotask-installer/*"
dotask --remove _/dotnet/install
```

Review conflicts rather than deleting locally modified tasks. Do not retain both
install tasks: their short name would be ambiguous. An exact project `install`
can wrap `_/dotask-installer/install` explicitly. Existing projects should sync
tracked copies with `dotask --sync "_/dotask-installer/*"`.
Until published, use the [local catalog override](SHARED-TASKS.md#official-catalog-and-unpublished-preview).
Updating the CLI alone does not update committed task copies.

The shared creator reads these settings, with matching CLI overrides:

| Setting | Override | Meaning |
| --- | --- | --- |
| `installer-config` | `--config` | Installer YAML file |
| `installer-engine` | `--engine` | Prebuilt host Rust installer executable |
| `installer-output` | `--output` | Parent directory for complete packages |

Paths in task settings/options are project-relative or absolute. Build the payload
before calling the creator. The creator never invokes an application compiler.
A project wrapper can call `CreateInstallerAsync("_/dotask-installer/create-installer",
parameters)` and forward the artifact using `SetInstallerResultAsync`.
A YAML task group can wrap the creator too. Packages are independently copied into
`<output>/<os>-<architecture>/<unique-id>/`, and contain:

```text
installer.exe             # installer on Linux/macOS
installer.yaml
payload/                  # or payload.zip
```

Distribute the entire directory. There is one installer executable, without a
companion .NET runtime. The destination does not need dotask, Rust, or an SDK.
Application runtime requirements still depend on the payload. Supply a binary
matching the host OS/architecture; cross-platform engine release distribution is
not yet automated. Windows builds statically link the CRT; Unix system-library
requirements depend on the build host/target. Native acceptance is recorded in
[VERIFICATION.md](VERIFICATION.md), separately from implementation support.

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
  author: Example Author
  description: A desktop application.
platform: windows
architecture: x64
payload: ./published
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

The Rust `create-installer.rs` task builds the native CLI and installer engine,
publishes the C# helper library using evaluated MSBuild `PublishDir`, and packages
a fresh payload containing `dotask[.exe]`, `Dotask.dotnet.dll`, and `sdk/`. It invokes the
existing Rust installer directly and returns the normal `InstallerArtifact`.
The Rust `pack` task uses the same creator. Compiler output remains external;
final packages go beneath `settings.installer-output`.
Rust 1.95 or newer, Cargo, rustfmt, and clippy are required for source verification.
The required gate includes Rust tests and the .NET suite. Destination machines
need Rust/Cargo only to execute Rust tasks and the .NET SDK only to execute C#
tasks. Neither toolchain is needed for task metadata, CLI help, initialization,
or completion. No language runtime, SDK, Roslyn or support host is bundled.

```powershell
.\build.cmd
.\build.cmd create-installer
# Use fresh, isolated locations for acceptance:
.\build.cmd install --installer-args '["--non-interactive","--set","add-to-path=false","--install-dir","C:/Temp/dotask-acceptance/apps/dotask","--bin-dir","C:/Temp/dotask-acceptance/bin"]'
```

Unix uses `./build.sh` and absolute temporary paths. All packages use the user's
installed language toolchains; there is no self-contained packaging option.
`build.cmd installer-engine` builds only
the host engine. `build.cmd installer-engine --verify` runs Rust checks.

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

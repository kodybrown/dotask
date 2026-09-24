# Application installers

`dotask install` creates and runs a fresh installer for console and GUI applications.
The shared tasks live under `_/dotask-installer/`; their C# implementation does not
restrict application payloads to .NET. Application builds remain project-specific.
Custom `create-installer` targets and other installer technologies remain supported.

## Shared tasks and migration

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
commands:
  - name: example
    executable: example.exe
shortcuts:
  - name: Example App
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
    desktop-shortcuts: false
    local-shortcuts: true
  windows:
    start-menu-shortcuts: true
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

The installer never prompts unless `--interactive` is specified, which requires a
terminal. Missing required values, invalid configuration, and ownership conflicts
stop with a diagnostic and nonzero exit code. Interactive answers replace defaults;
CLI-supplied inputs skip their prompts. Automation does not require a second
confirmation flag after explicitly requesting install or uninstall.

```sh
./installer --install-dir /tmp/example/apps/example --bin-dir /tmp/example/bin
./installer --interactive
./installer --set desktop-shortcuts=false --prune-old-versions
./installer --validate
./installer uninstall --install-dir /tmp/example/apps/example
```

Built-in inputs are `install-dir`, `bin-dir`, `desktop-dir`, `start-menu-dir`,
`desktop-shortcuts`, `start-menu-shortcuts`, `local-shortcuts`, and
`prune-old-versions`. Boolean flags enable the corresponding option; use
`--set NAME=false` to disable it. Shortcut and pruning defaults are false unless
YAML changes them. `--validate` checks package/configuration without installing.
Failures return 1; success returns 0. Detailed errors identify failed operations;
output can be captured by the invoking automation. There is no separate log-file
or dry-run installation-plan interface yet.

The install task's `--installer-args` is a JSON string array. It replaces the
artifact's default argument tokens; `[]` clears them. It does not forward build
options implicitly. Configure creator defaults on the creator's full target name.

```sh
dotask install --installer-args '["--interactive"]'
```

## Locations, launchers, and version retention

Default application roots are `%LOCALAPPDATA%/Programs/<id>` on Windows and
`~/.local/lib/<id>` on Linux/macOS. `--install-dir` selects the complete application
root, not its parent. Command directories default to `%LOCALAPPDATA%/bin` or
`~/.local/bin`; a nonempty `BIN` overrides that default. PATH is not modified.
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
The shared task exposes `--remove-settings` and `--interactive`.
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

To migrate a recognized older `.dotask-install.json` installation, retain its
original root/bin locations and explicitly pass `--migrate-legacy`. The engine
verifies legacy command ownership and build inventories, imports build snapshots,
and tracks original owned files for uninstall. Unrelated installers are never
adopted. Legacy pending activation must first be recovered by the old installer.
There is no Windows Installed Apps registration or Inno Setup dependency.

## Build and install dotask itself

The source creator publishes dotask using evaluated MSBuild `PublishDir`, builds
the Rust engine, and delegates packaging to the shared creator. Compiler output
remains external. Final packages go beneath `settings.installer-output`.
Rust 1.95 or newer, Cargo, rustfmt, and clippy are required for source verification.
The required gate includes Rust tests and the .NET suite; destination machines do
not need these tools.

```powershell
.\build.cmd
.\build.cmd create-installer
.\build.cmd create-installer --self-contained=false
# Use fresh, isolated locations for acceptance:
.\build.cmd install --installer-args '["--install-dir","C:/Temp/dotask-acceptance/apps/dotask","--bin-dir","C:/Temp/dotask-acceptance/bin"]'
```

Unix uses `./build.sh` and absolute temporary paths. `--self-contained=false`
requires the application's .NET runtime on the destination; it never changes the
Rust installer's runtime requirements. `build.cmd installer-engine` builds only
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

## Low-level installation engine

This API remains available for installer authors and compatibility. The shared
install task never calls it as a fallback. The retained .NET compatibility installer uses
it, retaining the same ownership and recovery format as older dotask installations.
The application files must already be published:

```csharp
var project = BuildContext.Current;
var result = await project.InstallAsync(new InstallationDefinition
{
    AppId = "example",
    Version = "1.2.3",
    SourceDirectory = publishDirectory,
    Commands = [new InstalledCommand("example", project.IsWindows ? "example.exe" : "example")],
    BinDirectory = project.Parameters.Contains("bin-dir") ? project.Parameters.GetPath("bin-dir") : null
});
```

`InstallationDefinition` also has optional `InstallRoot`. Paths in `InstalledCommand`
are portable relative paths inside `SourceDirectory`; command names omit `.exe`.
Multiple commands can point at the same executable. `AppId`, version and command
names are validated as portable path components. Windows entry points must be
`.exe` files; Unix entry points must have executable permissions. The generic
installer does not certify arbitrary binaries' OS/ABI compatibility.

`InstallationResult` returns `InstallDirectory`, `BinDirectory`, the full
`Fingerprint`, `Reused`, and `Warnings` about command lookup. It does not print by
itself. The task decides how to present the result.

`BuildContext.InstallAsync(definition, cancellationToken)` roots explicit relative
directories at the project root and links cancellation to the task lifetime.
`UserInstaller.InstallAsync(definition, cancellationToken)` is also available
without an ambient context; relative paths then use the process working directory.

Other applications migrating from the old shared task must explicitly choose how
their installer handles existing `.dotask-install.json` ownership. An MSI or other
installer does not automatically understand those records. Do not overwrite or
adopt such installations merely because the destination matches. There is no
automatic cross-installer migration or removal in the shared task.

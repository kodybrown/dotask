# Application installers

`dotask install` creates and runs the project's installer for the current OS and
architecture. Console and GUI applications follow the same workflow. There is no
OutputType check, direct-copy fallback, installer discovery, or freshness guess.
The project installer owns its application files, updates, shortcuts, and removal.
Windows, Linux, and macOS have implementation support; see
[verification status](VERIFICATION.md) for native acceptance.

## Reuse the .NET task

Add `dotnet/install` from the configured source, or update an existing tracked copy:

```sh
dotask --sync _/dotnet/install
```

Use the documented [local/private source workflow](SHARED-TASKS.md) when the newer
catalog is not published. Updating the CLI alone does not update project tasks.
The new task requires a library version exposing the installer APIs below.

Create a project-owned `.tasks/create-installer.cs`. It builds an installer using
your packaging technology, then calls `SetInstallerResultAsync` exactly once.
It must not launch the installer. For example, after your build has produced an
executable installer:

```csharp
await project.SetInstallerResultAsync(new InstallerArtifact {
    FilePath = installerPath,
    Kind = InstallerKind.Executable,
    OS = project.OS,
    Architecture = project.Architecture,
    DefaultArguments = []
});
```

Use the artifact's actual platform metadata, not the host values if your build
cross-compiles. This installation contract rejects non-host artifacts. An empty
argument list means the installer provides its normal interactive/default behavior;
the library never adds silent, elevation, or scope flags.

The shared task resolves `create-installer` through the normal catalog, executes
it on every invocation, reads its structured result, validates it, and launches
it. Only genuine absence produces missing-target guidance; ambiguity, build
failure, invalid results, and cycles fail. The result is stored in a private,
invocation-scoped session and removed afterward. It cannot be reused by a later
invocation. A YAML group can wrap a producer, but must return exactly one artifact.
C# wrappers can use `CreateInstallerAsync("packaging/create-installer")` followed
by `SetInstallerResultAsync(artifact)` to forward a nested producer's result.

The single shared option is `--installer-args`: a JSON array of string tokens.
Explicit tokens replace `DefaultArguments`; they do not append. `[]` clears the
defaults. Spaces, quotes, empty strings, Unicode, and shell metacharacters remain
literal. Platform launch arguments, such as MSI's `/i` and the artifact path,
are not removed. Invalid JSON or token values fail before creating an installer.

```sh
dotask install
dotask install --installer-args '["--scope","user"]'
```

An exact `.tasks/install.cs` or `.tasks/install.task` overrides the short `install`
name. Delegate explicitly to `_/dotnet/install` to avoid recursion:

```csharp
await BuildContext.Current.ExecTargetAsync("_/dotnet/install",
    new Dictionary<string, string> { ["installer-args"] = "[\"--scope\",\"user\"]" });
```

A future `deploy` task can own deployment to a target environment. `release` has
no special installation meaning. Existing publishing/packaging tasks are not
renamed automatically; `dotnet publish` remains a build primitive.

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

## Install dotask from its source

```sh
./build.sh install
```

On Windows, use `.\build.cmd install`. These launchers bootstrap the current
library before calling the shared task. Run `./build.sh` / `build.cmd` first for
the complete verification gate; installation does not run all tests.

Dotask's project-owned `create-installer` publishes the application and a standalone
`dotask-installer` executable in Release for the current x64/ARM64 host. Both are
self-contained by default. It respects evaluated MSBuild `PublishDir` values,
then copies the complete installer and its payload to `settings.installer-output`.
This setting is required by dotask's project-owned creator. The checkout sets:

```yaml
settings:
  installer-output: artifacts/installers
```

Relative paths start at the project root; absolute directories are also accepted.
Each run creates `<installer-output>/<os>-<architecture>/<unique-id>/`, preserving
previous packages. The printed path and returned installer artifact point to this
final copy, so `dotask install` runs it from there. The output cannot be inside
either publish directory, including through directory links. Build/intermediate/
publish paths still follow the MSBuild output policy; only the final deliverable
is copied into the project. `artifacts/` is ignored by Git. The next publish cannot
change an existing package. Distribute the entire printed directory, not just the executable.
Creating it does not install anything and requires no installed dotask or shim compiler.

```sh
./build.sh create-installer
./build.sh create-installer --self-contained=false
```

The framework-dependent variant requires .NET 10 on the destination. Configure
creator options in `.dotasks.yaml` under `targets.create-installer.defaults`;
the shared install task does not forward build options implicitly.

Dotask's standalone installer accepts `--install-root PATH` and `--bin-dir PATH`.
Use absolute paths for these arguments, since the working directory is the
installer package directory. To test without changing the active installation:

```sh
./build.sh install --installer-args '["--install-root","/tmp/dotask-test/apps","--bin-dir","/tmp/dotask-test/bin"]'
/tmp/dotask-test/bin/dotask --version
```

PowerShell equivalent (use an unused test directory):

```powershell
.\build.cmd install --installer-args '["--install-root","C:/Temp/dotask-test/apps","--bin-dir","C:/Temp/dotask-test/bin"]'
C:/Temp/dotask-test/bin/dotask.exe --version
```

The following ownership and directory rules describe **dotask's installer**, not
arbitrary project installers. It retains the old dotask installation identity and
receipt format, so existing owned installations are updated rather than duplicated.
Keep the original command directory and location overrides. Existing NuGet global
tools and unrelated installers are never adopted. Rollback/pruning/uninstall
commands remain deferred for dotask's installer.

## Locations and command lookup

| OS | Application root | Default command directory |
| --- | --- | --- |
| Windows | `%LOCALAPPDATA%\Programs` | `%LOCALAPPDATA%\bin` |
| Linux | `~/.local/lib` | `~/.local/bin` |
| macOS | `~/.local/lib` | `~/.local/bin` |

The macOS locations are dotask's CLI installation convention. The command
directory is selected in this order: explicit `--bin-dir`, nonempty `BIN`
environment variable, platform default. `--install-root` overrides the
application root; dotask appends the application ID and build directory.
Relative explicit paths start at the project root. An intentional directory
symlink such as `~/bin` is resolved before installation.

Windows commands consist of `<command>.exe` (the bundled generic shim) and
`<command>.shim` (a quoted absolute executable path). Linux/macOS commands are
file symlinks. Only the command directory belongs on PATH. The installer does not
edit PATH, profiles, registry settings, or machine-wide directories.

If it reports that the default command directory is missing from PATH:

- Bash: add `export PATH="$HOME/.local/bin:$PATH"` to `~/.bashrc`.
- Zsh: add that line to `~/.zshrc`.
- Fish: run `fish_add_path "$HOME/.local/bin"`.
- Windows: add `%LOCALAPPDATA%\bin` to your **user Path** in Environment
  Variables, then open a new terminal.

For `BIN` or `--bin-dir`, substitute the selected directory. Check resolution with
`command -v dotask` (Bash/Zsh), or `Get-Command dotask -All` (PowerShell).
Shell aliases/functions and command hashing may also affect resolution; use
`hash -r` in Bash after changing an installation.

## Existing global-tool installations

Installing this way does not uninstall a previous NuGet global tool, change a
`dt` shortcut, or take ownership of an existing command. The installer reports
other matching executables on PATH and in `~/.dotnet/tools`.

When migrating dotask, first install to a separate command directory, run that
new executable by its full path, and confirm it works. Then, if you want to remove
the old global tool, explicitly run:

```sh
dotnet tool uninstall --global dotask
```

Ensure the new command directory is on PATH. Do not point `--bin-dir` at the old
tool's directory expecting dotask to overwrite its launcher. Existing commands
are protected even if their contents happen to match.

## Versions and safe updates

An installation has this shape:

```text
<application-root>/<app-id>/
  .dotask-install.json
  1.2.3-0123456789abcdef/
    .dotask-build.json
    app executable and published files
  1.2.3-fedcba9876543210/
    .dotask-build.json
    app executable and published files
```

The directory suffix is the first 16 hexadecimal characters of a SHA-256
fingerprint. It covers the host OS/architecture and the sorted published file
paths, file hashes and Unix permissions. Full fingerprints and inventories are
recorded; a truncated collision fails safely. The application's declared version
is retained. A changed build with the same version gets another directory.
An unchanged build is checked and reused. Timestamps do not determine identity.

If a previously installed command is deleted, rerun the install task with the
original command directory. The installer recreates missing owned launchers,
including either missing member of a Windows `.exe`/`.shim` pair. This works
when reusing an identical build and when installing changed files; no force flag
or removal of installation records is needed. An existing launcher that was
changed or replaced (including a retargeted or dangling replacement symlink)
still causes a conflict. Missing launchers do not bypass build verification or
protection of other commands.

All published files are copied into staging, checked against the original
inventory, and moved into place before commands switch. Existing builds remain
untouched. Installations are serialized using `.dotask-install.lock` files in
the application root and command directory. These temporary files are removed
when their locks are released after success, failure, or handled cancellation.
Do not manually delete them while an installation is active. A forced process
termination can leave a file on Unix; it is unlocked and the next installation
can reuse and remove it. Leftover files from older dotask versions are cleaned up
the same way.

The installer refuses unowned application directories, occupied commands, modified
owned launchers, and modified builds that would otherwise be reused. Published
file/directory symlinks within the published output are rejected (the source root
itself is resolved like other explicit directories). Managed installation records and build
directories cannot be symlinks. Install output and command directories must not
overlap the publish output or each other within the application directory.

Activation uses a `.dotask-pending.json` recovery journal. A subsequent install
rolls an interrupted activation back before proceeding, provided affected files
still match their recorded old or new states. Manual edits stop recovery with an
error. A repair records the launcher's original absence separately from its
ownership; rollback restores that absence before installation retries the repair.
Preserve the journal and installation records when resolving a conflict;
do not delete them to bypass ownership checks. An abrupt process termination
can leave an unused `.staging-*` directory; it is never activated or automatically
adopted.

Multiple commands update individually; the operation is recoverable, but another
process could observe different active versions during a multi-command switch.
Reusing an existing application requires its original command directory.
Changing only an installed command's letter casing is rejected; retain its
original spelling when updating.
Dedicated relocation, rollback, pruning and uninstall commands are deferred.
Normal installation does not delete older builds.

## Low-level installation engine

This API remains available for installer authors and compatibility. The shared
install task never calls it as a fallback. Dotask's own generated installer uses
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

# Install applications for the current user

The reusable `dotnet/install.cs` task publishes a console application and passes
the output to DoTask's installation library. It does not require NuGet tool
packaging. Windows, Linux, and macOS are supported by the implementation; consult
[verification status](VERIFICATION.md) for the hosts actually exercised.

## Install dotask from its source

From the dotask checkout:

```sh
./build.sh install
```

On Windows:

```powershell
.\build.cmd install
```

These launchers build a temporary runner from the current source and execute the
install task. The task publishes Release output for the host OS/architecture,
self-contained by default. No installed dotask or shim compiler is required.
Run `./build.sh` / `build.cmd` first when you also want the full verification gate;
the install task itself publishes and installs without running all tests.

Once this installation API is present in your installed dotask, `dotask install`
works from the checkout too. An older installed dotask may not expose the new API:
use the bootstrap launcher for the first upgrade, or whenever updating the shim
binary while the installed Windows shim is running.

To exercise installation without changing your normal command:

```sh
./build.sh install --install-root ./artifacts/install-test/apps --bin-dir ./artifacts/install-test/bin
./artifacts/install-test/bin/dotask --version
```

The Windows equivalents use `.\build.cmd` and
`.\artifacts\install-test\bin\dotask.exe`.

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

## Reuse the .NET task

Add the shared task when the online catalog is published, or use the documented
[local/private source workflow](SHARED-TASKS.md) during development:

```sh
dotask --add dotnet/install
dotask help dotask-official/dotnet/install
dotask dotask-official/dotnet/install
```

The task is self-contained in `install.cs`, including its .NET publishing and
MSBuild metadata queries. Copy that one file if copying manually; there is no
separate `DotNetInstall.cs` helper. Configure `settings.project` in `.dotasks.yaml`
with the `.csproj` path.
The task supports console applications (`OutputType=Exe`) on x64 and ARM64 hosts.
Linux selects `linux-musl` when running on a musl .NET host; otherwise it selects
`linux`. Platform-specific acceptance is listed in the verification document.
It reads the evaluated `PublishDir` after publishing, respecting custom MSBuild
output policies. No `bin/Release` path is assumed or overridden.

| Option | Default / behavior |
| --- | --- |
| `--configuration`, `-c` | `Release`; accepts `Debug` or `Release` |
| `--self-contained` | `true`; includes the runtime |
| `--framework` | Required for multi-target projects without a selected framework |
| `--dotnet` | `dotnet` executable |
| `--bin-dir` | Explicit command directory override |
| `--install-root` | Application root override |
| `--app-id` | Evaluated `AssemblyName` |
| `--command` | Evaluated `ToolCommandName`, otherwise `AssemblyName` |

Use `--self-contained=false` for a framework-dependent application. Installing a
self-contained dotask does **not** eliminate its SDK requirement: dotask still
compiles C# tasks. This task does not create desktop shortcuts, install services,
register a system package, or implement Windows Apps & Features registration.

## Library contract

The application files must already be published. C# tasks call:

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

The library owns installation and command activation. The shared .NET task owns
publishing and project metadata. The shim only launches an executable. The
definition is serializable, but no public JSON installation CLI or non-C# task
runtime is implemented yet. See the [shim source and build instructions](../src/Dotask.Shim/README.md).

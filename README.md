# dotask

Portable project tasks written as individual C# files. Put targets in `.tasks/`,
describe them with XML documentation, and keep project-specific settings in
`.dotasks.yaml`. Copy the same target file between projects without a
registration step.

```text
.dotasks.yaml
.dotasks-lock.yaml
.tasks/
  build.cs
  dotask-official/dotnet/build.cs
  dotask-official/dotnet/run.cs
  release.cs
```

This is a **0.1.0 local preview**, with no public package release yet. Windows,
Linux, and macOS are the intended hosts; Linux has been tested locally and the
other two still need acceptance. See [verification status](docs/VERIFICATION.md).

Use **.NET SDK 10.0.300 or later in the .NET 10 family**. The SDK compiles targets;
a runtime-only installation is insufficient. Run `dotnet --version` from the
project directory to check the selected SDK. The project's `global.json` can
select an older SDK even if a suitable one is installed.

## Start here

- **Build dotask:** [build and verify this checkout](#build-and-verify).
- **Already ran the build?** [Install dotask for your user account](#install-dotask).
- **Try it without installing:** [run the included example](#try-the-example-without-installing).
- **Use it in a project:** [commands, discovery, completion, and troubleshooting](docs/USAGE.md).
- **Write or reuse tasks:** [complete example and authoring reference](docs/TARGETS.md).
- **Use an AI assistant:** [task workflow, API rules, and reusable agent instructions](docs/AI-ASSISTANTS.md).
- **Work on dotask itself:** read [AGENTS.md](AGENTS.md), then [development](#development).

## Build and verify

From the repository checkout, bootstrap and verify the project:

```sh
# Linux or macOS
./build.sh
```

```powershell
# Windows (PowerShell or cmd.exe)
.\build.cmd
```

Only the .NET SDK and Git are required; dotask does not need to be installed.
The launchers compile this checkout, stage a temporary runner, and use it to run
the repository's `verify` task in Release. That task checks prerequisites, builds
and tests the solution, and checks formatting, documentation, and the shared catalog.
The temporary runner is removed when the launcher finishes normally or reports a failure.

**After a successful build, install dotask using the next section.** Building
does not install the command, update an existing installation, or add anything
to PATH. If you only want to try the example, you can
[continue without installing](#try-the-example-without-installing).

## Install dotask

To use `dotask` from any project, install it for your current user account.
From this checkout after the build succeeds:

**Linux/macOS (Bash or Zsh):**

```sh
./build.sh install
```

**Windows (PowerShell):**

```powershell
.\build.cmd install
```

The task publishes a self-contained Release application for your host and
installs it without administrator access. Each version/build gets a separate
directory; older builds are retained. Windows gets a small `.exe` launcher and
`.shim` file; Linux/macOS get a symlink. No shim compiler is needed.

The command directory is `--bin-dir` when provided, otherwise the `BIN`
environment variable, otherwise the platform default below. If it is not already
on PATH, add **that one directory** once:

- **Bash:** add `export PATH="$HOME/.local/bin:$PATH"` to `~/.bashrc`.
- **Zsh:** add that line to `~/.zshrc`.
- **Fish:** run `fish_add_path "$HOME/.local/bin"`.
- **Windows:** add `%LOCALAPPDATA%\bin` to your **user Path** in Environment
  Variables, then open a new terminal.

For a custom command directory, substitute its path. The installer prints the
selected locations and PATH diagnostics; it never edits PATH or shell profiles.
Then run `dotask --version` (expected: `dotask 0.1.0`).

The installation is independent of this source checkout. To check which command
your shell resolves, use `command -v dotask` on Bash/Zsh or `Get-Command dotask -All`
on PowerShell. An existing global tool or `dt` shortcut is not changed. See
[installation and migration](docs/INSTALLATION.md) for directories, overrides,
safe updates, and moving from the earlier NuGet global-tool installation.

### Try the installed command

While still in this checkout, run the included example:

```text
dotask --use-dir examples/basic/.tasks hello -n Ada -c Release
```

It should print `Hello from dotask, Ada!` and `configuration: Release`. You can
then change to another project containing its own `.dotasks.yaml` and `.tasks/`,
run `dotask` to see its tasks, and run `dotask TARGET` to execute one. Installing
the CLI does not add task files to other projects. See
[project setup and task authoring](docs/TARGETS.md#a-complete-target) and
[shared tasks](docs/SHARED-TASKS.md) for that next step.

### Replace an existing installation after source changes

Run the installation task again after changing source:

```sh
./build.sh install
```

On Windows, use `.\build.cmd install`. Use the same directory overrides as your
first installation. A content fingerprint distinguishes builds with the same
version, so no uninstall is necessary. The command switches after copying and
validation succeed. An unchanged build is checked and reused.

## Try the example without installing

Pass any dotask command after the launcher. For the basic example on Linux/macOS
(replace `./build.sh` with `.\build.cmd` on Windows):

```sh
./build.sh --use-dir examples/basic/.tasks
./build.sh --use-dir examples/basic/.tasks hello -n Ada -c Release
./build.sh --use-dir examples/basic/.tasks write
```

To build and run the CLI directly with the SDK, these commands also work in
Bash, Zsh, Fish, and PowerShell:

```text
dotnet build dotask.slnx -c Release
dotnet run --project src/Dotask.Cli -c Release --no-build -- --use-dir examples/basic/.tasks
dotnet run --project src/Dotask.Cli -c Release --no-build -- --use-dir examples/basic/.tasks hello -n Ada -c Release
dotnet run --project src/Dotask.Cli -c Release --no-build -- --use-dir examples/basic/.tasks write
```

The greeting should include `Hello from dotask, Ada!` and `configuration: Release`.
`write` calls `check` and `hello`, then creates or replaces
`examples/basic/artifacts/example.txt` with `Written by dotask.` and a newline.
The SDK also writes compilation/cache output. See the
[example walkthrough](examples/basic/README.md) for all commands and expected results.

The source checkout also contains
reusable `build`, `test`, `format`, `check`, `verify`, and `publish` targets in its own
[.tasks](.tasks/) directory.

Here, `check` requires Git and the configured solution's SDK/tools. Run `test` for
tests or `format --verify` to check solution whitespace. The project-specific
`verify` requires every check, including docs and catalog validation; failures
stop the sequence. The reusable `dotnet/verify` target still offers optional
checks for consuming projects. Use the launchers for work on dotask itself so
rebuilds run from a separate executable snapshot.

## Optional: install only inside this checkout

For an isolated preview instead of a user-wide installation, run from the checkout root:

```sh
./build.sh pack
dotnet tool install dotask --tool-path ./artifacts/tools --source ./artifacts/packages --version 0.1.0 --no-http-cache
```

On Windows use `.\build.cmd pack` for the first command.

Run `./artifacts/tools/dotask` on Linux/macOS or `./artifacts/tools/dotask.exe` on
Windows. To use the bare `dotask` command, add the **absolute** tools directory to
the current shell's PATH. Run the applicable command while still at the checkout root:

```sh
# Bash or Zsh
export PATH="$PWD/artifacts/tools:$PATH"
```

```fish
set -gx PATH "$PWD/artifacts/tools" $PATH
```

```powershell
$env:PATH = "$(Join-Path (Get-Location) 'artifacts/tools')$([IO.Path]::PathSeparator)$env:PATH"
```

Confirm `dotask --version` prints `dotask 0.1.0`. These changes last for the
current shell session. They do not perform a global installation or edit a profile.
Keep this checkout in place while using its tool directory.

After changing dotask's implementation, rebuild the package and replace the old
preview by running `dotnet tool uninstall dotask --tool-path ./artifacts/tools`,
then the install command above again. Use `--no-http-cache` when rebuilding the same
preview version. Uninstall affects this tool directory only.

## Commands

With `dotask` on PATH, run these from the checkout root:

```text
dotask
dotask --help
dotask help build
dotask build --configuration Release
dotask build -c Release
dotask dotnet-build -c Release
dotask verify -c Release
dotask build configuration=Release
dotask --use-dir examples/basic/.tasks hello -n Ada
```

`build`, `run`, `publish`, and similar names are ordinary project-defined targets,
not built-in dotask commands. Available targets depend on the selected `.tasks`
directory. You create the directory and files yourself; there is no `init` command.

Shared tasks live at `.tasks/<source>/<group>/<task>.cs`. Their full names are
paths such as `dotask-official/dotnet/build`; `dotnet/build` and `build` work when
unique. A top-level `build.cs` can orchestrate multiple groups. Old space-grouped
filenames remain supported. Use source-qualified names in calls between tasks.

The target list shows each file once using its short name when available;
detailed target help shows the same label and the actual source file. Completion
supports both invocation forms. Use full names
for YAML target defaults and reusable nested calls;
see [target naming](docs/TARGETS.md#target-names).

`dotask` and `dotask help` show only the current project's name, description,
shared settings, targets, and target options. `dotask --help` (or `-h`) shows only
CLI usage and works without a project. `dotask help build` shows just that target.
Set the optional project identity in `.dotasks.yaml`:

```yaml
version: 1
name: MyApp
description: Build, test, and publish MyApp.
settings:
  solution: MyApp.slnx
```

If `name` is omitted, the project directory's name is used. These top-level
identity fields are separate from shared `settings` accessed through `project.Config`.

Project summaries and target help read XML documentation and YAML settings without compiling,
restoring packages, or running target code. Metadata and default-value errors
appear beside affected targets; compiler errors are reported when you run a
target. Completion only reads metadata and paths. See the [usage guide](docs/USAGE.md)
for discovery, argument rules, exit codes, and troubleshooting.

## Downloadable shared tasks

```sh
dotask --list "dotnet/*"
dotask --add "dotnet/{build,run,format}"
dotask --sync --dry-run
dotask --sync
```

Shared tasks are copied into the project and committed with `.dotasks.yaml` and
`.dotasks-lock.yaml`. Existing handwritten files and locally modified shared tasks
are protected. Private tasks use the same workflow without publishing anything.
Normal execution/help/completion stay offline and use the committed project files.

Maintainer catalog generation is also a C# task: `./build.sh catalog`
(Windows: `.\build.cmd catalog`).

The official catalog is prepared under [shared-tasks](shared-tasks/) but must be
published before live online downloads work. Private tasks and the local preview
catalog work now. See [shared-task usage, conflict handling, and preview setup](docs/SHARED-TASKS.md).

## Shell completion

Load the integration for your current shell, with `dotask` on PATH. Choose one
block below.

**Bash:**

```sh
source <(dotask completion bash)
```

**Zsh:** initialize completion first if your shell has not already done so.

```zsh
autoload -Uz compinit
compinit
source <(dotask completion zsh)
```

**Fish:**

```fish
dotask completion fish | source
```

**PowerShell:**

```powershell
dotask completion powershell | Out-String | Invoke-Expression
```

Targets, full option names, aliases, declared choices, booleans, and declared
file/directory values are completed. The selected `--use-dir` is honored.
These commands print/load a script; dotask does not edit your shell profile.
Try `dotask --use-dir examples/basic/.tasks hello --configuration ` followed by
Tab: it should offer `Debug` and `Release`. See [completion details](docs/USAGE.md#shell-completion).

## Development

```sh
./build.sh                         # Complete verification, Release
./build.sh build -c Debug          # Build the solution
./build.sh test -c Release         # Build and run tests
./build.sh format --verify         # Check solution formatting and task C# whitespace
./build.sh verify-docs             # Required docs and Git whitespace
./build.sh git/check --whitespace  # Git availability and staged/unstaged whitespace
./build.sh catalog --verify        # Check shared source hashes/metadata
./build.sh catalog                 # Regenerate the shared catalog
./build.sh shim --verify           # Verify bundled shim source/binary hashes
./build.sh pack                    # dotnet/pack: build local CLI NuGet packages
./build.sh install                 # Install the current source for this user
./build.sh help                    # Project task help
```

On Windows use `.\build.cmd` with the same arguments. With no arguments the
launchers run `verify`; with arguments they forward them unchanged to dotask.
`./build.sh help` displays project help, while `./build.sh --help` displays CLI help.
Both launchers select the checkout containing the script even when called from
another directory; relative task paths and `--use-dir` start at that checkout.

The bootstrap always builds its runner in Release using normal incremental SDK
builds. A task's `-c Debug` selects that task's configuration independently.
The runner and all dependencies are copied from the evaluated publish directory
into a unique OS temporary directory, allowing tasks to rebuild or clean the
original outputs on Windows. Bootstrapping alone does not install anything;
the explicit `install` task does. First use can restore
NuGet packages and requires access to the configured feeds.

The MSBuild copy hook lives in `.tasks/misc/bootstrap.targets` and is imported
by the CLI project. It is build support data, so dotask does not discover it as a
task. Only supported code extensions are considered; currently that means `.cs`.

After adding, editing, renaming, or removing a shared task or one of its declared
support files, run `./build.sh catalog` **after your final edits/formatting and
before committing**. Commit the regenerated `shared-tasks/catalog.json` with the
sources. Run `./build.sh` afterward; its catalog verification fails if the index
is stale and never rewrites it. `catalog.cs` contains the generation logic and
reuses the CLI metadata parser; there is no separate catalog helper task file.

`verify-docs` checks required files, then calls its declared `git/check` dependency
with `--whitespace` to check staged and unstaged Git diffs. Plain `git/check` only
checks Git availability and works outside repositories. The whitespace checks
do not require a clean working tree and do not inspect untracked files.
`verify-docs` does not validate Markdown links or execute examples. Check those separately
when changing documentation. Focused test runs may use `dotnet test dotask.slnx
-c Release --filter ...` directly. `format` resolves to `dotnet/format`, the
standard shared formatter; it checks the solution and standalone C# tasks.
Without `--verify`, it applies formatting and runs the optional official
`text/fixeol` task when installed. The shared library's canonical task sources
live in `shared-tasks/`; the format, install, and pack copies under `.tasks/dotnet/`
are kept identical. Installation uses one `install.cs` file. `pack` creates local
NuGet packages from `settings.project`, with `--output` defaulting to
`artifacts/packages`; it does not publish to a feed.

Repository builds respect an existing user-level `Directory.Build.props` output
policy. Runtime task builds use an isolated external cache. Windows, Linux, and
macOS verification is configured in [.github/workflows/verify.yml](.github/workflows/verify.yml).
Local results are documented in [docs/VERIFICATION.md](docs/VERIFICATION.md).

The [documentation index](docs/README.md) links to all guides and design contracts.

## License

dotask is licensed under the [MIT License](LICENSE.md).
Copyright (c) 2026 Kody Brown.

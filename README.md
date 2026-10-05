# dotask

Portable project tasks written as individual C# or Rust files, with YAML `.task` groups
for composing existing tasks. Use `dotask --create-task` for interactive group creation. Put targets in `.tasks/`,
describe C# tasks with XML documentation and Rust tasks with leading YAML documentation, and keep project-specific settings in
`.dotasks.yaml`. Copy the same target file between projects without a
registration step.

```text
.dotasks.yaml
.dotasks-lock.yaml
.tasks/
  build.cs
  _/dotnet/build.cs
  _/dotnet/run.cs
  create-installer.cs
```

This is a **0.1.0 local preview**, with no public package release yet. Windows,
Linux, and macOS are intended hosts. Current native CLI/task work is verified on
Windows x64; Linux/macOS acceptance for this cutover is deferred. See [verification status](docs/VERIFICATION.md).

Use **.NET SDK 10.0.300 or later in the .NET 10 family** for C# execution, and
**Rust 1.95+/Cargo** for Rust execution. The .NET SDK compiles C# targets;
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

The .NET SDK, Git, and Rust 1.95+ with rustfmt, clippy, and the host linker are
required; dotask does not need to be installed.
The launchers compile this checkout, stage a temporary runner, and use it to run
the repository's `verify` task in Release. That task checks prerequisites, builds
and tests the Rust CLI, installer, SDK, and .NET solution, and checks formatting,
documentation, and the shared catalog.
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

The task executes the project's `create-installer`, then runs the resulting
standalone installer. Dotask's installer contains a self-contained Release
application for your host and installs it without administrator access. Each version/build gets a separate
directory; older builds are retained. Windows gets a small `.exe` launcher and
`.shim` file; Linux/macOS get a symlink. No shim compiler is needed.

The command directory is the installer’s `--bin-dir` when provided, otherwise the `BIN`
environment variable, otherwise the platform default below. If it is not already
on PATH, add **that one directory** once:

- **Bash:** add `export PATH="$HOME/.local/bin:$PATH"` to `~/.bashrc`.
- **Zsh:** add that line to `~/.zshrc`.
- **Fish:** run `fish_add_path "$HOME/.local/bin"`.
- **Windows:** add `%LOCALAPPDATA%\bin` to your **user Path** in Environment
  Variables, then open a new terminal.

Pass installer options through `--installer-args` as a JSON array; see the
[installer examples](docs/INSTALLATION.md#build-and-install-dotask-itself).
For a custom command directory, substitute its path. The installer prints the
selected installation location; it never edits PATH or shell profiles.
Then run `dotask --version` (expected: `dotask 0.1.0`).

The installation is independent of this source checkout. To check which command
your shell resolves, use `command -v dotask` on Bash/Zsh or `Get-Command dotask -All`
on PowerShell. An existing global tool or `dt` shortcut is not changed. See
[installation](docs/INSTALLATION.md) for directories, overrides,
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
If you deleted the installed launcher, the same command recreates it using the
existing installation records. Changed or replaced launchers remain protected;
do not delete ownership records to bypass a conflict.

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
stop the sequence. It also requires Rust 1.95+, rustfmt, and clippy to build/test
the standalone installer and Rust CLI preview before the .NET integration suite.
Destination machines need Rust only to execute Rust tasks and a .NET SDK only
to execute C# tasks. The standalone installer needs neither toolchain.
See [installer tasks and schema](docs/INSTALLATION.md).
The reusable `dotnet/verify` target still offers optional
checks for consuming projects. Use the launchers for work on dotask itself so
rebuilds run from a separate executable snapshot.

## Optional: install only inside this checkout

Use the standalone installer with explicit isolated locations. This uses the
same native payload as current-user installation, without changing the normal
installation. On Windows:

```powershell
.\build.cmd install --installer-args '["--install-dir","C:/tmp/dotask-local-preview/app","--bin-dir","C:/tmp/dotask-local-preview/bin"]'
& 'C:/tmp/dotask-local-preview/bin/dotask.exe' --version
```

On Unix, use `./build.sh install` and absolute temporary install/bin paths in the
same JSON argument array. Uninstall through the retained installer inside the
chosen application's `installer/` directory. Do not use `dotnet tool install`
for the native payload; the C# CLI project remains a verification reference.
## Initialize a project

With the current `dotask` installed, run this **from the project directory you
want to initialize**:

```sh
dotask --init
dotask
```

This creates an empty `.tasks/` directory and `.dotasks.yaml`, using the directory
name for the project name:

```yaml
version: 1
name: "MyProject"
description: ''
settings: {}
```

Edit the description and shared settings, then write your own tasks or select
shared tasks with `dotask --add "dotnet/{build,run,format}"`. Commit the
configuration and task files. The lock file is created when shared tasks are added.
Initialization is offline, needs no language SDK, and safely preserves existing
files when repeated. It initializes the current directory even inside another
DoTask project; it does not search parent directories.

Use `dotask --init --help` for details, including custom task directories and
existing-configuration handling. The [initialization reference](docs/USAGE.md#initialize-a-project)
describes those rules. The source bootstrap launchers always select their own
checkout, so use the installed command from the intended project directory when
initializing another project.

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
directory. Use `dotask --init` to create the project structure, then write task
files or add shared tasks. Bare `init` remains an ordinary task name.

Shared tasks live at `.tasks/<source>/<group>/<task>.cs`. Their full names are
paths such as `_/dotnet/build`; `dotnet/build` and `build` work when
unique. A top-level `build.cs` can orchestrate multiple groups. Old space-grouped
filenames remain supported. Use source-qualified names in calls between tasks.

The target list shows each file once using its short name when available;
detailed target help shows the same label and the actual source file. Completion
supports both invocation forms. Use full names
for YAML target defaults and reusable nested calls;
see [target naming](docs/TARGETS.md#target-names).

`dotask` and `dotask help` show only the current project's name, description,
shared settings, targets, and target options when `--verbose` is supplied.
By default they show only Targets and the detailed-help hint. `dotask --help` (or `-h`) shows only
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
./build.sh rust-cli                # Build the native Rust CLI preview
./build.sh rust-cli --verify       # Rust CLI tests, rustfmt, and clippy
./build.sh pack                    # Create the native CLI standalone installer package
./build.sh install                 # Install the current source for this user
./build.sh help                    # Project task help
```

On Windows use `.\build.cmd` with the same arguments. With no arguments the
launchers run `verify`; with arguments they forward them unchanged to dotask.
`./build.sh help` displays project help, while `./build.sh --help` displays CLI help.
Both launchers select the checkout containing the script even when called from
another directory; relative task paths and `--use-dir` start at that checkout.

The bootstrap builds the native runner in Release with Cargo and publishes the
C# support host. A task's `-c Debug` selects its C# configuration independently.
The native executable, SDK sources, and evaluated C# publish output are copied
into a unique OS temporary directory, allowing tasks to rebuild or clean the
original outputs on Windows. Bootstrapping alone does not install anything;
the explicit `install` task does. First use can restore
NuGet packages and requires access to the configured feeds.

Windows staging lives in `.tasks/misc/prepare-bootstrap.ps1`; Bash stages the
same layout. These launchers only build/stage the runner. Discovery accepts
`.cs` and `.rs` targets and declarative `.task` YAML groups.

After adding, editing, renaming, or removing a shared task or one of its declared
support files, run `./build.sh catalog` **after your final edits/formatting and
before committing**. Commit the regenerated `shared-tasks/catalog.json` with the
sources. Run `./build.sh` afterward; its catalog verification fails if the index
is stale and never rewrites it. `catalog.rs` uses the CLI's compiler-free metadata
transport; Roslyn still reads C# documentation in the support host.

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
live in `shared-tasks/`; the format, install, and pack copies under `.tasks/_/dotnet/`
are kept identical. Installation uses one shared `install.cs` file to invoke the
Rust `create-installer` task. The local `pack.rs` creates the native installer;
explicit `dotnet/pack` remains the generic .NET packaging task, using
`settings.project` (the retained C# reference here). Neither publishes a release.

Repository builds respect an existing user-level `Directory.Build.props` output
policy. Runtime task builds use an isolated external cache. Windows, Linux, and
macOS verification is configured in [.github/workflows/verify.yml](.github/workflows/verify.yml).
Local results are documented in [docs/VERIFICATION.md](docs/VERIFICATION.md).

The [documentation index](docs/README.md) links to all guides and design contracts.

## Rust CLI development preview

The root Cargo workspace contains `src/dotask-cli`, `src/dotask-installer`, and `src/dotask-sdk`,
with a single root `Cargo.lock` and release profile. The new CLI builds a native
`dotask` executable alongside the installer. The Rust CLI is the bootstrap
runner and installer payload. The C# CLI remains a behavior reference.
The C# task authoring library and shared tasks remain maintained components.
See the [transition design](docs/DESIGN.md#rust-cli-transition) for the agreed
replacement stages and future task-language selection.

Build and try the Rust CLI on Windows:

```powershell
.\build.cmd rust-cli
& 'C:\tmp\_rust\dotask\target\release\dotask.exe' --help
& 'C:\tmp\_rust\dotask\target\release\dotask.exe' --version
& 'C:\tmp\_rust\dotask\target\release\dotask.exe' --use-dir .\examples\basic\.tasks hello --name Rust
```

On Linux/macOS:

```sh
./build.sh rust-cli
/tmp/_rust/dotask/target/release/dotask --help
/tmp/_rust/dotask/target/release/dotask --version
/tmp/_rust/dotask/target/release/dotask --use-dir ./examples/basic/.tasks hello --name Rust
```

The preview runs `.cs` and `.rs` tasks and `.task` groups, including parameters,
YAML defaults, nested calls, structured installer results, exit codes, and Ctrl+C.
Bare invocation and `help` list project tasks; `help TARGET` and `TARGET --help`
read metadata without compilation. CLI-only `--help`/`-h` and `--version` need
no language host. C# metadata and compilation use the staged .NET 10 support host
under `release/csharp/`; keep that directory beside the executable.

The native CLI also implements `--init`, shared-task listing/save/add/sync/remove,
shell completion, full project/target help, and `--create-task`. Configuration,
binding, requirements, and YAML groups run in Rust. Initialization and YAML-only
projects need no .NET host. Existing shared-task ownership, local-edit protection,
and recovery journals are preserved. Rust tasks use the bundled `sdk/` sources;
see [Rust authoring](docs/TARGETS.md#rust-tasks). Shared Rust language variants and
language selection remain pending. These build commands do not install or replace an active CLI.

Use `rust-cli --verify` through the launcher for its tests, rustfmt, and clippy.
The complete repository gate requires those checks too. `.cargo/config.toml`
sets the target directory to `/tmp/_rust/dotask/target` (on Windows, under the
current drive's `tmp` directory). Build tasks, support staging, packaging, and
native tests query `cargo metadata` for the evaluated directory and honor an
explicit `CARGO_TARGET_DIR` override. The commands above use the checked-in
default. Direct Cargo invocations use the same configuration; on Windows set
`RUSTFLAGS=-C target-feature=+crt-static` to match the task's static CRT build.

## License

dotask is licensed under the [MIT License](LICENSE.md).
Copyright (c) 2026 Kody Brown.

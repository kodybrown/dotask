# Using dotask

dotask runs project-defined C# tasks. A **target** is one task file: `.tasks/build.cs`
becomes `dotask build`. A **parameter** is a declared command-line option for that
target. **Settings** are shared values from `.dotasks.yaml`.

After building the source, follow [Install dotask](../README.md#install-dotask)
to put the command on PATH for use in other projects. Building alone does not
install or update the command. The commands here assume that installation.
The reusable `install` target is documented in [current-user installation](INSTALLATION.md);
it is a project task, not a built-in CLI management flag.
To try the source checkout without installing, replace `dotask` with `./build.sh`
(Windows: `.\build.cmd`); the launcher selects its own checkout, and `--use-dir`
can select another project's tasks. See [writing targets](TARGETS.md) to create your own.

## Project layout and discovery

```text
my-project/
  .dotasks.yaml                        Optional identity, settings, and defaults
  .dotasks-lock.yaml                   Generated tracking for added shared tasks
  .tasks/
    build.cs                          Handwritten orchestrator
    _/dotnet/build.cs                 Installed shared task
    _/dotnet/restore.cs               Its required dependency
    private-tasks/tools/check.cs      Installed private task
    _support/Helpers.cs               Included helper, not a target
  src/
```

Discovery walks upward to the nearest `.dotasks.yaml` or `.tasks` directory.
The configuration anchors the project even before `.tasks` exists. A nearer
project wins; configurations and task directories are never merged across roots.
Tasks are discovered recursively. The task-root `_` directory contains official tasks.
Other hidden and underscore-prefixed entries,
`bin`, `obj`, `node_modules`, and symbolic links/reparse points are skipped.

The task's path relative to `.tasks`, without `.cs` or `.task`, is its full name. For example,
`_/dotnet/build.cs` is `_/dotnet/build`. `dotnet/build`
and `build` work as shortcuts when unique. Exact project-relative names take
precedence, so `.tasks/build.cs` owns `dotask build` and can orchestrate several
source-qualified tasks. Ambiguity produces an error with the available full names.
There is no `default-group`, automatic language selection, or registration list.
Older `<group> <target>.cs` files retain their hyphenated full names and aliases.
Grouped paths also provide compatible `group-target` shortcuts when unambiguous.
See [target naming](TARGETS.md#target-names).

Use `--use-dir PATH` to select a task directory explicitly, relative to the original
invocation directory or by absolute path. There is no fallback for a missing
explicit directory during help/execution. Shared-task `--add` or `--init` may create it.
The nearest root `.dotasks.yaml` above the selected directory anchors the project;
without it, the selected directory's parent is the root. A legacy directory with
its own `config.yaml` retains its parent-root behavior during migration.

Targets, process working directories, parameter paths, and settings accessed with
`GetPath` use that project root. Task-local assets use `project.TargetDirectory`.
Nested calls and completion retain the selected directory. CLI-only `--help` does
not inspect it. Shared-task management requires a subdirectory within the project.

### Migrate an existing project

Move `.tasks/config.yaml` to `.dotasks.yaml` in the project root without changing
its settings. Legacy configuration is still readable; if both files exist,
DoTask reports a conflict instead of merging them. Plain and space-grouped task
filenames continue to work. When moving a task into subdirectories, update its
full-name YAML defaults and explicit calls to reflect its new identity.

Existing handwritten/copied files are never automatically adopted as managed
shared tasks. Review and relocate such files yourself before using `--add` at the
same destination. Use the exact `.tasks` and `.dotasks.yaml` spelling on
case-sensitive filesystems.

## Initialize a project

Run `dotask --init` in the existing directory that should become the project root.
It creates `.dotasks.yaml` and an empty `.tasks/`, prints what it created or kept,
and suggests editing settings and writing or adding tasks. Unlike normal project
discovery, initialization uses exactly the current directory, even when a parent
already contains a DoTask project. It does not initialize Git or detect a language.

For a directory named `MyProject`, the initial configuration is:

```yaml
version: 1
name: "MyProject"
description: ''
settings: {}
```

The name is quoted/escaped as a YAML string, including names such as `true` or
`2026`. Initialization creates no sample targets, registrations, lock file, or
cache. It does not download, restore, compile, or execute anything and needs no
language SDK. Adding shared tasks later creates `.dotasks-lock.yaml`; `--add`
also retains its existing ability to create a minimal missing configuration.
Git records the empty task directory only after files are added to it.

Repeating `--init` succeeds while preserving existing configuration bytes,
comments, task files, and tracking. It creates only missing pieces. Invalid
existing YAML, wrong entry types (such as a file named `.tasks`), and symbolic
links/reparse points in initialization paths are errors. A selected task
directory containing legacy `config.yaml` requires manual migration first;
initialization will not create conflicting old/new configuration. These checks
happen before initialization writes files. New configuration is published
without replacing a destination that another initializer or editor created.

For a custom task directory:

```sh
dotask --init --use-dir "build support/tasks"
dotask --use-dir "build support/tasks"
```

The selected directory must be beneath the current directory; the configuration
stays at the project root. Root/outside locations and paths overlapping the
configuration or lock file are rejected. The override is not stored in YAML;
continue passing `--use-dir` when listing, adding, or running tasks there.
Relative and absolute paths beneath the current root are accepted.

`dotask --init --help` (or `-h`) prints initialization help without reading project
configuration or creating files. There are no positional arguments, `--force`,
or initialization `--dry-run` options. Use the installed command in the intended
directory: `build.sh`/`build.cmd` always change to the DoTask source checkout.

## Commands

| Command                                                       | Behavior                                                                                   |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `dotask`, `dotask help`                                       | Show only project name, description, shared settings, targets, and combined target options |
| `dotask --help`, `dotask -h`                                  | Show only CLI usage; no project directory is required or inspected                         |
| `dotask --init`                                               | Create missing project configuration and a task directory in the current directory        |
| `dotask help build`, `dotask build --help`, `dotask build -h` | Show one target's description, options, requirements, and examples                         |
| `dotask build -c Release`                                     | Validate arguments and requirements, compile, then execute `build.cs`                      |
| `dotask --version`                                            | Print the CLI version; no project directory is required                                    |
| `dotask completion bash`                                      | Print a shell integration script; no project directory is required                         |
| `dotask --list "dotnet/*"`                                    | Browse shared tasks in the official online cache                                           |
| `dotask --save dotnet/build`                                  | Download a selection without changing the project                                          |
| `dotask --add "dotnet/{build,run,format}"`                    | Copy selected tasks and required companions into the project                               |
| `dotask --sync [TASK...]`                                     | Refresh installed shared tasks while protecting local edits                                |
| `dotask --remove TASK...`                                     | Remove only unchanged tracked shared tasks                                                 |

See [shared tasks](SHARED-TASKS.md) for private tasks, offline use, caches,
`--dry-run`, conflict review, and the unpublished online-catalog preview.

`build` is an example target name. `build`, `test`, `check`, `verify`, `format`, `run`,
`publish`, `deploy`, and `release` exist only when the project supplies those files.
`--init` is built in; bare `init` is still a project-defined task name. There is
no built-in `validate` or JSON help output. Management
`--dry-run` applies to add/sync/remove; it does not preview arbitrary task execution. Run one target per invocation; use a C# orchestration
target for a sequence of tasks.

This repository's top-level `check` requires its Git and .NET checks.
Its `dotnet/check` task checks the configured solution's presence and the
.NET SDK and bundled MSBuild/formatter tools, reporting their versions.
It takes no target options and does not run
tests or check source formatting. Use `dotask test` and `dotask format --verify`
for those operations. Its top-level `verify` requires prerequisites, solution
build/tests, formatting, docs, and catalog validation; use `./build.sh` or
`.\build.cmd` to bootstrap that gate from a source checkout without an installed
dotask. See the [development commands](../README.md#development).

The separate reusable `dotnet/verify` task runs available `dotnet-check`,
`dotnet-test`, and `dotnet-format` tasks, with formatting in verification mode.
It reports and skips missing targets, stops on failure, and fails if none are
available. This optional behavior is not the source repository's gate. The detailed
[task reference](TARGETS.md#reuse-a-target) describes the scope and SDK bootstrap
limitation; another project's `check` can have its own behavior.

The project summary uses the optional top-level `name` and `description` from
`.dotasks.yaml`. Without a nonempty name, it uses the project root directory's name;
an absent description is omitted. `Settings` shows shared values first, preserving
nested objects. String values such as `project: src/App/App.csproj` are unquoted
unless empty or containing whitespace/control characters; those use single
quotes, such as `solution: 'My Application.slnx'`. Apostrophes inside quoted
strings are doubled, and control characters are escaped. Arrays and other scalar
values retain their JSON representation. The final `tasks` item identifies the
selected task directory relative to the project root, using `/` separators (for
example, `tasks: ./.tasks` or `tasks: ./.abc`). It is display information, not an
added key in `project.Config`. `Targets` lists the targets without a
directory path in its heading. `Target options`
shows the declared command-line parameters and any common defaults. Shared
settings are not automatically command-line options.
Each target appears once in `Targets:`, using only its short name (for example,
`run`) when available. When the short name is ambiguous or taken, its full name
is listed. Detailed help uses the same target label and identifies the actual
task file on a `Source:` line below its description.

Project summaries and target help read XML documentation from C# source and load
the YAML configuration. All help forms avoid invoking the SDK, restoring packages,
compiling targets, or running their entry points, including on the first invocation.
CLI-only `--help`/`-h` also skips discovery, metadata, and YAML loading, so it works
even with invalid project configuration. `dotask help --help` shows CLI usage too.
There is no metadata cache to warm up or invalidate. Metadata/default-value
errors for individual targets replace their descriptions in the listing; other
targets remain usable. Invalid project YAML can prevent the whole listing.
If neither project configuration nor a task directory is found, `dotask` and `dotask help` report an error (exit 1)
without printing CLI usage. Use `dotask --help` for the CLI reference.

Combined help groups options when their names, aliases, types, choices,
required flags, and completion rules agree. Differences in descriptions or
effective defaults do not repeat the option. Each entry shows the option name,
alias, and declared choices in an aligned left column, with its description and
any common default on the right. Choices appear as `<Debug|Release>` regardless
of the option's type; options without declared choices show their type, such as
`<string>`, `<bool>`, or `<path>`. Per-option target lists and default breakdowns
are omitted from the summary:

```text
Target options:
  --configuration, -c <Debug|Release>  Build configuration.
  --verify, -v <bool>                  Check formatting. (default: false)

See `dotask help <target>` for detailed information on each target.
```

There are no blank lines between options. A default appears inline only when it
is shared by every target in that option group. When defaults differ, or a target
has no default, the summary omits that default; use detailed target help to see
the exact value. The summary ends with a hint directing you to that help.
The summary uses the most common nonempty description, taking the first one in
target order when tied. Target-specific help retains the exact description and
effective default. Incompatible contracts remain separate entries, even when
their option names match. Required options are marked `(required)`. Long option
signatures occupy their own line to keep the description column readable.
An option shown in combined help is **not**
automatically accepted by every target; use `dotask help TARGET` for its contract.

Help text wraps to the detected console width when it is **at least 60 columns**.
Continuation lines align with their descriptions or setting values. Long tokens
are split when needed to fit. Below 60 columns, when the width is unavailable, or
when stdout is redirected to a file/pipe, dotask does not insert wrapping. This
applies to the project summary, target help, and CLI help; completion data and
task/process output are not reformatted.

Selected-target help (`dotask build --help` or `dotask help build`) separates
the target, options, requirements, and examples, with the source beneath the target
description:

```text
Target:
  build           Restore packages and build the solution.
                  Source: ./.tasks/_/dotnet/build.cs

Options:
  --configuration, -c <Debug|Release>  Build configuration. (default: Debug)
  --dotnet <string>                    .NET CLI executable name or path. (default: dotnet)

Requires:
  - tool: dotnet
  - setting: solution

Examples:
  dotask build
  dotask build -c Release
```

Only populated optional sections are shown. Remarks follow the target details;
declared capabilities appear in a `Capabilities:` list after requirements. Source
paths are relative to the selected project root, use `/` separators, and retain
the actual filename. Paths containing spaces are enclosed in single quotes;
paths without spaces are unquoted. For `--use-dir .abc`, the source could be
`'./.abc/dotnet build.cs'`. Invoking from a subdirectory does not change that
project-relative path. Headings directly precede their contents. The `Source:`
line is aligned with the target description, with no blank line between them.

Help checks metadata and effective defaults, but does not check compilation,
execution requirements, or missing mandatory arguments. Compiler errors are
reported when you run a target. A listing may return exit code 0 even when it
displays individual metadata/default errors. **Do not use help's exit code as a
build or validation gate.** There is no `help --validate` option in this version.

## Arguments and case

For a target declaring `configuration` with alias `c`, these forms are equivalent:

```text
dotask build --configuration Release
dotask build --configuration=Release
dotask build -c Release
dotask build configuration=Release
dotask build c=Release
```

Target names, option names, aliases, YAML keys, and declared choices match without
case sensitivity. `dotask BUILD C=release` resolves the same option and returns
the declared spelling `Release` to the task. Arbitrary string values and paths
retain their case; filesystem lookup follows the host's rules.

Quote a value containing spaces using your shell's syntax, for example
`dotask hello --name "Ada Lovelace"`. Pass a literal empty string as `name=`.
Only the first `=` separates a name from its value, so `name=a=b` passes `a=b`.
When a value starts with `--`, use `name=--value` or `--name=--value`.

Boolean options accept a flag (`--verify` means true), an explicit value
(`--verify false`), or an assignment (`verify=false`). Boolean values are `true`
and `false`, not `1`, `0`, `yes`, or `no`. An optional boolean with no default or
argument resolves to false. A required boolean still needs a value or default.

Unknown options, repeated options (even using both alias and full name), invalid
types, and unsupported choices fail before task execution. There are no short
option bundles, positional task arguments, repeated list-valued options, or `--`
passthrough. Declare the options your target needs and construct child arguments
in C#.

Put the target before its options. Global `--use-dir`, `--help`/`-h`, and
`--version` are recognized throughout the argument list. To pass a literal global
option as a value, attach it to the option name: `name=--help`.

## Project identity, settings, and defaults

Shared settings are read by target code through `project.Config`. Task parameters
come from `project.Parameters`; the two collections are separate. For example:

```yaml
version: 1
name: MyApp
description: Build, test, and publish MyApp.
settings:
  solution: MyApp.slnx
targets:
  _/dotnet/build:
    defaults:
      configuration: Release
```

`name` and `description` are optional display strings, separate from `settings`.
They do not become target parameters or keys in `project.Config`. Existing
configuration files without these fields continue to work.

For each parameter, the precedence from highest to lowest is:

1. Explicit command-line argument or explicit `ExecTargetAsync` argument.
2. `targets.<full-target-name>.defaults.<full-option-name>` in `.dotasks.yaml`.
3. The `<option default="...">` value in the target source.

You do not register targets in YAML. Add a `targets` entry only to override that
target's defaults. Setting `settings.configuration` does not automatically set
the `configuration` parameter. Likewise, `configuration=Release` changes a
parameter for that invocation; it does not edit YAML or override a shared setting.
There is no environment-variable interpolation in YAML.

The example configures `.tasks/_/dotnet/build.cs`, whether invoked
by its full name or an available shortcut. Use `targets.build` for a plain `build.cs`; short aliases are not
looked up when loading defaults. Renaming a plain task to a grouped filename
requires updating its YAML target-default key to the new full name.

See the [configuration reference](TARGETS.md#yaml-configuration) for value types,
arrays, nested keys, and how defaults are validated.

## Shell completion

Load the appropriate [shell integration](../README.md#shell-completion) in each
shell session. The same task XML powers help, argument validation, and completion;
no separate completion registration is needed for new targets or options.

Using the included basic example from the checkout root, type these lines and
press Tab where shown; do not type the `<Tab>` marker:

```text
dotask --use-dir examples/basic/.tasks he<Tab>
dotask --use-dir examples/basic/.tasks hello --con<Tab>
dotask --use-dir examples/basic/.tasks hello --configuration <Tab>
dotask --use-dir examples/basic/.tasks hello configuration=Re<Tab>
```

Expected suggestions include `hello`, `--configuration`, `Debug`/`Release`, and
`configuration=Release`. Choice matching ignores case. Shells differ in how they
display menus and whether another Tab is needed for multiple candidates.

Completion supports full target names and available short aliases, option names
and aliases, choices, booleans,
and paths declared with `type="path"`, `completion="file"`, or
`completion="directory"`. Task-option
paths use the selected project root; `--use-dir` paths use the invocation
directory. Paths containing spaces are quoted/escaped by the shell adapter.

Target completion reads source metadata and enumerates paths without loading project
configuration. Shared-management completion reads local catalog/private-source metadata
or installed tracking. Neither mode downloads, compiles, restores packages, checks
requirements, or executes targets. A suggested
target may still have a compiler or configuration error. Completion is not
validation and does not evaluate arbitrary shell expressions.

The generated integrations attach to the command name `dotask`. Put the tool on
PATH and use that command when testing; `dotnet run ...` does not activate these
completions. Internal commands `__complete` and `__exec` are implementation
protocols, not stable automation APIs.

## Failures and exit codes

| Outcome                                                                                                           | Exit code                      |
| ----------------------------------------------------------------------------------------------------------------- | ------------------------------ |
| Successful task                                                                                                   | 0                              |
| Invalid CLI/configuration/metadata, failed requirement, compilation failure, or ordinary unhandled task exception | 1                              |
| Task explicitly returns a nonzero exit code                                                                       | That exit code                 |
| Unhandled `ProcessFailedException`, including failed nested tasks                                                 | The failed process's exit code |
| Cancellation handled by dotask, normally Ctrl+C                                                                   | 130                            |

Shells and operating systems can constrain numeric exit codes. A task can also
choose to handle an error itself. `RunAsync` throws on a nonzero exit by default;
`ThrowOnError = false` lets the target inspect the result. Awaited nested calls
stop the caller on failure unless the caller catches that exception.

Each `ExecTargetAsync` call executes again; dotask does not deduplicate calls or
skip completed tasks. A recursive call chain is an error. Cancellation terminates
launched process trees; independently detached processes remain the task author's
responsibility.

## Troubleshooting

| Symptom                                               | Check or action                                                                                                                                                                                                     |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `dotask` is not found                                 | Use the installed tool's full path or add its directory to this shell's PATH; see installation in the README.                                                                                                       |
| No `.tasks` found / wrong targets shown               | Check the current directory and nearer `.tasks` directories; select the intended directory with `--use-dir`.                                                                                                        |
| SDK/file-based-app build failure                      | Run `dotnet --version` from the task directory; check the consuming project's `global.json` selects .NET 10.0.300 or later in the .NET 10 family.                                                                   |
| Restore fails while running a target                  | `#:package`/`#:project` may need package feeds; inspect NuGet configuration, feed access, and compiler diagnostics. Help does not restore.                                                                          |
| Help pauses for every target                          | Check that you rebuilt/reinstalled the current preview. Older previews compiled each target during help. For measurements, use the installed command or built CLI directly; `dotnet run` adds SDK startup overhead. |
| `(no description)` or declared options are missing    | Put XML directly on the explicit entry-point class or static `Main`; top-level statements do not expose this metadata. Use exact lowercase XML names.                                                               |
| Unknown option / duplicate option                     | Check `help TARGET`, XML spelling, single-letter aliases, and whether you passed the full name and its alias together.                                                                                              |
| Setting/default fails to load                         | Check `settings` versus `targets.<name>.defaults`, scalar types, duplicate keys, and full option names; aliases are not YAML default keys.                                                                          |
| `No dotask context is available`                      | Run the task through dotask; direct `dotnet run task.cs` does not supply the context or injected library.                                                                                                           |
| `Cannot start` a command                              | Check its installation, working directory, execute permission, and PATH. Shell built-ins and Windows `.cmd`/`.bat` launchers need an explicit shell.                                                                |
| Path resolves to the wrong place                      | Settings/parameter/helper paths are project-relative, not relative to the task file or invocation directory. Use the appropriate context directory explicitly.                                                      |
| Tab gives no task suggestions                         | Confirm `dotask --version` works, load the integration in this session, check XML, and confirm the selected tasks directory. Zsh needs `compinit`.                                                                  |
| New source changes do not appear in the installed CLI | Repack and reinstall the local preview as described in the README; a source build does not replace an installed tool.                                                                                               |

Services, generated configuration properties, remote execution, and task graph
scheduling are not available in this preview. See [current verification](VERIFICATION.md)
for the distinction between implemented behavior and host/shell acceptance.

## Run a YAML task group

A `.tasks/check.task` YAML file appears as `check` in help and completion and runs
with `dotask check`. It executes its listed existing targets in order; optional
missing targets are silently skipped, while failures stop execution. Use
`dotask help check` to inspect the steps and their explicit `with` parameters.
See [YAML task groups](TARGETS.md#yaml-task-groups) for the schema and
`require_at_least_1_step` behavior. Ordinary `.yaml` files are not targets.

## Create a group interactively

```sh
dotask --create-task
dotask --create-task --use-dir path/to/tasks
dotask --create-task --help
```

The creation wizard searches project tasks, prompts for explicit step parameters,
lets you reorder steps, and previews YAML before saving a new `.task` file.
It requires a terminal and an existing project; use `dotask --init` first for a
new project. It never runs selected tasks or overwrites existing files. Enter
`:cancel` or press Ctrl+C to cancel. See the [wizard reference](TARGETS.md#interactive-group-creation)
for prompts, defaults, and manual entry of absent tasks. Scripts should write YAML
directly; redirected wizard input/output is rejected with guidance.

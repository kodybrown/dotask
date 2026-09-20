# dotask guidance for AI assistants

Use this guide when creating, editing, explaining, or running dotask tasks in a
**consuming project**. For development of dotask itself, follow the repository's
[AGENTS.md](../AGENTS.md). This guide describes implemented preview behavior;
services and the other [deferred features](DESIGN.md#deferred) are not APIs.

In the dotask source checkout, use `./build.sh` (Windows: `.\build.cmd`) to
bootstrap and run the required repository verification. These launchers accept
dotask arguments and need no installed dotask; `./build.sh help` lists tasks.
The source repository's `verify` requires all stages, including documentation and
catalog checks. The reusable `dotnet/verify` task's optional behavior described
below applies to consuming projects, not that repository gate.

After final edits/formatting to any shared task or declared support file, run
`./build.sh catalog`, then `./build.sh`; its catalog check detects stale output
without rewriting it. Commit the generated `shared-tasks/catalog.json` with
those sources. The generator's methods are inside `.tasks/catalog.cs`.
Repository `verify-docs` checks required documents and delegates Git whitespace
checks to its declared `git/check` dependency with `Whitespace = true`.
Ordinary `git/check` remains a tool-availability check; `--whitespace` checks
both tracked diffs without changing files or requiring a clean working tree.

## Establish the project context

1. Read the consuming project's instructions and existing task files before
   editing. Identify the working directory, selected tasks directory, and its
   discovered project root. Inspect `.dotasks.yaml` if present.
2. Check `dotask --version`. If the tool is absent, use the
   [source or local-preview instructions](../README.md); there is no public
   package release to assume. Check the SDK selected from the task directory
   with `dotnet --version`.
3. Run `dotask --verbose` (or `dotask help --verbose`) for the project's name, description, shared
   settings, targets, and combined options. Use `dotask help TARGET` for one target
   and `dotask --help` for CLI usage only; `--help` does not inspect the project. Read
   diagnostics, not just exit status: help can exit 0 while reporting individual
   metadata/default errors. Help only reads source metadata and YAML. It never
   restores packages, compiles, or executes tasks, and does not prove they compile.
4. Confirm the actual task's effects from its source. `check`, `publish`, and
   similar names are user-defined commands, not guarantees about side effects.
   Follow the user's authorization and repository instructions for execution.

For an explicitly new project, run `dotask --init` from its intended root. This
creates missing `.dotasks.yaml` and `.tasks/` entries without replacing existing
files. It uses the current directory, not upward discovery; do not run it from a
source subdirectory expecting it to find a parent root. It runs offline with no
language SDK and does not create sample tasks, a lock file, or caches. Edit shared
settings explicitly before adding/running tasks; no language is auto-detected.
`--init --help` is read-only. Handle legacy/invalid configuration diagnostics by
reviewing the existing files, never deleting them to make initialization pass.
With `--init --use-dir PATH`, choose a subdirectory within the current root and
keep passing that override on later commands; it is not saved in YAML. Use the
installed command when initializing a consumer project because the source
bootstrap launchers always change to their own checkout.

With a custom task directory, include `--use-dir PATH` consistently. Relative
paths resolve from where the CLI was invoked. The nearest root `.dotasks.yaml` above the selected directory anchors the project;
otherwise its parent is the root. Legacy directory-local configuration retains
its parent-root behavior. Shared-task management requires a project subdirectory.
The verbose summary's `Settings` section ends with `tasks: ./.tasks` (or the selected
directory) relative to that root; this display entry is not a shared configuration
key. String settings omit quotes unless empty or containing whitespace/control
characters, in which case they use single quotes; arrays retain JSON formatting.
Options with declared choices show them in the signature, such as
`--configuration, -c <Debug|Release>`; otherwise the signature shows the type.
Compatible options appear once despite differing descriptions or effective
defaults. The summary omits target lists and default breakdowns, showing an inline
default only when it is shared. Incompatible types, aliases, choices, required flags, or completion
rules remain separate entries. Use `dotask help TARGET` for the exact target's
description and effective default.
Help wraps at console widths of at least 60 columns, with aligned continuation
lines; narrower/unknown widths and redirected stdout are left unwrapped.
Target help has labeled sections and a `Source:` line beneath the target
description. Its path is relative to the selected project root and uses the actual
task filename, including any grouping space;
paths containing spaces are enclosed in single quotes for display. Do not infer
a hyphenated filename from a command such as `dotnet-build`.

The reusable `_/dotnet/check.cs` in the starter catalog checks the configured
solution's presence and the SDK, MSBuild, and bundled formatter versions. It
takes no target parameters and does not run tests or verify formatting. Use
`test` and `format --verify` explicitly, or use `verify` to run the available
check/test/format targets in sequence. Verification skips missing targets, stops
on failures, and fails if no checks are available. Read a consuming
project's actual target rather than assuming that every `check` has this behavior.

## Compose existing tasks

For static orchestration, write a YAML `.task` group; see
[YAML task groups](TARGETS.md#yaml-task-groups). Use ordered `steps` with `run`,
optional `optional: true`, and explicit scalar parameters under `with`.
`require_at_least_1_step: true` rejects all-skipped execution; its default is
false. Only missing optional targets are skipped, never failures or ambiguity.
Humans can use `dotask --create-task` in a terminal for guided creation; agents
and scripts should author YAML directly rather than pipe answers into the wizard.
Groups have no CLI parameters or automatic forwarding. Use source-qualified
names and keep dynamic logic in C#. Group help and completion remain metadata-only.

## Write a task

Start from the [complete run target](TARGETS.md#a-complete-target) or copy an
existing target such as [dotnet/build.cs](../shared-tasks/dotnet/build.cs). For a first runnable
example without an application dependency, use
[hello.cs](../examples/basic/.tasks/hello.cs) with its
[.dotasks.yaml](../examples/basic/.dotasks.yaml).

1. Create a `.cs` task under `.tasks`, optionally in a group directory. Shared
   copies use `.tasks/<source>/<group>/<task>.cs`. Names follow project-relative
   paths; exact names win and shortcuts must be unique. Old space-grouped files
   remain supported. Keep helper sources under `_support`, which discovery skips.
2. Use `using DoTask;` and an explicit class containing `public static void Main()`
   or `public static async Task Main()`. Put XML documentation directly on the class
   or its static `Main`. Class docs win. Top-level-statement metadata is not read.
3. Add a clear `<summary>`, declare every accepted parameter with `<option>`,
   and add useful `<example>` and `<requires>` entries. XML names and attributes
   are lowercase and case-sensitive. Metadata remains in C#, not YAML.
4. Get the context with `var project = BuildContext.Current;` and shared settings
   with `var config = project.Config;`. Read validated parameters through
   `project.Parameters.Get<T>("full-name")`.
5. Put project-specific paths/settings under `settings` in `.dotasks.yaml`.
   Optional top-level `name` and `description` identify the project in summaries;
   they are not keys in `project.Config` or task parameters.
   Add `targets.<full-name>.defaults` only when overriding a parameter default
   (for example, `targets._/dotnet/run.defaults`, not `targets.run.defaults`).
   There is no registration requirement and no generated `config.SomeProperty`.
6. Use argument lists for child processes and portable-path helpers for actual
   paths. Await operations. Use `ExecTargetAsync` for other targets and forward
   each intended parameter explicitly. Prefer source-qualified target paths in reusable
   orchestration files so other groups or project entry points cannot change
   their dependencies. Copy the dependency files along with the caller.
   Shared tasks declare required companions in that same XML documentation:
   `<requires task="dotnet/restore" />` uses a same-source `group/task` ID;
   `<requires file="dotnet/_support/Helpers.cs" />` uses a source-root-relative
   path. Add/sync copy these companions; the declarations do not execute them.
   Keep `#:include` paths relative to the task file. Do not create `.task.json`
   sidecars or declare optional target calls as required dependencies.

Use the [authoring reference](TARGETS.md) for exact signatures and the
[CLI guide](USAGE.md) for accepted syntax. If a helper is not documented, inspect
the library before claiming it exists. Ordinary .NET APIs can fill gaps.

## Rules that prevent common mistakes

For installation tasks, read [the installation contract](INSTALLATION.md).
`BuildContext.InstallAsync(InstallationDefinition, CancellationToken)` installs
already published files; publishing and language-specific metadata stay in the
task sources. The reusable `dotnet/install.cs` contains its publishing/query
methods in the same file; it needs no separate helper source. Use the bootstrap
launcher for the initial upgrade from an older library, and explicit temporary
`--install-root` / `--bin-dir` directories for isolated tests. Do not remove
ownership/recovery records, edit hashes, or overwrite an existing global-tool
launcher to bypass a conflict. If an owned launcher was deleted, rerun the
bootstrap install with its original command directory to recreate it. Existing
modified launchers remain protected; no force flag is needed for a missing one.
Installation never edits PATH, removes old builds,
or retargets an existing `dt` shortcut automatically. Other task languages and a
public JSON installation interface remain deferred.

| Need                       | Use / behavior                                                                                                                       |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Ambient context            | `BuildContext.Current`; no constructor or DI container                                                                               |
| Shared setting             | `project.Config.Get<T>("key")`; dotted nested keys supported                                                                         |
| Optional setting/parameter | `Contains(key)` or `Get<T>(key, fallback)`; null counts as absent                                                                    |
| Task input                 | `project.Parameters.Get<T>("full-name")`; aliases are input syntax, not stored keys                                                  |
| Path setting               | `config.GetPath("output")`; absolute native path rooted at the project                                                               |
| Direct portable path       | `project.Path("artifacts", "output.txt")`; no manual separator replacement                                                           |
| Task-local resource        | `project.Path(project.TargetDirectory, "templates", "file.txt")`                                                                     |
| Child command              | `await project.RunAsync("dotnet", ["build", config.GetPath("solution")])`                                                            |
| Command options            | `new ProcessDefinition { Executable = "git", Arguments = ["status"], CaptureOutput = true }`                                         |
| Another task               | `await project.ExecTargetAsync("_/dotnet/build", new { Configuration = "Release" })`                                   |
| Target presence            | `await project.TargetExistsAsync("_/dotnet/test")`; no compilation/execution; invalid metadata still counts as present |
| Optional task              | `await project.ExecTargetIfExistsAsync("_/dotnet/test")`; inspect `Status`, `ExitCode`, and `Error`                    |
| Current task identity      | `project.TargetName` is the full name; `TargetFile` retains the actual source path                                                   |
| Task directories           | `project.TaskDirectory` is the selected tasks root, including `--use-dir`; `TargetDirectory` contains the current target file       |
| Host detection             | `project.OS`, `IsWindows`, `IsLinux`, `IsMacOS`; an `OS=linux` parameter does not change the host                                    |
| Validation beyond XML      | Explicit checks in `Main` and actionable `TaskException` messages                                                                    |

- Option types are `string`, `bool`, `int`, `number`, and `path`; read `number`
  as `double`. Aliases are one ASCII letter. Choices use a comma-separated
  `choices="Debug,Release"` attribute. There is no option array/object type.
- Explicit arguments override YAML target defaults, which override XML defaults.
  `settings` and task parameters are separate collections. YAML default keys use
  full option names. Identifiers match without case sensitivity; arbitrary string
  contents and filesystem case are preserved.
- A default satisfies `required="true"`. `type="path"` normalizes a path without
  checking existence. `completion="directory"` controls suggestions, not runtime
  validation. `<requires setting="x" />` checks presence, not type or file existence.
- `RunAsync` takes an executable plus separate arguments. Do not prequote list
  entries, concatenate a shell command, or normalize every argument as a path.
  Pipes, redirects, shell built-ins, and `.cmd`/`.bat` launchers need an explicitly
  invoked shell and its own quoting rules. A Unix-only shell command is not a
  portable task just because it appears inside C#.
- `RunAsync` throws on nonzero exit by default. If disabling `ThrowOnError`,
  inspect the result. Do not silently turn command failure into task success.
- Nested calls execute each time and receive only explicitly supplied parameters
  plus their own defaults. They share the configuration snapshot. Await sequential
  dependencies; dotask does not schedule a dependency graph or deduplicate calls.
- Optional execution returns `TargetExecutionStatus.Succeeded`, `NotFound`, or
  `Failed`. Child exit 1 is a failure, not absence. Pre-execution failures have a
  null exit code and an error diagnostic. Handle `Failed`; do not silently skip
  malformed/ambiguous targets. Cancellation and CLI/transport failures still throw.
  Use optional execution directly rather than checking presence and then running.
- Use `--init` for project setup; bare `init` remains a task name. No built-in
  `validate`, JSON help API, automatic `ValidateAsync`
  hook, service management, or generated configuration properties exist yet.
  `<capability>` is descriptive; it does not install features.

## Manage shared project tasks safely

In the DoTask source repository, `shared-tasks/` is the canonical source of
reusable tasks; `.tasks/` contains the copies used by the repository itself.
Keep the `dotnet/format.cs`, `dotnet/install.cs`, `dotnet/pack.cs`, and `git/check.cs` copies
identical when updating them, and regenerate the catalog. The standard formatter
formats the solution and C# files throughout the selected tasks directory;
`--verify` checks both without running optional `_/text/fixeol`.
Formatting changes to tracked shared copies are local edits for sync purposes.
`pack` creates local NuGet packages; it does not upload them.

Read [shared tasks](SHARED-TASKS.md) before adding, syncing, or removing them.
Use explicit selections such as `dotask --add "dotnet/{build,run,format}"`.
Commit `.dotasks.yaml`, `.dotasks-lock.yaml`, and installed `.tasks/` files.
Sources are `_` or `private-tasks`; other remote sources are deferred.

- Run management `--dry-run` to inspect an update plan. Cache downloads are allowed;
  project files and tracking are unchanged. Review task source before executing it.
- Never delete tracking, rewrite hashes, or replace files to bypass a conflict.
  The original installed hashes protect local edits and handwritten files.
- A conflict exits 1 without prompting. Review the reported base/project/incoming
  comparison files. Do not use `--accept-merge` until the user-authorized merge
  has actually been completed; that flag records the new upstream baseline.
- `--remove` only removes unchanged tracked tasks, and checks all files first.
  It never removes handwritten tasks or modified shared copies.
- Private originals under the user's config directory are user data. Never treat
  them as disposable cache or upload them to the official repository.
- A fresh checkout executes its project copies without shared-task caches. SDK,
  packages, and other tool prerequisites still apply.
- An incomplete transaction blocks execution. Preserve recovery backups and let
  the next management operation recover; do not delete state to hide an error.

## Verify and report

For a new or changed target:

1. Run `dotask help TARGET` and inspect the displayed metadata and any metadata
   or default-value errors. Confirm each option's name, alias, type, choices, and
   effective default. Help does not check compilation or execution requirements.
2. Exercise the target using inputs appropriate to its effects and the user's
   request. Include a normal call, an override/alias, and a relevant rejected
   value when useful. Use `name=` for an intentional empty string. Management `--dry-run` only previews add/sync/remove. For task execution, only
   use a preview/verify parameter when that task declares one.
3. If the target calls others, verify the sequence, explicit parameter forwarding,
   outputs, and failure propagation. Repeated calls repeat their effects.
4. For new completion metadata, load the documented shell adapter and check an
   option name and value. Target completion does not compile or load project configuration, so it cannot
   substitute for the preceding checks. Internal `__complete`/`__exec` commands
   are not a public integration contract.
5. Report the exact working directory, commands, exit codes, and relevant outputs
   or changed files. Distinguish metadata-only help from compilation and execution.
   State which operating systems and shells were exercised; Linux tests alone
   do not prove Windows or macOS behavior.

For tasks that publish, deploy, release, or delete data, verify the portions
authorized by the user and identify any unexecuted external effects in the
handoff. Do not claim those operations succeeded based on help output.

## Reusable instructions for a consuming project's AGENTS.md

The following block can be adapted into an existing project's agent instructions.
Replace `.tasks` if that project uses another directory, and add its real
verification commands. It does not replace the project's existing workflow.

```markdown
## Project tasks (dotask)

- Tasks live recursively under `.tasks`; each relative path defines a target. Read the task
  source and `.dotasks.yaml` before changing or running it.
- Shared files use `<source>/<group>/<task>.cs`. Use source-qualified paths in
  `ExecTargetAsync`, `TargetExistsAsync`, and `ExecTargetIfExistsAsync`. Exact names
  win; ambiguous shortcuts fail. Do not add `default-group` or an imports list.
- Use `dotask` or `dotask help` for project details, shared settings, and targets.
  `dotask --help` shows only CLI usage and does not inspect this project.
- Use `dotask help TARGET` to inspect its XML options and effective defaults.
  Help never compiles, restores, or executes tasks. Exit 0 can include individual
  metadata/default errors and does not prove the target compiles.
- Metadata belongs on an explicit entry-point class or static Main. Use
  `<summary>`, `<option name="..." alias="c" ...>`, and `<requires ... />`.
  Top-level-statement comments do not define task options in this preview.
- Declare shared companions in XML: `<requires task="group/task" />` and
  `<requires file="group/_support/Helper.cs" />`, relative to the same source.
  These control copying, not execution; no `.task.json` sidecar is used.
- Use `BuildContext.Current`, `project.Config`, and `project.Parameters`.
  Read typed keys with `Get<T>`/`GetPath`; YAML does not generate C# properties.
- Keep project-specific settings in YAML; no task registration is required.
  Top-level `name`/`description` are display metadata, outside `project.Config`.
  Explicit parameters override YAML target defaults, which override XML defaults.
- Call processes with `RunAsync(executable, arguments)` using separate unquoted
  arguments. Use `project.Path` for paths and `ExecTargetAsync` for other targets.
  Nested calls need explicit parameter forwarding and run every time.
- `TargetExistsAsync` queries current names without compiling or executing.
  `ExecTargetIfExistsAsync` returns `Succeeded`/`NotFound`/`Failed` with exit code
  and error details. Only `NotFound` is safe to skip; handle `Failed` explicitly.
- Preserve cross-platform behavior. `project.OS` is the host, not an output-OS
  parameter. Use filesystem helpers instead of assuming a particular shell.
- Validate according to each task's effects and the user's authorization.
  Management `--dry-run` only previews add/sync/remove, not task execution.
  There is no automatic validate command. Report exact commands and hosts
  tested; successful help alone is not successful execution.
```

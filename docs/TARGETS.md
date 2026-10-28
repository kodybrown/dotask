# Writing targets

This is the authoring contract for the current preview. See [usage](USAGE.md) for
CLI behavior and [AI assistant guidance](AI-ASSISTANTS.md) for a task-writing
workflow. A target is a C# file-based application, a Rust `.rs` entry file using
the bundled `dotask-sdk`, a Go `.go` entry file using `sdk/go`, or a declarative YAML `.task` group. The native CLI
supports nested calls between C#, Rust, Go and YAML tasks. Explicit `.cs`, `.rs`,
`.go`, and `.task` names identify implementations; extensionless names require an
unambiguous target. The native CLI reads
metadata directly; C# compilation uses the installed SDK and maintained authoring
library. See [Rust tasks](#rust-tasks) for the Rust source and SDK contract.

Jump to [a complete target](#a-complete-target), [reuse](#reuse-a-target),
[options](#option-metadata), [YAML](#yaml-configuration),
[context and nested calls](#context-and-target-calls),
[processes and files](#processes-and-files), or [requirements](#requirements-and-scope).

## Implementation identity and defaults

Files such as `tools/check.cs` and `tools/check.rs` may coexist. Invoke either
full path or an unambiguous shortcut, such as `check.cs`. `tools/check` is an
error when multiple implementations exist. The same rule applies to help,
completion, YAML group steps, existence queries, optional execution, and nested
calls. Ambiguity is a failure, never an absent optional task.

A single implementation retains its existing canonical `TargetName`; when
implementations coexist, the canonical names include their extensions. Cycle
detection distinguishes implementations and rejects a return to an already
active one. Case-only or space-grouped collisions within one language remain
errors. Requirements naming an ambiguous shared task must use `.cs`, `.rs`, `.go` or `.task`.

Project defaults under `targets.tools/check.defaults` apply to all variants.
Defaults under `targets.tools/check.cs.defaults`, `.rs`, `.go` or `.task` override the base values
case-insensitively. These are literal mapping keys, not nested settings paths.
CLI arguments override both layers. Help and the wizard use the same effective
values. There is no project-wide language preference.

`dotask tools/check --lang rust` selects the Rust implementation of that target;
`dotask help tools/check --lang csharp` selects its C# help. The selector is a
reserved CLI option, not a task parameter, and accepts equals syntax too. A
conflicting explicit extension is rejected. The filter applies only to the
requested target; nested calls resolve normally and can cross languages. YAML
groups have no C#/Rust/Go implementation language, so select them without this
filter and name their child implementations explicitly. Directory execution is
not implied by the selector.

See [Go tasks](#go-tasks) for the Go compiler, bundled SDK, support packages, and cache contract.

## Rust tasks

Save this as `.tasks/hello.rs`, then run `dotask hello --name Rust`:

```rust
// dotask: 1
// description: Print a greeting from a Rust task.
// options:
//   - { name: name, alias: n, default: Developer, description: Name to greet. }
// examples: [dotask hello --name Rust]
// end-dotask
use dotask_sdk::{BuildContext, Result};

fn main() { dotask_sdk::run(task); }
fn task(project: &BuildContext) -> Result<()> {
    println!("Hello from Rust, {}!", project.string("name")?);
    Ok(())
}
```

The same optional YAML header is used by C#, Rust and Go tasks. It starts with
`// dotask: 1` at the first nonblank line (a BOM or executable shebang is allowed)
and ends with `// end-dotask`. Every intervening line is an ordinary `//` comment.
Leading header indentation is preserved after stripping the comment prefix.
Empty headers are allowed. A header belongs to the file's one callable task;
ordinary documentation on classes/methods is independent of task metadata.

Keys are `description`, `remarks`, `examples`, `capabilities`, `options`, and
`requires`. Option keys are `name`, `alias`, `type`, `description`, `default`,
`required`, `choices`, and `completion`. Types are `string`, `path`, `bool`, `int`,
and `number`; completion is `file`, `directory`, or `none`. Choices are a sequence
of strings and defaults are scalar values. Requirements use `{kind: tool|setting|task|file|os,
value: ...}`. Unknown keys, duplicate/reserved options, unsupported versions,
invalid defaults and escaping support paths fail before compilation.

Task names come from paths and languages from supported extensions. Both C# and
Rust help/completion work without toolchains; missing executables leave tasks
visible with an unavailable notice in help. C# execution requires .NET SDK
10.0.300+ in the .NET 10 family, selected from the task directory; Rust execution
requires Rust 1.95+ and Cargo. Nothing installs dependencies automatically.
The header convention can extend to `#` comments for Python later; Python and
other additional execution handlers remain unimplemented.

Execution uses Rust 1.95+/Cargo and the bundled `sdk/rust/` sources. Generated manifests,
task/SDK snapshots, lockfiles, and build outputs are in a per-user external
temporary cache. Source and declared support bytes identify immutable snapshots;
Cargo handles build locking and compiler changes. Source-local `Cargo.toml`,
`Cargo.lock`, and `target/` are never created. SDK dependencies are pinned; the
first build may restore crates. Arbitrary per-task crate dependencies are not
currently supported. A normal `fn main()` without metadata is also executable.

Use `#[path = "_support/Helpers.rs"] mod helpers;` for a module next to a root task,
and declare `requires: [{ kind: file, value: _support/Helpers.rs }]`. Requirement
paths start at the task root, whereas Rust module paths start at the entry file;
declare every required source or compile-time included file. Missing sources fail
compilation. Helpers remain excluded from task discovery beneath `_support/`.

`dotask_sdk::run` loads `BuildContext` and preserves `ProcessFailure` exit codes.
Context exposes `root`, `invocation`, `task_file`, `task_name`, `path`, `os`,
`architecture`, `parameters`, and `settings`. `string` and `boolean` read typed
parameters; `parameter` and `setting` return JSON values, with case-insensitive
dotted setting lookup. `setting_path` resolves a portable path from the root.
For example, `project.parameter("count")?.as_i64()` reads an integer option.

Build a standard `Command` using `project.command("tool")`, add exact argument
tokens with `.args(...)`, then call `execute` to inherit streams or `capture` to
read UTF-8 stdout. Both preserve process failures; the CLI owns cancellation and
descendant cleanup. Calls use `exec_target("name", json!({...}))`, `target_exists`,
or `exec_if_exists`. The optional reply contains `Exists`, `ExitCode`, and `Error`;
only absence is `Exists: false`. Both languages share immutable configuration
snapshots and cycle detection. `create_installer` returns the existing JSON
artifact contract; `installer_artifact` constructs an executable artifact for
the current host, and `set_installer_result` returns it to the invocation's caller. `run_installer` launches a returned artifact with exact argument tokens.

The reusable `_/rust/test` runs an optional project `verify` task after successful
Rust build/tests. Missing verification is skipped; failures and ambiguity are
errors. The hook must contain extra checks and must not call `test` recursively.
A wrapper can pass `verify: false`, then invoke its own required verification
with explicit project parameters. See the [Rust group](../shared-tasks/rust/README.md).

## Go tasks

Go entry files use `.go`, `package main`, and the same ordinary-comment YAML
header as Rust/C#. Run a task with `dotask hello.go` or `dotask hello --lang go`.
The selector applies only to this invocation; nested calls resolve normally.
Coexisting `.cs`, `.rs`, `.go`, and `.task` entries require an explicit extension
when their base name is ambiguous. Help, completion, requirements, and effective
defaults use the same catalog without invoking Go.

```go
// dotask: 1
// description: Print a greeting from Go.
// options: [{name: name, default: Developer}]
// end-dotask
package main

import (
    "fmt"
    dotask "dotask.local/sdk"
)

func main() { dotask.Run(task) }
func task(c *dotask.BuildContext) error {
    name, err := c.String("name")
    if err != nil { return err }
    fmt.Println("Hello,", name)
    return nil
}
```

Execution requires a stable, locally installed Go 1.22+ compiler. Dotask forces
`GOTOOLCHAIN=local`, disables module-network access, and never downloads a Go
toolchain or invokes Rust/Cargo/.NET to execute a Go task. Missing/unsupported
compilers fail clearly on execution. Help works without Go; it only checks tool
availability, not compiler compatibility or task syntax.

The bundled `sdk/go` module uses only the standard library. Initial task imports
are limited to the standard library, `dotask.local/sdk`, and declared local task
support packages. Import local packages as `dotask.local/task/_support/message`
and declare **every** required source/data file through `requires: [{kind: file,
value: _support/message/message.go}]`. Requirement paths start at the task root;
shared catalog declarations are group-relative, as with Rust. Imports are rooted
at the entry file directory, so an adjacent `_support/message` package keeps the
same import when a shared task is installed beneath any source/group prefix.
Declared support outside the entry directory retains its task-root-relative path
in the snapshot; conflicting destination paths are rejected. The
[Go example](../examples/go/README.md) demonstrates this convention. Only the
selected entry file is built, so neighboring tasks can each declare `main`.
Support packages must use ordinary Go package directories; arbitrary consumer
modules/dependencies and Cgo are not supported. Consumer module/workspace files
cannot be supplied as support files.

Compilation uses private content-addressed task/SDK snapshots and generated
modules under the external temporary `_go/dotask/tasks/<user>` cache. Consumer
`go.mod`, `go.sum`, `go.work`, environment defaults, external cache helpers, and build flags do not
participate and are never rewritten. Cgo is disabled, host OS/architecture are
selected explicitly, and builds use readonly modules, trimpath and no VCS stamps.
Cache identity includes source identity/bytes, support files, the complete SDK,
compiler version/location/target metadata, generated manifests, and controlled
build settings. Per-snapshot locks serialize concurrent builds; binaries are
published only after success. Failed/cancelled compilations can be retried.

`BuildContext` provides `Root`, `TaskDirectory`, `Invocation`, `TaskFile`,
`TaskName`, `OS`, `Architecture`, and a cancellation `Context`. `Path` resolves
explicitly portable paths; it rejects NUL and foreign Windows roots on Unix.
`String`, `Bool`, `Int` (int64), `Number`, `Parameter`, and `Setting` read typed
values. `ParameterAs[T]`/`SettingAs[T]` decode additional typed values without
losing integer precision. Names/settings use case-insensitive dotted lookup.
The CLI's existing `int` option remains a signed 32-bit integer; typed settings
can carry larger integers. `SettingPath` combines setting lookup and paths.

Use `c.Command("tool", "argument with spaces", "")`, then `Execute` or `Capture`.
Arguments remain separate tokens, streams behave like the Rust SDK, and capture
requires UTF-8. Wrapped `ProcessFailure` errors retain their exit code through
`Run`. The native CLI owns cancellation and descendant cleanup; cooperative Go
code can also observe `c.Context().Done()`.

Nested APIs are `ExecTarget`, `TargetExists`, `ExecIfExists`, and
`CreateInstaller`. Pass a named mapping or a Go struct with explicit JSON field
names. `ExecIfExists` returns `OptionalResult`: only absence has `Exists=false`;
ambiguity/errors remain failures. Settings snapshots and implementation-aware
cycle detection are shared with the other languages.

Installer APIs are `InstallerArtifact`, `ValidateInstaller`, `SetInstallerResult`,
`RecordInstaller`, `FindInstaller`, and `RunInstaller`. They use the existing
artifact wire format. Recording, discovery and launching delegate to the native
CLI to preserve locking, atomic state replacement, hash checks, unknown-state
protection and Windows activation. Record only after all packaging succeeds;
returning an invocation result is a separate operation. Use
`CreateInstaller("_/doinstall/create-installer.task", parameters)` with the
precompiled creator shipped beside dotask. Native helper and declarative assembly
steps require no Rust/Cargo/.NET; the recipe's own tasks determine other tool
requirements. No doinstall tools are bundled in dotask. See
[installation](INSTALLATION.md#self-extracting-build-and-gather-workflow).

The reusable [Go installer group](../shared-tasks/go/README.md) provides
`go/create-installer.go` and `go/stage-installer.go`. Configure `settings.go`
for the native package, executable, literal version source, expected version
output and explicit payload files. Project wrappers delegate through the Go SDK;
the native creator owns successful artifact recording.

## Target names

Tasks are discovered recursively under `.tasks`. Their project-relative paths
without `.cs`, `.rs`, `.go`, or `.task` are their full names. Directory/file identifiers start with an ASCII
letter and then use letters, digits, `_`, or `-`; matching is case-insensitive.
The reserved official source directory `_` is allowed at the task root.

| File below `.tasks/`              | Full name                      | Available shortcuts                                             |
| --------------------------------- | ------------------------------ | --------------------------------------------------------------- |
| `build.cs`                        | `build`                        | Exact project orchestrator                                      |
| `_/dotnet/build.cs` | `_/dotnet/build` | `dotnet/build`, `build`, and legacy `dotnet-build`, when unique |
| `private-tasks/tools/check.cs`    | `private-tasks/tools/check`    | `tools/check`, `check`, and `tools-check`, when unique          |
| `dotnet run.cs`                   | `dotnet-run`                   | Legacy `run`, when unique                                       |

Exact names take precedence. Ambiguous shortcuts fail and suggest fully qualified
names. Namespaces come from directories, not header fields or C# class names. There is no
`default-group`. A top-level `.tasks/build.cs` can coordinate several ecosystems.
Old `<group> <target>.cs` names with exactly one space remain supported; plain
hyphenated filenames retain their original exact identities. Duplicate full names
are errors. `help`, `completion`, `__complete`, and `__exec` are reserved top-level
names and never become basename aliases.

The shortest unambiguous name appears in help; completion offers all available
names. The canonical full name keys YAML defaults, `project.TargetName`, and cycle
detection. **Use source-qualified names in calls between shared tasks**, including
`TargetExistsAsync` and `ExecTargetIfExistsAsync`, so adding another source cannot
change dependencies or make them ambiguous.

Hidden/underscore-prefixed entries, `bin`, `obj`, `node_modules`, and symlinks are
excluded from discovery, except for the task-root `_` directory for official tasks. Put helper C# files under `_support`, and reference them
with source-relative `#:include` directives. See [shared task distribution](SHARED-TASKS.md)
for installing copies, support-file declarations, and protected synchronization.

## Precompiled task adapters

A .task file may select a shipped task helper instead of steps:

    description: Run configured installer build steps and gather the payload.
    native-helper: doinstall-create-installer

Only doinstall-create-installer is supported. It maps to the adjacent native
dotask-doinstall-create-installer[.exe] in the actual dotask installation.
The adapter may declare the ordinary options, requirements, remarks and examples;
it cannot declare steps or require_at_least_1_step. Unknown helper identifiers
and path/command substitutions are metadata errors.

Execution binds options and target settings exactly as for a code task, then
supplies DOTASK_EXECUTION_CONTEXT to the precompiled helper. The CLI preserves
session-local InstallerArtifact results, nested calls, cycle checks, cancellation,
and child failure codes. It never invokes Cargo or .NET for this adapter.
Help and completion read metadata without requiring the helper to be present.
A missing helper fails execution with guidance to restore the complete dotask
distribution. See the [installer workflow](INSTALLATION.md#self-extracting-build-and-gather-workflow).

## YAML task groups

Groups may call Rust, C# and Go tasks. A `.cs`, `.rs`, `.go`, or `.task` file
may share a base name with another implementation; use the extension-qualified
name when calling it from a group. Extensionless ambiguity remains an error.

Create `.tasks/check.task` to compose existing tasks without writing C#:

```yaml
description: Check the project.
require_at_least_1_step: true
steps:
  - run: _/dotnet/check
  - run: _/git/check
  - run: _/dotnet/test
    optional: true
    with:
      configuration: Release
```

Run it with `dotask check`; `dotask help check` shows its description, source,
ordered steps, parameters, and execution requirement without executing children.
Groups use the same naming, shortcuts, completion, and cycle detection as code
tasks. Coexisting `check.cs` and `check.task` require extension-qualified calls.
Groups may call groups, C# or Rust targets, and SDK target-call helpers can call
or query groups.

The file contains one YAML mapping. Keys are case-sensitive; unknown keys,
duplicate keys (including case variants), invalid types, and multiple documents
are errors. `description` is an optional string; `steps` is a required sequence
and may be empty (`steps: []`). Each step requires exactly one `run`, `exec`, `file`,
`http`, `json`, `text`, or `foreach` action. A `foreach` also declares nested `steps`.
Top-level `options`, `requires`, `remarks`, `examples`, and `capabilities`
use the same metadata contract as C#, Rust and Go headers.

- Steps execute sequentially in listed order, including repeated calls.
- `optional` defaults to `false`. A missing required target fails; a missing
  optional target is silently skipped. Ambiguity, invalid metadata, invalid
  parameters, compilation errors, and execution failures always fail the group.
- The first failure stops subsequent steps; child exit codes are preserved.
- `require_at_least_1_step` defaults to `false`. When true, executing zero steps
  fails. When false, an empty or entirely skipped group succeeds silently.
  Invoking a child group counts as one step even if that child executes none.
- `with` is an optional mapping of child parameter names to scalar strings,
  booleans, or numbers. Quoted strings stay strings, including values such as
  `'007'`. Child option binding validates/converts values using the child's
  declared types; explicit values override that child's YAML and header defaults.
  Exact references may pass parameters, settings, environment variables, and
  earlier step results. Nulls, sequences, and other mappings are rejected.

Parameters are passed through the normal named-argument binding; values are not
shell commands or expressions. Declarative tasks can declare typed options, but
parent arguments are never automatically forwarded to `run` children. Explicit
references can forward selected values. Use code
tasks for dynamic orchestration.
Names resolve against the project catalog, not relative to the group file;
prefer source-qualified names such as `_/dotnet/test`. Execution never downloads
missing targets. Shared-task add/sync/catalog distribution supports C#, Rust and
YAML `.task` files; reuse preserves the same ownership and hash protections.

See [native YAML workflows](YAML-WORKFLOWS.md) for HTTP GET/pagination, JSON
filtering/projection, sequential loops, text formatting, frontmatter updates,
file actions, named results, boolean result guards, process environment, captured
output, and timeouts. The [GitHub project-page example](../examples/github-projects/README.md)
combines these capabilities without a code task or domain-specific command.

## Direct process steps

A declarative task needs no C# or Rust toolchain to execute processes. Example:

```yaml
description: Check Git, optionally checking whitespace.
options:
  - {name: whitespace, type: bool, default: false}
steps:
  - exec:
      executable: git
      arguments: [--version]
      cwd: .
  - when: {parameter: whitespace}
    exec:
      executable: git
      arguments: [diff, --check]
```

`exec` contains `executable`, optional `arguments` (a sequence, default empty),
and optional `cwd` (default `.`). Omitted/relative cwd resolves from the project
root, independently of the invocation directory and task-file location. A bare
executable uses PATH; explicitly portable executable paths resolve from the
project root. Only executable paths and cwd are normalized; argument strings
retain backslashes, quotes, empty strings, newlines, and option-like values.

Basic values are literal strings or exact references `{parameter: name}` or
`{setting: dotted.name}`. Parameter references use declared option names, not
aliases. References resolve scalar strings, numbers and booleans to one argument;
no interpolation, splitting, expression evaluation, or implicit shell occurs.
For example, `arguments: [{parameter: message}]` preserves a message with spaces.
Use a setting reference for configured executable/cwd paths. Missing/non-scalar
references and invalid cwd fail before starting that process.

`{path-parent: VALUE}` and `{path-name: VALUE}` resolve an existing path from the
project root, canonicalize it, then return its parent directory or final name.
VALUE can be a literal path or an exact reference; nested path operations are
limited to 16 levels. Missing paths and roots without the requested component
fail before execution. These operations preserve native filenames and do not
introduce string interpolation or general expressions. Plain argument values
remain unchanged.

`when: {parameter: enabled}` may guard any action and must reference a declared
boolean option. False skips the step without invoking its process or target.
Conditions are simple references, not expressions. Executed process steps count
toward `require_at_least_1_step`. `optional` and `with` are only for `run` actions;
a missing executable or process failure cannot be silently skipped.

Processes inherit stdin/stdout/stderr, run sequentially, preserve nonzero exit
codes, stop later steps on failure, and use the native process-tree cancellation
path. Help/completion read metadata and never launch a command. To run shell
syntax or a script, explicitly invoke its interpreter and provide argument-list
entries; Windows `.cmd` tools need an explicit `cmd` invocation. Dotask does not
introduce a shell or bundle interpreters.

### Staged process file output

An `exec` step can declare `output-file` to publish a new file only after the
command succeeds. Use `{output-file: true}` as an argument to pass the temporary
filename to the producer:

```yaml
options:
  - {name: source, type: path, required: true}
  - {name: output, type: path, required: true}
steps:
  - exec:
      executable: 7z
      cwd: {path-parent: {parameter: source}}
      arguments: [a, -tzip, -y, {output-file: true}, --, {path-name: {parameter: source}}]
      output-file:
        path: {parameter: output}
        outside-directory: {parameter: source}
        suffix: .zip
```

`path` is required and resolves from the project root. Optional
`outside-directory` must resolve to an existing directory and forbids outputs
inside it, including through linked parent directories. Optional `suffix`
defaults to empty and uses portable filename characters (letters, digits, `.`,
`-`, `_`); it affects only the temporary filename, not the final destination.
The output reference is valid only as a direct argument of that process and
must appear at least once. Help/completion only validate metadata.

Existing destinations are refused. Parent directories are created after
validation, and staging lives on the destination filesystem. Exit zero must
produce an ordinary file at the supplied temporary path. Dotask publishes it
without replacing a concurrently created destination. A missing/invalid output
or publication failure fails the task. Process failures preserve their exit
codes and leave no final output; staging is cleaned on normal completion and
handled failures. Forcefully terminating the CLI can leave temporary staging.

This is general process output handling; the producing application owns the
contents and validation of its file. Filesystems must support hard links.

### Reusable process tasks

`git/check.task` is the only reusable Git check. Its default runs `git --version`;
`--whitespace` also checks unstaged and staged diffs. Invoke it without `--lang`
because YAML has no C#/Rust/Go implementation language.

The reusable `archive/create-zip.task` and `archive/create-7z.task` use these
features to call a configurable `7z` executable with no language toolchain.
They retain the source directory as the archive's top-level folder, require a
new destination outside that directory, and publish only successful output.
Supply `--source`, `--output`, and optionally `--archiver` (command name or path).
Replace tracked Rust implementations explicitly with shared `--remove` and
`--add`; do not invoke these YAML tasks with `--lang rust`.
They require a dotask CLI supporting path references and staged process output.

## A complete target

Create `.tasks/` in your project's root and save the following as `.tasks/dotnet/run.cs`:

```csharp
// dotask: 1
// description: "Run the application."
// options:
//   - {"name": "configuration", "alias": "c", "type": "string", "choices": ["Debug", "Release"], "default": "Debug", "description": "Build configuration."}
// requires:
//   - {"kind": "tool", "value": "dotnet"}
//   - {"kind": "setting", "value": "application"}
// examples: ["dotask run -c Release"]
// end-dotask
using DoTask;


public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var config = project.Config;
    await project.RunAsync("dotnet", [
      "run", "--project", config.GetPath("application"),
      "--configuration", project.Parameters.Get<string>("configuration")
    ]);
  }
}
```

Create `.dotasks.yaml` in the project root, replacing the application path with
your project's real `.csproj` path:

```yaml
version: 1
settings:
  application: src/MyApp/MyApp.csproj
```

From that project root, run `dotask help run`, then `dotask run -c Release`
(or `dotask dotnet/run -c Release`).
The first command displays source metadata and checks effective defaults; the
second compiles the target and launches the application. Help does not check
whether the target compiles. No target registration, separate task `.csproj`, DI
container, or library package directive is needed. Both files can be created in
any editor. [The basic example](../examples/basic/README.md) is runnable without
creating an application first.

The class name is arbitrary. Put the common YAML header before code and SDK
directives. The header describes the file's one task, including top-level
statements; helper methods are not independently callable targets. Ordinary
C# XML documentation can still describe code, but is not task metadata.


Use one explicit entry-point class with a static `Main` per target. Standard
synchronous or asynchronous entry points are supported, including `Task<int>`
when the target needs to return a status. A target without metadata is valid and
appears as `(no description)`. Top-level statements work for execution, but this
version does **not** extract metadata attached to them. Use explicit `Main` for
self-describing tasks. Parameters come from `project.Parameters`; dotask does not
forward its command-line arguments to `Main(string[] args)`.

The library reference and a small startup helper are attached during compilation.
Task source paths are normalized before matching the SDK project, including
Windows paths supplied with forward slashes. Referenced projects retain their
own build properties and targets.
Use `using DoTask;` to import the library namespace. Existing target files using
the earlier `using Dotask;` spelling must update that import. C# namespace names
are case-sensitive. Rebuild dotask and reinstall any packaged preview when
switching namespaces. Targets do not need a package directive for `Dotask.Library`.
Native SDK directives such as `#:package`, `#:project`, and `#:include` are supported.
Included files remain relative to the original target. Put shared `.cs` files
under an underscore-prefixed subdirectory such as `_support` so they are not discovered as targets.

Discovery accepts `.cs`, `.rs` and `.go` executable targets and `.task` YAML groups. Files such as `.txt`, `.md`, `.target`, `.targets`, `.yaml`, `.yml`, and
`.json` are never targets, even when stored alongside tasks. Future `.py` or `.go`
support would require explicit language handlers, not execute arbitrary files.
For example, this repository stores Windows native bootstrap staging in
`.tasks/misc/prepare-bootstrap.ps1`; it does not appear in task help or completion.

For example, put `#:include _support/Helpers.cs` before `using` directives to include
a helper source file. Keep project-specific paths in YAML so the same target can
be copied between projects. Run targets through dotask; compiling/running them
directly does not supply the library and ambient context.

## Reuse a target

Copy the target file into another project's `.tasks`, then supply the settings
it declares with `{kind: setting, value: ...}`. Copy any explicit includes or
task-local assets it uses as well. Run its help and verify an appropriate call
in the new project. The target file can remain identical between projects while
each project owns its configuration.

Use [shared-task management](SHARED-TASKS.md) to add and synchronize reusable
copies. For manual copying, preserve source/group/task paths and include required
companions. Calls between shared tasks should use paths such as
`_/dotnet/build`, including the source, to stay stable as other
sources are installed. Executing a target never fetches missing tasks.

The [official catalog sources](../shared-tasks/) provide reusable examples:

| Task                             | Dependencies / effect                                                                            |
| -------------------------------- | ------------------------------------------------------------------------------------------------ |
| `_/dotnet/build`   | `settings.solution`; requires `dotnet/restore`, then builds                                      |
| `_/dotnet/test`    | `settings.solution`; runs solution tests                                                         |
| `_/dotnet/format`  | `settings.solution`; formats or verifies; optionally calls the source-qualified text/fixeol task |
| `_/dotnet/check`   | Checks solution presence and SDK/MSBuild/formatter versions                                      |
| `_/dotnet/verify`  | Runs installed official check/test/format tasks; skips missing ones and stops on failure         |
| `_/dotnet/publish` | `settings.project`; publishes for the requested OS/architecture                                  |
| `_/installer/install` | Finds and runs an existing configured installer; offers guided setup if unconfigured |
| `_/doinstall/create-installer` | Runs YAML build steps and gathers their expected files into an installer |
| `_/doinstall/build` | Explicitly builds a configured external source checkout using doinstall's own Cargo tooling |
| `_/doinstall/assemble.task` | Directly invokes a separately supplied builder/runtime pair with resolved configuration and payload; no language toolchain |
| `_/doinstall/uninstall` | Runs the retained engine without rebuilding the application |
| `_/dotnet/pack`    | `settings.project`; builds local NuGet packages; no upload                                      |
| `_/dotnet/update-version.cs` | Utah-time major.minor.yyMM.ddHH preparation for one .NET project; preserves encoding and never builds or invokes Git |
| `_/git/check`      | Checks Git availability; optional `--whitespace` checks staged/unstaged diffs                    |

`install` requires no creation-task convention. Configure `settings.installer.installer_file`
as a literal path or version template. Creation tasks can explicitly record their
successful output separately from user configuration for first-run setup.
See [installer contracts and customization](INSTALLATION.md) for structured
results, `installer-args`, and overriding the default task. No application files
are copied directly by the shared task.

The build/test/format targets need the `dotnet` executable. The publish target
also uses `dotnet`; it creates publish output, not a public package release or
deployment. Adjust descriptions and declared options if you adapt a target's
behavior.

`dotask git/check` checks Git availability without requiring a repository.
`dotask git/check --whitespace` additionally runs `git diff --check` and
`git diff --cached --check`, stopping on the first failure and preserving its
exit code. These read-only checks find whitespace errors in tracked changes;
they allow a dirty tree and do not examine untracked files. The repository's
`verify-docs.rs` declares `requires: [{ kind: task, value: git/check }]` and calls
`project.exec_target("_/git/check", json!({"whitespace":true}))?` after
checking required documents. The declaration alone does not run the task.

`shared-tasks/` is the canonical authoring location in the DoTask repository.
The corresponding `.tasks/_/` files are runnable project copies. All installed
official C# task copies in this repository are kept byte-identical to their
canonical sources, enforced by the test suite. In consuming projects, shared-task synchronization still protects
local edits; this convention does not authorize overwriting modified copies.

`dotask format` runs the solution formatter (settings.format.solutions can select an explicit list), then checks/formats C# whitespace
throughout `project.TaskDirectory`, including other groups and custom `--use-dir`
locations. `--verify` adds `--verify-no-changes` to both steps and skips optional
line-ending normalization. Normal formatting can disable broad text normalization with --normalize-text false; otherwise it runs the optional
`_/text/fixeol` target only if installed and propagates its failures.
The reusable `text/fixeol.task` runs the `fixeol` app directly without C#/Rust/Go
tooling. It requires `fixeol` on PATH; a missing executable fails rather than
warning and skipping. Its `--eol lf|crlf` option defaults to LF for ordinary text;
`.cmd` and `.bat` always use UTF-8/CRLF. All three recursive passes retain the
existing file patterns (including `Makefile`) and exclude `.tasks`, generated
outputs and `.mcp_file_state.json`. Existing tracked C# copies require explicit
removal/addition with the updated CLI/catalog; sync does not migrate them.
The repository no longer has a separate root `format.cs` shadowing this task.
Task formatting follows the consuming project's `.editorconfig`. Changes to
tracked shared copies count as local edits and remain protected during sync.

`dotask pack` builds the project from `settings.project` in `Release` by default.
Use `-c Debug` to select Debug, `--output`/`-o` to choose a package directory
(default `artifacts/packages`, relative to the project root), and `--dotnet` to
select the CLI executable. Packing writes `.nupkg` files locally; publishing to
a package feed is a separate action.

`dotask _/dotnet/check` runs `dotnet --version`,
`dotnet msbuild -version -nologo`, and `dotnet format --version` from the project
root. SDK selection follows `global.json`. The target reports versions and fails
on an unsuccessful or empty version response. It accepts no target options and
checks that the file specified by `settings.solution` exists, without evaluating
the solution. It does not depend on other task files. It checks
the SDK and these bundled tools, not NuGet dependencies, workloads, or additional
global/local .NET tools. It does not test, format, restore, build, or publish the
consumer project. Run `dotask test` or `dotask format --verify` for those checks.

`dotask _/dotnet/verify` combines the available official
`dotnet/check`, `dotnet/test`, and `dotnet/format` targets in that order. It forwards
`--configuration`/`-c` (default `Release`) to the test target and `verify=true` to
the format target. Missing targets are reported and skipped, while any failure
stops the sequence. An executed child's exit code is preserved; a failure before
execution returns 1. If no verification targets exist, it fails rather than
claiming that verification passed. Its full-name calls are independent of other
groups or project entry points with the same short names.

Like every C# target, `check` itself must compile before it can execute. If no
suitable SDK is available to compile it, dotask reports that failure first. This
target is not a standalone SDK installer or bootstrap command. The .NET CLI
documents [SDK selection](https://learn.microsoft.com/en-us/dotnet/core/tools/global-json)
and [formatter version queries](https://learn.microsoft.com/en-us/dotnet/core/tools/dotnet-format).

### Guarded .NET tests

`dotask test` uses `settings.solution`, or `--solution PATH` to select a test
project/solution. `-c` defaults to Release. `--filter EXPRESSION` forwards one
literal dotnet-test filter argument; `--no-build` uses an existing build.
`--dotnet` selects the executable. Fresh workers (`--disable-build-servers`)
keep reused build servers from escaping the guarded process tree.

The shared task owns its support file and applies a 600-second command timeout
by default. `--timeout-seconds` accepts 1..86400. A named single-run mutex rejects
overlap; `--lock-name` selects a shared project identity, otherwise the project
root determines it. The mutex is released on success, failure and cancellation.
`--memory-limit-mb` defaults to 0 (no memory cap); a positive value caps the
complete process tree using a Windows Job Object or Linux systemd user scope.
Linux reuses a suitable pre-existing cgroup-v2 limit when present. A configured
cap fails visibly if the host cannot enforce it; macOS callers must currently
use 0. Timeout/cancellation stop descendants, and child exit codes survive.

Consumer-specific defaults belong in `.dotasks.yaml`, for example:

```yaml
targets:
  _/dotnet/test:
    defaults:
      timeout-seconds: 600
      memory-limit-mb: 4096
      lock-name: Example-TestRunner
```

A thin local test task may forward configuration/filter to `_/dotnet/test`.
It should not duplicate the guard or bypass it with direct dotnet test.
These settings apply to explicit shared calls as well as project wrappers.
The task does not impose a product's particular thresholds on other consumers.


### .NET calendar version preparation

`dotask update-version` refreshes `major.minor.yyMM.ddHH` in America/Denver.
Use `--increment-preview` to increment/start preview, `--increment-minor` to bump minor,
or `--increment-major` to bump major/reset minor. Minor/major clear preview; default
and `--increment` preserve it. Choose at most one increment; `--dry-run` previews.

The task accepts three- or four-component `Version` values, optionally followed
by `-preview.N`. It preserves explicit `AssemblyVersion` properties, updates an
existing numeric `FileVersion` to the new four-part numeric version, and updates
an existing literal `InformationalVersion` while preserving its custom metadata
suffix. Informational values beginning with `$(Version)` remain expressions;
the SDK can append Git revision metadata later. Other prerelease identifiers are
rejected. Preview counters must fit Int32, and incrementing its maximum fails.

Missing optional properties are not added: SDK defaults still apply. Declare
`AssemblyVersion` explicitly to keep assembly binding stable, and declare
`FileVersion` explicitly to track the calendar version independently of binding.
Conditional, duplicate or inherited properties that need updating are refused
before any write. Comments, encoding/BOM, whitespace and line endings survive;
no XML reserialization, build, restore, Git operation or lockfile edit occurs.

## Option metadata

| Field        | Meaning                                                 |
| ------------ | ------------------------------------------------------- |
| `name`       | Required full CLI name                                  |
| `alias`      | Optional single ASCII letter                            |
| `type`       | `string` (default), `bool`, `int`, `number`, or `path`  |
| `default`    | Default value, validated like command-line input        |
| `required`   | `true` requires a resolved value; default is `false`    |
| `choices`    | YAML sequence of allowed strings; matching ignores case   |
| `completion` | `file`, `directory`, or `none`; paths default to file completion |

`completion` changes suggestions only. Even `type: path` does not require that
the file/directory exists; validate existence in the target if needed. A
`type="string" completion="directory"` value remains a string without automatic
path normalization. `required: true` requires a resolved value, not necessarily
an explicitly supplied argument or a nonempty string. A default satisfies it.

Option element text is its description. Allowed syntax includes:

```text
--configuration Release
--configuration=Release
-c Release
configuration=Release
c=Release
```

A value may contain additional `=` characters. Use `name=` for an empty string.
Boolean flags support `--detailed`, `--detailed false`, and `detailed=false`; an
unspecified optional boolean is false. Repeated options (including alias/full-name
duplicates), unknown options, and invalid values fail before execution. Short
option bundles and positional arguments are not supported in this version.
Global options are reserved throughout the command line; use `name=VALUE` when
a literal value would otherwise look like a global option.

`--verbose` is reserved globally for dotask diagnostics and full project summaries;
do not declare a task option named `verbose`. A task may still use alias `-v`
for another option. Verbosity is inherited by nested calls without forwarding a
task parameter. See [verbose output](USAGE.md#verbose-output).

`int` is a signed 32-bit integer; `number` is a finite double. Choice values are
returned using their declared spelling. Path options are resolved relative to
the project root. Arbitrary strings retain their original contents.

| Declared type | Read the value with                                                                 |
| ------------- | ----------------------------------------------------------------------------------- |
| `string`      | `project.Parameters.Get<string>("name")`                                            |
| `bool`        | `project.Parameters.Get<bool>("detailed")`                                           |
| `int`         | `project.Parameters.Get<int>("retries")`                                            |
| `number`      | `project.Parameters.Get<double>("ratio")`                                           |
| `path`        | `project.Parameters.GetPath("output")` or `Get<string>("output")`; already absolute |

Read parameters by their full names; aliases are accepted when binding input,
but are not stored as additional keys. An absent optional non-boolean parameter
has no value; use `Contains` or `Get<T>(name, fallback)` when reading it.

The same definitions drive help, validation, and completion. Combined help
groups options when their names, aliases, types, choices, required flags, and
completion rules match. Different descriptions use a common summary description.
Only a shared default appears inline; target lists and differing default values
are omitted. Individual target help retains its exact description and default.

## YAML configuration

```yaml
version: 1
name: MyApp
description: Build, run, and publish MyApp across supported platforms.
settings:
  application: src/MyApp/MyApp.csproj
  output: artifacts
  deployment:
    retries: 3
    enabled: true
targets:
  _/dotnet/run:
    defaults:
      configuration: Release
    settings:
      project: src/MyApp/MyApp.csproj
```

`.dotasks.yaml` is optional. Its supported top-level keys are `version` (currently
1), `name`, `description`, `settings`, and `targets` with optional per-target
`defaults` and `settings` mappings. Defining a target here is never required.

`dotask --init` creates a starter configuration in the current directory with a
directory-derived name, empty description, and `settings: {}`, plus an empty
`.tasks/`. It preserves existing files. See the
[initialization reference](USAGE.md#initialize-a-project) for reruns, conflicts,
and custom task directories.

`name` and `description` are optional strings displayed by `dotask` and
`dotask help`. Leading/trailing whitespace is trimmed; missing, null, or blank
values count as absent. The name falls back to the project root directory's name,
and an absent description is omitted. YAML block strings can supply a multiline
description. Other value types are rejected. These are project display metadata,
not task settings: `project.Config` exposes resolved settings, not display metadata.
For example, `settings.name` is an independent value and does not set the project
name or any target's `name` parameter. CLI-only `dotask --help` does not read YAML.

Unknown top-level keys and case-insensitive duplicate keys are errors. Target
entries use full command names: `targets._/dotnet/run.defaults` configures
`.tasks/_/dotnet/run.cs`, even when invoked as `dotask run`. A `targets.run` entry applies
only to a plain `run.cs`; it does not configure the grouped file through its alias.
Defaults refer to full option names, and misspelled options are rejected for that target.
Defaults must be scalar strings, booleans, or numbers; arrays, objects, and nulls
are not parameter defaults. A YAML entry for a nonexistent target does not create
one. Keep entries aligned with the files you actually use.

Resolution order is target default, then project default, then explicit arguments.
Settings support mappings, arrays, strings, booleans, nulls, and numbers. Quoted
scalars remain strings. No code execution, environment substitution, templates,
or expressions occur while loading YAML.

Keep values shared by multiple tasks in top-level `settings`. Put task-specific
data under `targets.<full-name>.settings`, beside its option `defaults`. Both
local and shared tasks use this layout. Settings do not become CLI options.
The current task's `project.Config`, Rust SDK `settings()`/`setting()` accessors,
YAML `{setting: name}` references and `requires: [{kind: setting, value: name}]`
all use the same resolved settings view.

Settings resolve in this order: shared project values, full base-name target
settings, then extension-qualified target settings. Mappings merge recursively
with case-insensitive keys. Arrays, scalars and explicit nulls replace earlier
values; null masks an inherited value and is absent to required-value accessors.
Task-name keys are literal full identities, so `tools/check.cs` is one key, not
a dotted setting path. Shortcut entries do not configure grouped tasks.

Nested calls resolve the called task's own settings from the original shared
snapshot, without inheriting the caller's target overrides. Explicit forwarded
parameters remain separate and override the child's option defaults. Editing
YAML during a task does not change settings or defaults for that call chain.

```csharp
var project = BuildContext.Current;
var config = project.Config;
var app = config.GetPath("application");
var retries = config.Get<int>("deployment.retries");
var optional = config.Get("optionalSetting", "fallback");
```

Accessors support dotted nested keys and case-insensitive lookup. Missing/null
required values and incompatible types produce clear errors. `Contains(key)` is
false for a missing or null value; `Get<T>(key, fallback)` uses its fallback in
both cases. It does not hide a type mismatch. `Get<string>` can read scalar values
as text; numeric and boolean accessors require compatible values, so a quoted
YAML string `"3"` is not an integer for `Get<int>`. Avoid dots in literal YAML key
names: the accessor treats dots as nested-key separators.

Arrays and objects are also readable as compatible C# types. For example, a
setting `frameworks: [net10.0, net9.0]` can be read with
`config.Get<string[]>("frameworks")`. There is no array-index syntax in dotted
keys; read the array and index it in C#.

Configuration is read-only and snapshotted for the entire target call chain.
YAML does not create
C# properties or IntelliSense for individual keys in this version. Use
`config.Get<string>("message")`, not `config.Message`. `project.Config` exposes
the resolved shared and current-target `settings`; per-target defaults become
validated `project.Parameters`.

## Context and target calls

`BuildContext.Current` is an ambient context available while dotask runs the
target. It is not dependency injection, and target authors do not construct it.
Use `var project = BuildContext.Current; var config = project.Config;` in `Main`.

| Member                                                  | Meaning                                                                                 |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `RootDirectory`                                         | Discovered project root containing `.dotasks.yaml`, or the legacy task-directory parent |
| `InvocationDirectory`                                   | Original directory where dotask was invoked                                             |
| `WorkingDirectory`                                      | Default command directory: project root                                                 |
| `TaskDirectory`                                         | Selected tasks root, including a custom `--use-dir` location                              |
| `TargetFile`, `TargetDirectory`, `TargetName`           | Original source location and full command name (e.g. `_/dotnet/run`)      |
| `OS`, `Architecture`, `IsWindows`, `IsLinux`, `IsMacOS` | Actual host information                                                                 |
| `Config`, `Parameters`                                  | Read-only typed configuration and validated parameters                                  |
| `CancellationToken`                                     | Cancellation for the running target                                                     |
| `Files`                                                 | Filesystem helpers                                                                      |
| `CreateInstallerAsync(target, parameters, cancellationToken)` | Build and read the project's installer result; see [installer contract](INSTALLATION.md#installer-library-contract) |
| `SetInstallerResultAsync(artifact, cancellationToken)` | Return one installer artifact to the calling task |
| `RecordInstallerAsync(artifact, cancellationToken)` | Explicitly record successful output for later installation, without editing configuration |
| `RunInstallerAsync(artifact, arguments, cancellationToken)` | Validate and run the installer, using defaults unless arguments are supplied |
| `Path(...)`                                             | Resolve explicit portable path components against the root                              |
| `DirSeparator`, `InvalidFileChars`, `InvalidDirChars`   | Host path facts; directory-name characters, not complete path validation                |

`OS` is a `HostOS` enum (`Windows`, `Linux`, `MacOS`, `Unknown`); the runner rejects
unknown hosts before executing tasks. `Architecture` is
`System.Runtime.InteropServices.Architecture` for the operating system. Linux
distributions are not separate enum values. Use the `IsWindows`, `IsLinux`, or
`IsMacOS` convenience properties when behavior genuinely depends on the host.

`WorkingDirectory` always describes dotask's default project-root directory. It
is not a mutable CWD setting; set `ProcessDefinition.WorkingDirectory` for a
particular command. To use a task-local asset, resolve it explicitly:

```csharp
var template = project.Path(project.TargetDirectory, "templates", "message.txt");
var text = project.Files.ReadText(template);
```

The file in that snippet must exist next to the target under `templates/`.

```csharp
await project.ExecTargetAsync("_/dotnet/check");
await project.ExecTargetAsync("_/dotnet/publish",
    new { OS = "linux", Configuration = "Release" });
```

Calls run sequentially when awaited and execute each time. Parameters are
explicitly forwarded; the caller's options are not inherited. Configuration,
root, original invocation directory, cancellation, and cycle detection are
shared. Each target has its own source paths and validated parameters. Failures
propagate, and cycles report their call chain. Output OS parameters never change
`project.OS`.

Nested calls stay within the selected task directory. Target names use
the same naming rules as the CLI; both full and available short names resolve
to the same target, and cycles are checked using full names. Prefer source-qualified paths
in reusable tasks to keep dependencies stable. Parameter objects must
have named properties whose values are strings, booleans, or numbers; arrays,
nulls, and nested objects are not supported. A dictionary can express a name
that is not a C# identifier:

```csharp
await project.ExecTargetAsync("_/dotnet/publish",
    new Dictionary<string, object> { ["output-dir"] = "artifacts/release" });
```

That example requires `_/dotnet/publish.cs` to declare `output-dir`. Child header/project defaults
still apply to parameters you omit. An orchestration task must explicitly pass
its resolved configuration to each child; a caller's options are not inherited.

### Optional targets

```csharp
bool hasTests = await project.TargetExistsAsync("_/dotnet/test");

var result = await project.ExecTargetIfExistsAsync("_/dotnet/test",
    new { Configuration = "Release" });
switch (result.Status)
{
  case TargetExecutionStatus.NotFound:
    Console.WriteLine("No test target is configured.");
    break;
  case TargetExecutionStatus.Failed:
    throw new TaskException(result.Error ?? "Tests failed.");
  case TargetExecutionStatus.Succeeded:
    Console.WriteLine("Tests passed.");
    break;
}
```

`Task<bool> TargetExistsAsync(string target, CancellationToken cancellationToken = default)`
queries the current selected task directory through the CLI, so it is
asynchronous. It uses the same full-name, short-alias, and case rules as execution.
It does not compile, restore, run targets, or validate defaults/requirements.
It returns true for a matching file even if the target's metadata or C# is
invalid. Ambiguous names, duplicate full names, and unreadable task directories
raise errors instead of returning false. The answer is a point-in-time lookup;
later filesystem changes can change the result.

`Task<TargetExecutionResult> ExecTargetIfExistsAsync(string target, object? parameters = null, CancellationToken cancellationToken = default)`
performs one lookup and, if found, attempts the normal target execution. It
returns a structured result rather than encoding missing/error states as process
exit codes. Use it directly when you want to run an optional target; a preceding
existence check is unnecessary.

| `Status`                          | `ExitCode`                          | `Error`                                                                      |
| --------------------------------- | ----------------------------------- | ---------------------------------------------------------------------------- |
| `Succeeded`                       | 0                                   | null                                                                         |
| `NotFound`                        | null                                | null                                                                         |
| `Failed`, child process completed | Original observed nonzero exit code | Failure summary; child diagnostics also appear on the console                |
| `Failed` before execution         | null                                | Ambiguity, metadata, argument, requirement, compilation, or cycle diagnostic |

These statuses belong to `TargetExecutionStatus`. An existing task that exits 1
is `Failed`, never `NotFound`. A malformed or ambiguous target is not silently
skipped. Cancellation throws `OperationCanceledException`; invalid API arguments,
CLI launch/transport errors, and an inaccessible task directory also raise errors.
If the OS constrains a child's exit code, `ExitCode` is the value observed by
dotask. Both execution helpers preserve selected-directory context, full-name
defaults, explicit parameter forwarding, and cycle detection. Calls still repeat
on every invocation. `ExecTargetAsync` remains the strict, throwing alternative.

To preserve an executed child's exit code when propagating a `Failed` result,
throw `new ProcessFailedException(targetName, result.ExitCode.Value)` when that
value is present; use `TaskException` with `result.Error` for a failure before
execution. Always handle `Failed` explicitly, even if `NotFound` is acceptable.

## Processes and files

The following statements illustrate operations inside an async `Main`. The
build needs a project/solution at the root, Git needs a repository with `src/`,
the copy needs `input.txt`, and the ZIP destination must not already exist:

```csharp
await project.RunAsync("dotnet", ["build", "--configuration", "Release"]);

var result = await project.RunAsync(new ProcessDefinition
{
  Executable = "git",
  Arguments = ["status", "--short"],
  WorkingDirectory = "src",
  CaptureOutput = true,
  ThrowOnError = false
});

if (result.ExitCode != 0)
{
  throw new TaskException($"git status failed: {result.StandardError}");
}

project.Files.WriteText("artifacts/message.txt", "Hello");
project.Files.CopyFile("input.txt", "artifacts/input.txt");
project.Files.CreateZip("artifacts", "packages/build.zip");
```

Process arguments are passed separately without shell interpretation; spaces,
quotes, punctuation, and empty arguments are preserved. Standard output/error
are inherited by default. With `CaptureOutput`, they are returned on
`ProcessResult`. Nonzero exits throw `ProcessFailedException` by default.
Explicit executable paths are resolved relative to the selected working directory;
bare names use the platform's executable lookup. NUL-containing arguments are
rejected. Arbitrary arguments are never normalized as paths.

Pass one argument per list entry. Do not add shell quotes around entries or join
them into a command string. For example, `project.Path("src/My App/MyApp.csproj")`
is one argument even when its returned path contains spaces. When using
`CaptureOutput = true` with `ThrowOnError = false`, inspect `ExitCode` and handle
failure explicitly. Captured output is not also printed live.

| `ProcessDefinition` property | Default / behavior                                                                         |
| ---------------------------- | ------------------------------------------------------------------------------------------ |
| `Executable`                 | Required executable name or path                                                           |
| `Arguments`                  | Empty list; each entry is passed as a distinct argument                                    |
| `WorkingDirectory`           | Project root; an explicit relative path resolves against the root                          |
| `Environment`                | Inherited environment plus these overrides; a null value removes a variable for that child |
| `CaptureOutput`              | `false`; when true, return stdout/stderr as strings                                        |
| `ThrowOnError`               | `true`; nonzero exit throws `ProcessFailedException` with `ExitCode`                       |

`ProcessResult` contains `ExitCode`, `StandardOutput`, and `StandardError`.
Without capture, the two output strings are empty and the process uses the
console streams. Both `RunAsync` overloads, `ExecTargetAsync`, `TargetExistsAsync`,
and `ExecTargetIfExistsAsync` accept an
optional `CancellationToken`, linked with the context token. Pass
`project.CancellationToken` to other asynchronous APIs that support cancellation.

Shell built-ins, pipelines, and Windows `.cmd`/`.bat` scripts require explicitly
invoking their shell and following that shell's quoting rules. `RunAsync` does
not silently introduce a shell. This includes tools whose Windows installation
provides only a command-script launcher.

Portable path helpers accept `/` or `\` as separators and return absolute native
paths. Use `/` in YAML for readability. A Windows drive or UNC path is rejected
on non-Windows hosts; swapping separators cannot translate a drive mapping.
Literal backslashes in Unix filenames require direct `System.IO` calls because
the portable-path API deliberately interprets them as separators.

Helpers do not expand `~`, environment variables, or wildcards in paths, or prove
that a path exists or is writable. Use an absolute path from
`Environment.GetFolderPath` for a home directory. Paths can resolve outside the
project via `..` or an absolute path. The path API is not a filesystem sandbox or
a complete filename validator for every operating system.

All `project.Files` paths resolve against the project root using portable path
rules:

| Method                                                   | Behavior                                                                            |
| -------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `CreateDirectory(path)`                                  | Create parents as needed; return the absolute directory path                        |
| `FileExists(path)`, `DirectoryExists(path)`              | Check whether the corresponding entry exists                                        |
| `ReadText(path)`                                         | Read the entire text file                                                           |
| `WriteText(path, text)`                                  | Create parent directories; create or replace the file                               |
| `CopyFile(source, destination, overwrite = false)`       | Create destination parents; reject an existing destination unless overwrite is true |
| `FindFiles(directory, pattern = "*", recursive = false)` | Enumerate absolute paths using .NET filename search patterns, not shell expansion   |
| `DeleteFile(path)`                                       | Delete the file; missing files are allowed                                          |
| `DeleteDirectory(path)`                                  | Recursively delete if present; reject the project root and filesystem root          |
| `CreateZip(sourceDirectory, destinationFile)`            | Create destination parents and a new ZIP; destination must not already exist        |

Put ZIP destinations outside the source directory being archived. Deletion's
root checks do not prevent deleting other important directories; choose the
specific generated-output path intended by the task.

## Requirements and scope

Declare requirements in the file header, one `{kind, value}` mapping per item:

```yaml
requires:
  - {kind: tool, value: git}
  - {kind: setting, value: application}
  - {kind: os, value: 'windows,linux'}
  - {kind: task, value: dotnet/restore}
  - {kind: file, value: dotnet/_support/Helpers.cs}
capabilities: [network]
```

Tool, setting, and OS requirements are checked before executing a selected task.
Tool declarations check availability, not versions. Settings check presence and
nonnull values, not their types or paths. Dotted setting names are supported.
Help reports metadata but does not execute requirement checks.

Task/file requirements declare companions for shared distribution. IDs are
same-source `group/task`; files use portable paths relative to that source's
root. They do not execute targets or schedule tasks. Keep nested calls explicit
with `await project.ExecTargetAsync("_/dotnet/restore")`. C# helper compilation
uses task-file-relative `#:include _support/Helpers.cs`; Rust support sources
must also be declared so they enter the immutable compile snapshot.
Capabilities are informational and grant no permissions. No `.task.json`
sidecar or automatic validation lifecycle is supported.

Targets are trusted code with the same privileges as their caller. Running a
target can execute referenced MSBuild/package logic during SDK restore/build.
Help and completion never restore, build, or execute targets. Services, remote
execution, dependency deduplication, incremental task
skipping, and parallel scheduling are deferred.

## Interactive group creation

Run `dotask --create-task` in an initialized project to create a `.task` group.
Use `--use-dir PATH` to select another existing task directory. The wizard
requires interactive input and output; `--create-task --help` works anywhere,
including scripts. For automation, write the YAML file directly.

1. Enter a task name (letters, digits, underscores, and hyphens, starting with a
   letter; no extension or directories) and an optional description.
2. Search installed task names/descriptions, select a numbered result, and choose
   whether that step is optional. Stored references use canonical full names.
   Use `:manual` at the search prompt to enter an absent task by name.
3. For installed tasks, inspect option descriptions, types, choices, and effective
   defaults. Enter a value to override a parameter; Enter keeps its default and
   omits it from `with`. Required parameters without defaults must be supplied.
   Enter `:empty` to supply an explicit empty string. Invalid values are reprompted.
   For absent tasks, enter parameter names and types manually; their compatibility
   cannot be checked until the task exists. Supported manual types are `string`,
   `bool`, `int`, and `number`.
4. Add more steps, optionally reorder them by listing their numbers, and select
   whether at least one step must execute (default no).
5. Review the complete YAML preview and explicitly confirm save (default no).

At any prompt while adding a step, enter `:back` to discard that unfinished step
and return to `Add a step?`. Previously completed steps, the name, and description
are preserved. Answer `n` there to continue to ordering and preview. This also
works during manual task/parameter entry. `:cancel` and Ctrl+C still cancel the
entire wizard.

Task selection and validation read metadata only. The wizard does not execute,
compile, restore, download, or install targets. It validates the generated file
with the same `.task` parser used by discovery. It never overwrites an existing
file or a conflicting C# target. Files are created only at final save; `:cancel`,
Ctrl+C, end of input, or declining save leave no task file. Existing YAML editing
is deliberately left to your editor, preserving comments and formatting.

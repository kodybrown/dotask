# Writing targets

This is the authoring contract for the current preview. See [usage](USAGE.md) for
CLI behavior and [AI assistant guidance](AI-ASSISTANTS.md) for a task-writing
workflow. A target is either a C# file-based application compiled with the helper
library attached, or a declarative YAML `.task` group that calls existing targets.

Jump to [a complete target](#a-complete-target), [reuse](#reuse-a-target),
[options](#option-metadata), [YAML](#yaml-configuration),
[context and nested calls](#context-and-target-calls),
[processes and files](#processes-and-files), or [requirements](#requirements-and-scope).

## Target names

Tasks are discovered recursively under `.tasks`. Their project-relative paths
without `.cs` or `.task` are their full names. Directory/file identifiers start with an ASCII
letter and then use letters, digits, `_`, or `-`; matching is case-insensitive.
The reserved official source directory `_` is allowed at the task root.

| File below `.tasks/`              | Full name                      | Available shortcuts                                             |
| --------------------------------- | ------------------------------ | --------------------------------------------------------------- |
| `build.cs`                        | `build`                        | Exact project orchestrator                                      |
| `_/dotnet/build.cs` | `_/dotnet/build` | `dotnet/build`, `build`, and legacy `dotnet-build`, when unique |
| `private-tasks/tools/check.cs`    | `private-tasks/tools/check`    | `tools/check`, `check`, and `tools-check`, when unique          |
| `dotnet run.cs`                   | `dotnet-run`                   | Legacy `run`, when unique                                       |

Exact names take precedence. Ambiguous shortcuts fail and suggest fully qualified
names. Namespaces come from directories, not XML or C# class names. There is no
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

## YAML task groups

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
Groups use the same naming, shortcuts, completion, and cycle detection as C#
targets. `check.cs` and `check.task` in the same directory are a duplicate-name
error. Groups may call groups or C# targets, and C# target-call helpers can call
or query groups.

The file contains one YAML mapping. Keys are case-sensitive; unknown keys,
duplicate keys (including case variants), invalid types, and multiple documents
are errors. `description` is an optional string; `steps` is a required sequence
and may be empty (`steps: []`). Each step requires a nonempty string `run`.

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
  declared types; explicit values override that child's YAML and XML defaults.
  Null, sequence, and mapping parameter values are rejected.

Parameters are passed through the normal named-argument binding; values are not
shell commands or expressions. No arguments are automatically forwarded, and
groups do not declare their own CLI parameters. Use C# for dynamic orchestration.
Names resolve against the project catalog, not relative to the group file;
prefer source-qualified names such as `_/dotnet/test`. Execution never downloads
missing targets. Shared-task add/sync/catalog distribution still manages C#
tasks; `.task` groups are project-authored files.

## A complete target

Create `.tasks/` in your project's root and save the following as `.tasks/dotnet/run.cs`:

```csharp
using DoTask;

/// <summary>Run the application.</summary>
/// <option name="configuration" alias="c" type="string"
///         choices="Debug,Release" default="Debug">Build configuration.</option>
/// <requires tool="dotnet" />
/// <requires setting="application" />
/// <example>dotask run -c Release</example>
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

The class name is arbitrary. Attach XML documentation to the entry-point class,
or to its static `Main` method. Class documentation takes precedence. Standard
`<summary>`, `<remarks>`, and `<example>` text is displayed in help. XML entities
and inline tags such as `<c>` are supported. Write XML tag and attribute names
exactly as shown, in lowercase; CLI case-insensitivity does not change XML rules.
Escape XML-special characters in text and attribute values, such as `&amp;` and
`&lt;`. These are custom XML tags, not C# attributes or Swagger annotations.

Use one explicit entry-point class with a static `Main` per target. Standard
synchronous or asynchronous entry points are supported, including `Task<int>`
when the target needs to return a status. A target without metadata is valid and
appears as `(no description)`. Top-level statements work for execution, but this
version does **not** extract metadata attached to them. Use explicit `Main` for
self-describing tasks. Parameters come from `project.Parameters`; dotask does not
forward its command-line arguments to `Main(string[] args)`.

The library reference and a small startup helper are attached during compilation.
Use `using DoTask;` to import the library namespace. Existing target files using
the earlier `using Dotask;` spelling must update that import. C# namespace names
are case-sensitive. Rebuild dotask and reinstall any packaged preview when
switching namespaces. Targets do not need a package directive for `Dotask.Library`.
Native SDK directives such as `#:package`, `#:project`, and `#:include` are supported.
Included files remain relative to the original target. Put shared `.cs` files
under an underscore-prefixed subdirectory such as `_support` so they are not discovered as targets.

Discovery accepts `.cs` executable targets and `.task` YAML groups. Files such as `.txt`, `.md`, `.target`, `.targets`, `.yaml`, `.yml`, and
`.json` are never targets, even when stored alongside tasks. Future `.rs`, `.py`,
or `.go` support will add explicit language handlers, not execute arbitrary files.
For example, this repository stores its MSBuild bootstrap hook in
`.tasks/misc/bootstrap.targets`; it does not appear in task help or completion.

For example, put `#:include _support/Helpers.cs` before `using` directives to include
a helper source file. Keep project-specific paths in YAML so the same target can
be copied between projects. Run targets through dotask; compiling/running them
directly does not supply the library and ambient context.

## Reuse a target

Copy the target file into another project's `.tasks`, then supply the settings
it declares with `<requires setting="..." />`. Copy any explicit includes or
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
| `_/dotnet/install` | `settings.project`; publishes and installs for the current user; all task logic is in one file   |
| `_/dotnet/pack`    | `settings.project`; builds local NuGet packages; no upload                                      |
| `_/git/check`      | Checks Git availability; optional `--whitespace` checks staged/unstaged diffs                    |

The build/test/format targets need the `dotnet` executable. The publish target
also uses `dotnet`; it creates publish output, not a public package release or
deployment. Adjust descriptions and declared options if you adapt a target's
behavior.

`dotask git/check` checks Git availability without requiring a repository.
`dotask git/check --whitespace` additionally runs `git diff --check` and
`git diff --cached --check`, stopping on the first failure and preserving its
exit code. These read-only checks find whitespace errors in tracked changes;
they allow a dirty tree and do not examine untracked files. The repository's
`verify-docs.cs` declares `<requires task="git/check" />` and explicitly calls
`await project.ExecTargetAsync("git/check", new { Whitespace = true })` after
checking required documents. The declaration alone does not run the task.

`shared-tasks/` is the canonical authoring location in the DoTask repository.
The corresponding `.tasks/` files are runnable project copies. The standard
`dotnet/format.cs`, `dotnet/install.cs`, `dotnet/pack.cs`, and `git/check.cs` copies are kept
identical. In consuming projects, shared-task synchronization still protects
local edits; this convention does not authorize overwriting modified copies.

`dotask format` runs the solution formatter, then checks/formats C# whitespace
throughout `project.TaskDirectory`, including other groups and custom `--use-dir`
locations. `--verify` adds `--verify-no-changes` to both steps and skips optional
line-ending normalization. Normal formatting runs the optional
`_/text/fixeol` target only if installed and propagates its failures.
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

## Option metadata

| Attribute    | Meaning                                                 |
| ------------ | ------------------------------------------------------- |
| `name`       | Required full CLI name                                  |
| `alias`      | Optional single ASCII letter                            |
| `type`       | `string` (default), `bool`, `int`, `number`, or `path`  |
| `default`    | Default value, validated like command-line input        |
| `required`   | `true` requires a resolved value; default is `false`    |
| `choices`    | Comma-separated allowed values; matching ignores case   |
| `completion` | `file` or `directory`; paths default to file completion |

`completion` changes suggestions only. Even `type="path"` does not require that
the file/directory exists; validate existence in the target if needed. A
`type="string" completion="directory"` value remains a string without automatic
path normalization. `required="true"` requires a resolved value, not necessarily
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
Boolean flags support `--verbose`, `--verbose false`, and `verbose=false`; an
unspecified optional boolean is false. Repeated options (including alias/full-name
duplicates), unknown options, and invalid values fail before execution. Short
option bundles and positional arguments are not supported in this version.
Global options are reserved throughout the command line; use `name=VALUE` when
a literal value would otherwise look like a global option.

`int` is a signed 32-bit integer; `number` is a finite double. Choice values are
returned using their declared spelling. Path options are resolved relative to
the project root. Arbitrary strings retain their original contents.

| Declared type | Read the value with                                                                 |
| ------------- | ----------------------------------------------------------------------------------- |
| `string`      | `project.Parameters.Get<string>("name")`                                            |
| `bool`        | `project.Parameters.Get<bool>("verbose")`                                           |
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
```

`.dotasks.yaml` is optional. Its supported top-level keys are `version` (currently
1), `name`, `description`, `settings`, and `targets` with optional per-target
`defaults`. Defining a target here is never required.

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
not shared settings: `project.Config` still exposes only the `settings` mapping.
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
only `settings`; per-target defaults become validated `project.Parameters`.

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
| `InstallAsync(definition, cancellationToken)` | Install published files for the current user; see [installation definitions and results](INSTALLATION.md#library-contract) |
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

That example requires `_/dotnet/publish.cs` to declare `output-dir`. Child XML/YAML defaults
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

Use a separate `<requires>` element for each requirement, with exactly one of
these attributes:

| Declaration                          | Check before compilation/execution of a selected target                                  |
| ------------------------------------ | ---------------------------------------------------------------------------------------- |
| `<requires tool="dotnet" />`         | Tool available via PATH, or the specified executable path; no version check              |
| `<requires setting="application" />` | Shared setting exists and is non-null; no type, nonempty-string, or file-existence check |
| `<requires os="windows,linux" />`    | Host belongs to the comma-separated list; names are `windows`, `linux`, `macos`          |

Multiple requirement elements must all pass. Nested setting names work, for
example `<requires setting="deployment.retries" />`. Requirements are reported
in help but are not checked during help/completion. More specific validation is
ordinary C# in `Main`: use `project.Files.FileExists(...)`, typed accessors, and
`throw new TaskException("Actionable message")` as appropriate. There is no
automatic `ValidateAsync` lifecycle hook in this preview.

For [shared tasks](SHARED-TASKS.md), two more `<requires>` forms declare companions
that add/sync must copy:

```csharp
/// <requires task="dotnet/restore" />
/// <requires file="dotnet/_support/Helpers.cs" />
```

Task IDs are `group/task` within the same source; files are relative to that
source's root. Use one element per companion in the same class/Main documentation
as the summary and options. These declarations appear in help but do not run
pre-execution checks or schedule tasks. Keep execution explicit with
`await project.ExecTargetAsync("_/dotnet/restore")`, and use a
task-file-relative `#:include _support/Helpers.cs` directive to compile helper
sources. No `.task.json` sidecar is needed.

`<capability name="network" />` adds informational help text. It neither enables
functionality nor grants permission. Unknown custom tags do not create APIs or
new validation behavior. XML never causes target entry points to execute during
discovery.

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

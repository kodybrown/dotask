# dotask guidance for AI assistants

Use this guide when creating, editing, explaining, or running dotask tasks in a
**consuming project**. For development of dotask itself, follow the repository's
[AGENTS.md](../AGENTS.md). This guide describes implemented preview behavior;
services and the other [deferred features](DESIGN.md#deferred) are not APIs.

The [Rust CLI](../README.md#rust-cli-development-preview) runs C#, Rust and Go tasks
and YAML tasks with target-call and direct-process steps, including nested calls,
project management, help, shell completion, and the interactive group wizard.
Stage `sdk/dotnet/Dotask.dotnet.dll` and the `sdk/rust/` helper crate beneath the
native executable's directory.
C# execution uses the installed .NET SDK; Rust uses Rust 1.95+/Cargo. Metadata for
both languages is an ordinary-comment YAML header and needs no toolchain.
Native initialization
and YAML-only projects need no .NET host. Use `.cs`/`.rs` task names when variants
coexist; extensionless calls must be unambiguous. Shared selection supports
group-only `--add git`, `--lang csharp|rust|go`, and exact files such as `tools/check.rs`.
Language filters select roots; dependencies retain their declared implementation.
For a direct CLI call/help request, `--lang csharp|rust|go` can select an individual
implementation without a filename suffix. It is CLI-reserved and does not flow
into nested calls; conflicting suffixes and ambiguous matches are errors.
Use explicit implementation names in dependencies, YAML steps and nested calls
when ambiguity is possible. No-op sync must preserve existing tracking bytes.

In the dotask source checkout, use `./build.sh` (Windows: `.\build.cmd`) to
bootstrap the checkout and list tasks. Use `./build.sh test` (Windows:
`.\build.cmd test`) for repository verification. These launchers accept
dotask arguments and need no installed dotask; `./build.sh help` lists tasks.
The source repository's `test` requires shared Rust build/tests followed by its
extra `verify` checks, including SDK/integration, documentation and catalog.
Standalone `verify` requires a prior inspected native build and never calls
`test`. The reusable `dotnet/verify` behavior below applies to consumers.

For dotask's own complete source installer workflow, use
`./build.sh create-dotask-installer` (Windows: `.\build.cmd create-dotask-installer`).
This local task consumes matching prebuilt doinstall tools, builds/lints/stages
dotask, and packages both Windows/Linux deliveries with native temporary-install acceptance.
Run the full `test` gate first, then use `--reuse-build` to package its inspected
executable. See [native releases](RELEASING.md) for the Windows/Linux-only
main-push workflow. Use `--doinstall-tools PATH`, `DOINSTALL_TOOL_DIR`
or builder discovery; `--source ../doinstall` explicitly selects source development
and verifies that project too. The install task uses the successful-build record.
It is not a shared consumer task; see
[installation](INSTALLATION.md#build-and-install-dotask-itself).

After final edits/formatting to any shared task or declared support file, run
`./build.sh catalog`, then `./build.sh test`; its catalog check detects stale output
without rewriting it. Commit the generated `shared-tasks/catalog.json` with
those sources. The generator is `.tasks/catalog.rs`, using the native CLI's
compiler-free metadata transport for C#, Rust, Go and YAML catalog entries.
Repository `verify-docs` checks required documents and delegates Git whitespace
checks to its declared `git/check` dependency with `Whitespace = true`.
Ordinary `git/check` remains a tool-availability check; `--whitespace` checks
both tracked diffs without changing files or requiring a clean working tree.

For Rust console tools, install the canonical group with `dotask --save rust`
then `dotask --add rust` (without a language filter: public tasks are YAML with
Rust helpers). Keep project entry points thin and leave `format-tasks` false.
Build/run/test preserve versions; explicit update-version increments patch by
default or minor/major on request. Edit canonical sources in `shared-tasks/rust/`,
regenerate and publish the catalog, then sync consumers; never edit their managed
copies. Shared `rust/test` optionally runs project `verify` after Rust tests;
only missing hooks are skipped, and hooks must never call `test`. Use
`verify: false` when a wrapper forwards extra parameters to its own required hook.
Extra product checks, packaging and release triggers remain project policy.
See the [Rust group contract](../shared-tasks/rust/README.md).

For .NET tests, use the shared dotnet/test task rather than duplicating process
guards in each consumer. Configure timeout-seconds, memory-limit-mb and lock-name
under targets._/dotnet/test.defaults; zero memory cap is the generic default.
The task supports filter, solution/project override and no-build, uses fresh
workers, rejects concurrent runs and cleans descendants on timeout/cancellation.
Windows caps use Job Objects; Linux uses a suitable existing cgroup cap or a
systemd user scope. Unsupported configured caps fail visibly (macOS needs 0).
Keep product-specific values in YAML and thin project entry points. See
[TARGETS](TARGETS.md#guarded-net-tests) for exact options and platform limits.
For formatting multiple explicit workspaces use settings.format.solutions;
--normalize-text false disables broad optional FixEol traversal for vendored code.
For .NET version preparation, install
`dotnet/update-version.cs` and configure `settings.project` (or pass `--project`).
With no local task, the short `dotask update-version` resolves to the shared task.
It writes `major.minor.yyMM.ddHH` in Utah's America/Denver timezone (24-hour,
DST-aware). Default/`--increment` refresh the date/hour; `--increment-minor`
increments minor and `--increment-major` increments major and resets minor.
`--increment-preview` increments `-preview.N` (or starts at `preview.1`).
Default/`--increment` preserve an existing preview counter; minor/major increments
remove it. Increment flags are mutually exclusive.
`--dry-run` reports without writing. Same-hour refresh is a no-op, including
the repeated autumn hour; no artificial counter or timezone offset is added. It
preserves encoding/BOM, comments and line endings. It refuses ambiguous,
conditional or inherited Version definitions and .NET component overflow.
It does not build, restore, invoke Git, or change NuGet dependency lockfiles
(which do not contain the root application version).
Three-component numeric/preview versions can be migrated; numeric output always
has four fields. Existing `AssemblyVersion` stays unchanged; existing `FileVersion`
stays numeric. `InformationalVersion` follows `Version`, preserving custom metadata
or a `$(Version)` expression. Optional properties are not added; declare
`AssemblyVersion` explicitly for stable binding and `FileVersion` explicitly for
an independent calendar file version.

For basic external commands, prefer declarative `.task` process steps rather than
requiring an unrelated language toolchain. Use `exec: {executable: ..., arguments:
[...] , cwd: ...}` with literal argument-list entries or exact parameter/setting
references. Declare typed options and simple boolean `when` guards as needed;
never construct shell command strings for ordinary argument passing. Interpreters
must be explicit. The reusable Git task is now only `git/check.task`; invoke/add
it without `--lang` and do not silently replace consumers' tracked code copies.
See [direct process steps](TARGETS.md#direct-process-steps).
For HTTP/JSON workflows, compose native `http`, `json`, `foreach`, `text`, and
`file` actions. Named results are local to the invocation; loop item references
and results stay lexical. Use `file: {operation: frontmatter}` with separate
`initial`, `fill-empty`, `set`, and `match-existing` policies to preserve handwritten
Markdown. For one-time Markdown seeding, inspect the frontmatter result's
`body-empty` flag and use explicit `body-if-empty` with optional `strip-first-h1`.
HTTP can return JSON or UTF-8 text; an optional 404 returns `found: false` and
must be guarded before consuming its data. Other request failures still stop the
workflow. No GitHub-specific synchronization command exists. See
[native YAML workflows](YAML-WORKFLOWS.md) and the
[project-page example](../examples/github-projects/README.md); do not introduce a
shell helper or regenerate an existing authored document from a template.
The reusable `text/fixeol.task` also calls its app directly: it requires `fixeol`
on PATH, preserves recursive exclusions, and always normalizes batch files to
CRLF. It defaults other text to LF; `--eol crlf` changes that pass. Missing tools
and normalization failures are errors. Replace old tracked C# copies explicitly.
The archive tasks are also declarative: `_/archive/create-zip.task` and
`_/archive/create-7z.task` use `7z` (or `--archiver`) without a task compiler.
They preserve the named source folder and refuse existing/nested outputs.
Process path references and `output-file` provide canonical input paths and
staged publication without overwriting. See [process file output](TARGETS.md#staged-process-file-output).
Replace old tracked Rust copies explicitly; invocation requires no `--lang`.
`_/doinstall/assemble.task` similarly invokes separately supplied doinstall tools
without a language compiler. The builder defaults to PATH; provide the matching
actual runtime path with `--installer`, not a command shim. Both assembly tasks
require `--result-file FILE` outside the package, with an existing parent.
Use `_/doinstall/assemble-self-extracting.task` plus an explicit
`--launcher-template FILE` for one self-extracting launcher. Use the returned
JSON `FilePath`, never a guessed `installer.exe` or launcher name. It takes resolved
YAML/payload, does not return an installer artifact, and never builds/downloads
doinstall. Dotask's build/gather creator is create-installer.task, backed by the
precompiled dotask-doinstall-create-installer executable beside dotask. It preserves
nested task context and InstallerArtifact results without a task compiler. Prebuilt
payloads with declarative build steps need neither Rust/Cargo nor .NET. The generic
install launcher remains a Rust task. See [installation](INSTALLATION.md) for tool configuration and replacement
of tracked `assemble.rs` copies. The two products are installed independently.
For the build/gather creator, allow source-only `packaging` filename templates,
pass `${result-file}` through the assembly step, and supply `${launcher-template}`
from `--launcher-template` or `settings.doinstall.launcher-template` for that mode.
When paths are unspecified, the creator asks the selected builder for
`--print-tool-paths` JSON instead. Installing doinstall's own self-extracting
package once makes its builder available on PATH; ordinary creator recipes then
need no project tool-path settings. Explicit target paths still override discovery.
The creator records the actual artifact only after every packaging step passes.
Follow the complete [self-extracting recipe](INSTALLATION.md#self-extracting-build-and-gather-workflow).

For Go tasks, import the bundled `dotask.local/sdk`, use `dotask.Run`, and
follow the [Go task contract](TARGETS.md#go-tasks). Never create a consumer
`go.mod`/`go.sum` for dotask or run `go get` to obtain its SDK. Declare every local
support file; use the precompiled shared installer creator for Go packaging.
Use the [shared Go installer tasks](../shared-tasks/go/README.md) for configurable
native test/vet/build, literal version lookup, exact payload staging and artifact
return through the precompiled creator. Keep consumer wrappers thin.
The same group provides Windows-only temporary-root `go/test-installer.go`
acceptance without Rust/.NET. Its `test-installer` shortcut uses the last
successful installer record when `--installer` is omitted (an explicit
`installer.installer_file` setting retains SDK precedence); no project wrapper
or filename guessing is needed. Records stay separate from `.dotasks.yaml`.
The group also provides `go/update-version.go` for explicit literal
preparation. Use three-part SemVer with a numeric patch (default) or the built-in
`go.version-patch: date` policy for Utah YYMMDD patches and preview preparation.
No project-specific version wrapper or generated metadata package is needed.
Preview actual project versions with `--dry-run`; never bump versions as part
of ordinary verification.
Go execution needs an installed stable Go 1.22+ compiler, with no Rust/.NET
requirement unless the recipe explicitly calls tasks in those languages.

## Establish the project context

1. Read the consuming project's instructions and existing task files before
   editing. Identify the working directory, selected tasks directory, and its
   discovered project root. Inspect `.dotasks.yaml` if present.
2. Check `dotask --version`. If the tool is absent, use the
   [source or local-preview instructions](../README.md); there is no public
   package release to assume. For C# execution, check the SDK selected from the task directory
   with `dotnet --version`; this is not a prerequisite for Go or native YAML tasks.
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
   or `public static async Task Main()`. Put the YAML task header at the top of the file
   before code or SDK directives. Top-level statements are also supported.
3. Add a clear `description`, declare every accepted parameter under `options`,
   and add useful `examples` and `requires` entries. Header keys
   are lowercase and case-sensitive. Metadata is YAML inside ordinary C# comments.
4. Get the context with `var project = BuildContext.Current;` and resolved settings
   with `var config = project.Config;`. Read validated parameters through
   `project.Parameters.Get<T>("full-name")`.
5. Put values shared by tasks under `settings` in `.dotasks.yaml`. Put task-specific
   paths/data under `targets.<full-name>.settings`, using the task's full identity.
   `project.Config` applies that task's settings over shared values; nested calls
   resolve their own settings and do not inherit the caller's overrides.
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
   Shared tasks declare required companions in that same YAML header:
   `{kind: task, value: dotnet/restore}` uses a same-source `group/task` ID;
   `{kind: file, value: dotnet/_support/Helpers.cs}` uses a source-root-relative
   path. Add/sync copy these companions; the declarations do not execute them.
   Keep `#:include` paths relative to the task file. Do not create `.task.json`
   sidecars or declare optional target calls as required dependencies.

Use the [authoring reference](TARGETS.md) for exact signatures and the
[CLI guide](USAGE.md) for accepted syntax. If a helper is not documented, inspect
the library before claiming it exists. Ordinary .NET APIs can fill gaps.

## Rules that prevent common mistakes

For installation tasks, read [the installation contract](INSTALLATION.md).
The shared `_/installer/install` finds an existing installer using
`settings.installer.installer_file`, a literal path or version template. It never
calls a creation task; build-task names are unrestricted. When unconfigured it
prints a YAML example, offers valid recorded output or bounded project discovery,
and prompts before saving a suggestion or placeholder. Without a terminal it
fails with guidance and makes no changes. Never silently rewrite user settings.
After all packaging steps pass, call `RecordInstallerAsync` (Rust:
`record_installer`) to save the successful artifact separately under
`artifacts/.dotask/installers/`. Keep that local state ignored. Returning an
invocation result with `SetInstallerResultAsync` is a separate operation.
Use `CreateInstallerAsync` and `RunInstallerAsync` for custom build-and-run
orchestration; do not scrape stdout or infer arbitrary output paths.
`installer-args` is a JSON string array replacing default installer arguments.
An exact project `install` can delegate to `_/installer/install` explicitly.
The console installer prompts by default. For unattended execution, pass
`--installer-args '["--non-interactive"]'` to install, or `--non-interactive` to
uninstall. Installer YAML owns defaults, prompt mode, and labels; Windows PATH
changes are configurable with `add-to-path`. Verification must use temporary
install/command roots and `--set add-to-path=false` to preserve the user's PATH.
Updates may change the additional command's location or disable it; actual
receipt-owned launcher paths remain protected. For application-menu shortcuts,
use `name: '${shortcut-name}'` in installer YAML to enable a safe custom name;
`start-menu-nested` selects Windows `name/name.lnk` placement. Dotask's own
package declares no menu shortcut. Use temporary Start Menu/desktop roots when
testing shortcut creation, updates, and ownership-safe cleanup.
The standalone engine reads packaged YAML matching its executable filename,
never `.dotasks.yaml`. Installed recovery YAML retains its internal name.
The creator chooses which project settings become installer configuration;
project identity is not copied implicitly. `application.name`, `description`,
and optional `copyright` supply the interactive console banner. Shortcut
declarations grant permissions while defaults choose the initial answers.
Group shared installer settings under `settings.installer`: `config` and `output`
control creation; `installer_file`, optional `select: highest-version`, `kind`
and `installer_args` control existing-output selection/launch. Accessors use
dotted keys such as `installer.config`. Separately
supplied tool selections belong under `settings.doinstall.builder` and
`settings.doinstall.installer`; `source` is only an explicit development helper.
Dotask's own stable source template is `.tasks/installer.yaml`. Its local
`create-dotask-installer` task builds/verifies the external tools and dotask,
then delegates build/gather/packaging and successful-output recording to the
shared creator. There is no local `create-installer` wrapper. Other
applications keep their own build wrapper and installer configuration.
Application versions are owned by each application.
Supply `application.build.stamp` and optional `commit` to align package and
installed directory names with the app's own build information. Named package
creation by `doinstall-builder` can use `--output-parent` and a structured `--result-file`; identical names
with different contents are rejected. Git is optional, and the application's
version is never inferred from the installer/tool version. Dotask's local creator
uses its inspected executable's committed Cargo version and build stamp. Only
explicit `update-version` changes that version; build/run/test/packaging do not.
Git naming policy is configured in `settings.app`.

Doinstall's standalone Rust installer uses YAML receipts and the `installer/` plus
`app/<build>/` layout. Older formats are unsupported; use fresh destinations or
manually remove the old installation first. Its creator copies packages beneath the project-relative
or absolute `settings.installer.output` directory; dotask supplies a fresh external
directory beneath Cargo's evaluated target directory,
without redirecting MSBuild outputs. Use the bootstrap launcher when upgrading an older CLI/library,
and pass explicit temporary absolute install/bin paths via `--installer-args`
for tests. Never remove ownership/recovery records or overwrite an unrelated
launcher to bypass conflicts.

| Need                       | Use / behavior                                                                                                                       |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Ambient context            | `BuildContext.Current`; no constructor or DI container                                                                               |
| Resolved setting           | `project.Config.Get<T>("key")`; shared values plus current-target settings; dotted nested keys supported                            |
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
| Validation beyond metadata      | Explicit checks in `Main` and actionable `TaskException` messages                                                                    |

- Option types are `string`, `bool`, `int`, `number`, and `path`; read `number`
  as `double`. Aliases are one ASCII letter. Choices use a comma-separated
  `choices="Debug,Release"` attribute. There is no option array/object type.
- Explicit arguments override YAML target defaults, which override header defaults.
  `settings` and task parameters are separate collections. YAML default keys use
  full option names. Identifiers match without case sensitivity; arbitrary string
  contents and filesystem case are preserved.
- A default satisfies `required: true`. `type: path` normalizes a path without
  checking existence. `completion="directory"` controls suggestions, not runtime
  validation. `{kind: setting, value: x}` checks presence, not type or file existence.
- `RunAsync` takes an executable plus separate arguments. Do not prequote list
  entries, concatenate a shell command, or normalize every argument as a path.
  Pipes, redirects, shell built-ins, and `.cmd`/`.bat` launchers need an explicitly
  invoked shell and its own quoting rules. A Unix-only shell command is not a
  portable task just because it appears inside C#.
- `RunAsync` throws on nonzero exit by default. If disabling `ThrowOnError`,
  inspect the result. Do not silently turn command failure into task success.
- Nested calls execute each time and receive only explicitly supplied parameters
  plus their own defaults. They resolve their own target settings from the shared
  configuration snapshot. Await sequential
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
Keep the `dotnet/format.cs`, `doinstall/*.rs` / `doinstall/*.task`, `dotnet/pack.cs`, and `git/check.task` copies
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
- Use `dotask help TARGET` to inspect its header options and effective defaults.
  Help never compiles, restores, or executes tasks. Exit 0 can include individual
  metadata/default errors and does not prove the target compiles.
- Metadata belongs at the top of the task file, before code/directives. Use
  `description`, `options`, and `requires` in a `// dotask: 1` header.
  Top-level statements are supported; helper methods are not separate CLI tasks.
- Declare shared companions in the YAML header: `{kind: task, value: group/task}` and
  `{kind: file, value: group/_support/Helper.cs}`, relative to the same source.
  These control copying, not execution; no `.task.json` sidecar is used.
- Use `BuildContext.Current`, `project.Config`, and `project.Parameters`.
  Read typed keys with `Get<T>`/`GetPath`; YAML does not generate C# properties.
- Keep shared values under `settings` and task data under `targets.<full-name>.settings`
  in YAML; no task registration is required. `project.Config` resolves shared values
  plus the current task's overrides. Nested calls resolve the child's settings.
  Top-level `name`/`description` are display metadata, outside `project.Config`.
  Explicit parameters override YAML target defaults, which override header defaults.
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

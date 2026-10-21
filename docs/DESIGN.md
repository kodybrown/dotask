# Design contracts

## Structure

`src/Dotask` builds `Dotask.dotnet.dll` with the `DoTask` namespace: ambient
context, typed values, safe argument passing, portable paths, filesystem helpers,
and target-call transport.
It has no third-party runtime dependencies or DI container.

`src/dotask-cli` is the sole executable implementation. Discovery, comment-header
metadata, YAML loading, binding, help, completion, SDK compilation, and execution
are separate native components. `main.rs` invokes the application.
The retired C# executable remains available in Git history; it is not built or
needed by tests. The .NET solution contains the maintained helper and test suite.

## Rust CLI transition

Keep dotask's Rust CLI at `src/dotask-cli`, Rust SDK at `src/dotask-sdk`, and the
maintained C# authoring library at
`src/Dotask`. The Rust packages share a root Cargo workspace, lockfile, and
release profile. Installer dependencies and verification live in doinstall's
independent workspace.

The Rust CLI now runs C# tasks, Rust tasks, and YAML groups. It owns command parsing,
root discovery, exact/shortcut selection, sessions, group sequencing, process
execution, nested-call dispatch, and exit/cancellation handling. It also owns
YAML configuration/groups, binding, requirement validation, initialization, shared
management, help, shell completion, and the interactive group wizard. Rust is the
bootstrap runner and packaged application; `src/dotask-sdk` is its maintained Rust task
helper crate. See the
[preview commands](../README.md#rust-cli-development-preview).

The installed application contains the native executable,
`sdk/dotnet/Dotask.dotnet.dll`, and the Rust helper source crate under `sdk/rust/`.
Language-specific helper directories leave room for future supported languages.
No language runtime, compiler,
Roslyn assembly, managed CLI or separate support host is shipped. Both task
languages use an ordinary-comment YAML header beginning `// dotask: 1` and
ending `// end-dotask`; native metadata discovery needs no language toolchain.
The extension selects the handler and a file exposes one task. Unknown header
versions/fields fail explicitly. Legacy XML and Rust doc-block task metadata are
not interpreted; ordinary code documentation remains untouched.

C# compilation is native orchestration of the user's selected .NET SDK. It
injects the helper DLL and initialization into an isolated file-based build,
preserves SDK/package/include/project directives, locks its external cache,
and copies compiled output into an invocation snapshot. Only implicit hooks of
the task's synthetic project are isolated; explicit project references retain
normal output policy. Rust compilation uses the installed Cargo/Rust toolchain.
Missing tools leave headers/help available and cause actionable execution errors.

Nested context carries the native CLI executable with an empty argument prefix.
C# and Rust tasks use the same `__exec` file transport and immutable configuration
snapshot. The public C# task
API and invocation-local installer results are unchanged. Context files are
private and execution snapshots are cleaned up after completion/cancellation.

On Windows the native runner assigns suspended children to job objects before
resuming them. On Unix it creates process groups and forwards Ctrl+C. After a
short cooperative cancellation interval, it terminates its child group and
returns 130. Native platform acceptance is recorded in [verification](VERIFICATION.md).

The following decisions describe the implemented transition:

1. Replace the CLI with Rust while preserving existing C# task authoring,
   metadata, compilation, nested calls, and observable CLI behavior. Retain the
   C# library and C# task support. Regression tests execute the native CLI; fixed
   pre-removal help/completion/YAML snapshots preserve earlier expectations.
2. Add Rust task execution and a bundled Rust helper crate at `src/dotask-sdk`.
   Each task has one `.rs` entry file. Generate Cargo build manifests in an
   external cache and reference the bundled helper crate's source. Use the
   developer's Rust toolchain; neither source-local build output nor nightly
   single-file scripting is required. C# and Rust tasks call each other through
   dotask's common invocation protocol, with explicit context, parameters,
   cancellation, failures, and supported structured results.
3. Keep task groups about functionality: `_/dotnet/build.cs` builds .NET
   projects, `_/rust/build.rs` builds Rust projects, and utilities such as
   language-neutral utilities such as `_/git/check.task` call apps directly.
   Canonical sources remain in `shared-tasks/<group>/`; installed copies remain
   in `.tasks/<source>/<group>/`. The extension identifies the implementation.
4. Generate catalog variants from the actual source files and record exact
   installed variants and original hashes in the lockfile. Allow `--add git` as
   shorthand for `--add _/git/*`, installing all available implementations;
   `--lang csharp` or `--lang rust` filters implementations without changing the
   task group. Explicit selection such as `--add tools/check.rs` chooses one.
   `--sync` updates tracked variants; adding the group again can add newly
   available variants. Preserve local-edit and ownership protections.
5. Use installed files for execution with no project-wide language setting.
   Allow extension-qualified calls such as `tools/check.cs` and `tools/check.rs`.
   Extensionless calls require an unambiguous target. Apply the same rules to
   nested calls, and identify implementations distinctly for cycle detection.
   Mixed-language projects need only the toolchains required by executed tasks.
   CLI `--lang csharp|rust` filters the requested target/help selection, not
   nested calls or group children. Explicit conflicting suffixes fail. The flag
   is reserved, and directory names do not become implicit execution groups.

Common YAML metadata parsing is owned by Rust. Native shared management
preserves the existing SHA-256 revision serialization, lock schema, transaction
ownership marker, and recovery journal. It never adopts untracked files or
overwrites local changes. Completion remains local and skips project configuration;
the wizard writes only after an explicit save and supports discarding an unfinished
step with `:back`. Native configuration preserves quoted scalars, rejects duplicate
keys regardless of case, and bounds nesting and alias expansion.

The Windows scope now combines Rust task authoring with bootstrap and packaging
cutover. Repository orchestration includes `.rs` files such as
`create-dotask-installer`; shared C# tasks and their authoring library remain maintained.
Thin build/run/test/format/Clippy/version tasks compose the managed Rust group.
`test` owns the full gate; its required `verify` hook contains only additional
SDK/integration, task-formatting, docs and catalog checks. The generic Rust test
hook is optional for consumers, but failures are never skipped. Bootstrap uses
a separate Cargo profile so it cannot replace the inspected release executable.
Staging and run validate the successful build hash; installer assembly reuses
that executable. Cargo's committed SemVer is authoritative; UTC build metadata
does not change package precedence. Only explicit update-version edits it.
Rust tasks declare leading ordinary-comment YAML metadata and compile through isolated,
content-addressed manifests/source snapshots outside the project. SDK dependencies
are pinned; Cargo retains a lockfile per snapshot and checks compiler fingerprints.
Support modules must be declared as task-root-relative file requirements. Help
never compiles or runs tasks. Catalog generation still emits existing C# shared
and Rust entries, with extension-qualified IDs for coexisting variants. Language
selection filters shared roots; dependencies use explicitly resolved files.
Existing tracking identities and revision serialization remain stable on no-op
sync. No untracked files are adopted and no language is chosen implicitly.
Linux/macOS and live remote catalog acceptance remain pending independently of
the Windows cutover. Installation and publication are separate user actions.

## Declarative process tasks

The existing `.task` format combines target-call `run` steps with direct `exec`
steps; exactly one action is allowed per step. Code headers and YAML files share
option/requirement validation. Literal strings or exact scalar parameter/setting
references preserve argument boundaries without shell evaluation. Cwd and
explicit executable paths resolve from the project root. Boolean parameter
conditions can skip either action; no general expression language is provided.

The native process runner owns streams, exit codes and cancellation for `exec`.
Native `http`, `json`, `text`, and sequential `foreach` steps compose remote-data
workflows without a language SDK. HTTP publishes only complete bounded collections;
loop-local results are cleared between iterations without repeatedly copying the
outer collection. Data references preserve JSON types and never reinterpret
inserted values as references. The frontmatter file operation patches selected
YAML fields with guarded atomic writes while preserving editorial text and the
Markdown body; unsupported source layouts fail before publication. Ownership
policies and GitHub-specific mappings belong to the consuming `.task`. See
[native YAML workflows](YAML-WORKFLOWS.md).
No compiler, restore or SDK helper is involved. Shared catalog/lock version 1
supports runtime `yaml` and exact `.task` entry-file ownership. Old code copies
are never migrated or adopted. Only `git/check.task` remains reusable; dotask's
repository tracking explicitly replaced its unchanged owned Rust copy. Consumer
repositories and all doinstall/Inno adapters remain untouched.

## Discovery and metadata

`--init` operates directly on the invocation directory before normal discovery.
It preflights configuration and destination types, creates missing pieces, and
publishes a new configuration without overwriting an existing destination.
Repeated initialization preserves existing user files; legacy/invalid YAML and
symlink destinations require user review. Initialization uses no SDK or shared
task sources and creates no lock file. See [initialization](USAGE.md#initialize-a-project).

Root `.dotasks.yaml` or the nearest `.tasks` anchors the project. Legacy
`.tasks/config.yaml` remains readable, but simultaneous old/new configuration is
an error. `--use-dir` is an exact override; root config above the selected directory
anchors paths, with parent-root fallback. See [usage](USAGE.md#project-layout-and-discovery).

Discovery recurses through ordinary directories, excluding hidden/underscore
entries (except the official task-root `_` directory), build output directories,
and symlinks. Canonical names are task-relative
paths, including source/group/task for shared copies. Exact names win; suffix
shortcuts must be unique. Legacy space-grouped filenames retain their identities.
There is no default group or imports registration. Help, completion, execution,
nested calls, defaults, and cycle detection share canonical identities.

A common YAML comment header at the top of each task file
supplies the description, options, requirements, examples, and capabilities.
Invalid metadata is isolated to its target. Project YAML configures values and optional defaults, not target registration.
Each `targets.<full-name>` entry can contain option `defaults` and task-specific
`settings`. Runtime settings recursively merge shared values, base-name target
settings and implementation-qualified settings. Arrays/scalars/nulls replace
earlier values. The native executor owns this resolution for C#, Rust and YAML
tasks, including setting requirements. Execution transports keep original shared
settings separate from the resolved view, so nested calls receive their own
overrides from one immutable snapshot rather than inheriting their caller's data.
Separate `.task` YAML files declare ordered groups of existing targets; their
strict metadata parser feeds the same catalog. The executor interprets groups
directly, preserving cancellation, canonical call chains, child option binding,
and failure exit codes. No C# compilation is needed for a group itself.

Bare `dotask` and `dotask help` show Targets and the detailed-help hint.
`--verbose` adds the optional YAML `name`/`description`, shared and target settings, and
combined target options. Execution verbosity travels in the private context to
nested calls and emits diagnostics on stderr without altering task parameters. The
name falls back to the project root directory's name. Identity fields remain
separate from the runtime `project.Config` settings.

CLI-only `dotask --help`/`-h` returns usage before any project discovery or reads.
Project summaries and selected-target help read the source metadata, load YAML,
validate effective defaults, and render the project's details. Help does not instantiate a
compiler, create a compilation cache, restore packages, or launch child processes.
C# and Rust use the common header parser; groups use strict YAML parsing. Both run
on every invocation without a metadata cache. Metadata/default errors remain visible, but help does not determine whether
a target compiles. Required arguments and execution requirements are not enforced
during help. Target completion skips project configuration entirely.

## Compilation and process boundaries

dotask delegates compilation to `dotnet build original-target.cs`. This preserves
the SDK's file-based directives, includes, package references, and project
references. It does not implement a second C# compiler or translate the source.
The source's `global.json` and NuGet configuration remain relevant.

Generated MSBuild wrappers isolate the entry-point task from the consumer's
implicit `Directory.Build.props` and `Directory.Build.targets`, attach the helper
assembly, and write outputs to a per-user external cache. Explicitly referenced
projects retain their own normal build properties and targets. The runtime cache
is under the system temporary directory's `_dotnet/dotask` tree; it is disposable.

Compilation is locked per target and requested when executing that target.
Compiler failures are reported before its entry point can run. A broken target
does not prevent help or execution of other targets. There is no independent stale-success cache:
the SDK checks source, include, and reference changes on each compilation request.

For execution, build outputs are copied into an invocation snapshot while holding
the compile lock. This prevents concurrent recompilation from modifying running
binaries, including on Windows. The lock is released before running target code,
so nested calls do not hold locks for their entire execution.

Each C# target runs in a child .NET process, with its working directory set to the
project root. A private JSON context file carries the configuration snapshot,
parameters, original paths, and call chain. Its filename is passed through the
environment. Nested calls invoke the same CLI with a private request file. Files
and execution snapshots are removed after execution; the compilation cache remains.
Each call owns a temporary subdirectory, allowing the caller to clean up after a
cancelled child. Existence queries and optional execution use the same request
protocol and current catalog. Replies use separate JSON files so target stdout
cannot corrupt the result and a child exit code cannot be mistaken for absence.
An existence query does not construct a compiler or enforce target validity.
Optional execution returns a status, an observed exit code when available, and
an error diagnostic. Strict execution retains its throwing behavior.
Forced process termination can leave temporary session files behind.

A generated module initializer turns unhandled target exceptions into reported
task failures without the runtime's abort/core-dump path. Nonzero process results
and nested failures propagate. Cancellation kills launched process trees and the
CLI returns 130. Arbitrary detached/background processes remain the author's
responsibility.

## Completion

One parsed target model powers validation, help, and completion. Shell adapters
query the hidden `__complete` command with the line and cursor. The protocol returns
tab-separated candidate, kind, and description. Completion parses source metadata
and enumerates paths, but never loads project configuration, restores packages, compiles,
or executes target entry points. Invalid or incomplete input quietly yields useful
partial suggestions where possible.

Adapters are supplied for Bash, Zsh, Fish, and PowerShell. Native shell syntax and
quoting are handled at the adapter boundary. General shell expression evaluation,
command substitution, and arbitrary environment interpolation are not performed.

## Shared task distribution

`SharedTasks` separates source catalogs/snapshots from project file management.
The online cache is a static JSON catalog plus selected C# source/support files.
Catalog generation and private-task indexing read common YAML headers with the
same native parser as help. `{kind: task, value: group/task}` declares a
same-source dependency; `{kind: file, value: group/_support/Helper.cs}`
declares a file relative to the source root. No per-task JSON manifest is needed;
old sidecars produce migration errors instead of silently losing companions.
Downloads are hashed before installation; immutable original blobs are retained
in the local cache. Private task originals are only read and snapshotted.

Project copies, not caches, determine runtime discovery. Configuration contains
settings/defaults; a separate committed YAML lock records source identity,
content revisions, requirements, and original per-file hashes. Catalog dependencies
control which files are installed, not runtime scheduling. No commands fetch
network data during normal help/completion/execution.

Management operations hold a per-project lease and preflight the entire batch.
Untracked or locally modified destinations fail safely. File changes and tracking
use a durable rollback journal; interrupted operations recover before subsequent
management commands. Unexpected later edits prevent automatic recovery, retaining
backups for manual inspection. Execution refuses an incomplete journal.

See [shared tasks](SHARED-TASKS.md) for the exact add/sync/remove, dry-run, manual
merge, source naming, and preview publishing contracts.

## Application installation

The shared `_/installer/install` selects an existing installer through a literal
`installer.installer_file` or host/version template. Numeric version precedence
selects the highest version; equal-version builds remain ambiguous. Selection
never relies on timestamps, orders hashes, or invokes a creation task. Creation
task names are unrestricted. The library validates platform identity and launches
supported installer kinds with tokenized defaults or explicit replacements.
Projects own packaging, installer UI/elevation requirements, shortcuts, and
application lifecycle; application copying is not an installation fallback.

Explicit SDK calls record only successful completed builds, atomically and per
platform under ignored `artifacts/.dotask/installers/`. Metadata and a digest
allow guided setup to reject missing/changed recorded output; invocation results
remain separate. A configured path/template is authoritative. When configuration
is absent, setup prefers a valid record, then bounded project discovery. A
terminal user confirms the installer and may save a suggested template or add an
editable placeholder. Noninteractive calls print guidance and fail. User YAML
is never rewritten on builds or reads: confirmed setup performs a surgical,
source-preserving insertion, rejects unsupported layouts/concurrent changes,
and leaves legacy configuration consolidation explicit. This policy lives in
the optional task SDK, leaving CLI discovery and help metadata-only.

Dotask's creator publishes its application and packages the Rust engine, YAML,
and payload using the shared creator. It respects evaluated output paths and
copies the package beneath `settings.installer.output`. Only the current YAML
format is supported: no legacy engine, compatibility API, or import path is kept.
Existing unrecognized directories and unowned launchers remain conflicts.

Unix commands are symlinks. Windows commands use the bundled native launcher plus
a UTF-8 `.shim` sidecar. Source and x64/ARM64 delivery assets remain in
the external doinstall repository; ordinary installs do not compile or download launchers.
The launcher preserves process arguments, standard streams, and exit behavior.
See [installation](INSTALLATION.md) for the ownership and activation contract.

## External installer integration

Reusable ZIP/7z tasks are declarative process calls. Generic `path-parent` and
`path-name` references resolve existing inputs; `output-file` stages a producer's
file on the destination filesystem and publishes it only after exit zero. It
refuses existing/concurrent destinations and optional source-directory overlap.
No archive format implementation or shell is added to the CLI. Metadata remains
compiler-free; literal arguments and process failure/cancellation behavior remain
unchanged. Hard-link publication preserves the previous archive task's contract.

Doinstall owns the runtime, builder, Windows shim, schema, and installer behavior
tests in its independent repository. Dotask owns generic InstallerArtifact
transport and launching, plus shared `_/doinstall/*` adapters and
`_/installer/install`. Producer/gather/archive orchestration remains task code.
The authoritative installer design lives at
[doinstall's design contract](https://github.com/kodybrown/doinstall/blob/main/docs/DESIGN.md).

Install dotask and doinstall independently. Dotask's installed payload contains
its native executable and task SDK helpers, not doinstall or its builder. The
optional adapters belong to dotask's shared-task catalog; doinstall never
discovers projects, writes task files or manages their tracking. The CLI needs
no special tool-discovery/download service. `assemble.task` is an ordinary direct
process call; dynamic build/gather and artifact launching retain their Rust tasks.
The builder can use PATH, but the packaged runtime uses an explicit actual
executable path to avoid distributing a command shim. Source builds are explicit
development/CI preparation; packaging and verification consume prebuilt tools.

## Deferred

Service management is deliberately deferred. There is no service provider,
service binary validation, or service installation functionality in this first version.
Also deferred: generated C# properties for YAML keys, remote execution, caching
of completed tasks, runtime dependency scheduling, additional task languages,
third-party/authenticated remote sources, CLI self-update, parallel scheduling,
installation rollback/version-switch commands, a JSON installation interface, and global installation
or publishing as part of repository verification.

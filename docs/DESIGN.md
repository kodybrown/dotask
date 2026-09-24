# Design contracts

## Structure

`src/Dotask` builds `Dotask.Library.dll` with the `DoTask` namespace: ambient
context, typed values, safe argument passing, portable paths, filesystem helpers,
and target-call transport.
It has no third-party runtime dependencies or DI container.

`src/Dotask.Cli` builds the `dotask` command. Discovery, XML metadata, YAML loading,
binding, help, completion, SDK compilation, and execution are separate components.
Roslyn reads actual C# documentation trivia, and YamlDotNet parses configuration.
`Program.cs` only wires cancellation and invokes the application.

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

XML documentation on the entry-point type, or its static Main method,
supplies the description, options, requirements, examples, and capabilities.
Invalid metadata is isolated to its target. Project YAML configures values and optional defaults, not target registration.
Separate `.task` YAML files declare ordered groups of existing targets; their
strict metadata parser feeds the same catalog. The executor interprets groups
directly, preserving cancellation, canonical call chains, child option binding,
and failure exit codes. No C# compilation is needed for a group itself.

Bare `dotask` and `dotask help` show Targets and the detailed-help hint.
`--verbose` adds the optional YAML `name`/`description`, shared settings, and
combined target options. Execution verbosity travels in the private context to
nested calls and emits diagnostics on stderr without altering task parameters. The
name falls back to the project root directory's name. Identity fields remain
separate from the runtime `project.Config` settings.

CLI-only `dotask --help`/`-h` returns usage before any project discovery or reads.
Project summaries and selected-target help read the source metadata, load YAML,
validate effective defaults, and render the project's details. Help does not instantiate a
compiler, create a compilation cache, restore packages, or launch child processes.
C# uses Roslyn/XML metadata parsing; groups use strict YAML parsing. Both run
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
Catalog generation and private-task indexing read the entry-point XML comments
with the same Roslyn metadata parser as help. `<requires task="group/task" />`
declares a same-source dependency; `<requires file="group/_support/Helper.cs" />`
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

The shared `dotask-installer/install.cs` coordinates `create-installer`
and launches its structured `InstallerArtifact`. Console and GUI apps use the
same contract; there is no direct-copy fallback or search for existing packages.
Creation and launching are separate, and invocation-scoped results prevent stale
handoffs. The library validates platform identity and launches supported installer
kinds with tokenized defaults or explicit replacements. Projects own packaging,
installer UI/elevation requirements, shortcuts, and application lifecycle.

Dotask's creator publishes its application and packages the Rust engine, YAML,
and payload using the shared creator. It respects evaluated output paths and
copies the package beneath `settings.installer-output`. Explicit legacy import
preserves ownership of older dotask-managed installations. The following
paragraphs describe the retained legacy .NET installation API.

`UserInstaller` copies verified snapshots to immutable version/fingerprint
directories, records ownership, and activates commands under per-root/bin locks.
Lock files use exclusive delete-on-close handles, so normal completion and error
unwinding remove them without a separate close-then-delete race.
Unix commands are symlinks. Windows commands use an embedded native launcher plus
a UTF-8 `.shim` sidecar. Launcher source and x64/ARM64 delivery assets live in
`src/Dotask.Shim`; ordinary installs do not compile or download launchers.
The launcher only starts the named executable and preserves process behavior.

Command activation uses a durable rollback journal; unexpected edits prevent
automatic recovery. Missing owned launchers are recreated on reinstall; the
journal records their physical absence while retaining the original ownership
receipt, so interrupted repairs can roll back safely. Creation refuses to
overwrite a command that appears after the missing-state check. Modified owned
commands and occupied unowned destinations remain conflicts.
Multiple command updates are recoverable but not jointly
atomic to observers. Old builds are retained. PATH editing, existing global-tool
removal and command-directory relocation are not automatic. See
[installation](INSTALLATION.md) for the public API and ownership contract.

## Standalone installer design

This section records the agreed design and deliberately deferred features.
The implemented interface and native acceptance limits are documented in
[Application installers](INSTALLATION.md) and [verification](VERIFICATION.md).

### Task and distribution boundaries

- Group reusable C# tasks under `_/dotask-installer/*`, with canonical sources in
  `shared-tasks/dotask-installer/`. Application build/publish steps remain
  project-specific; packaging and installation must work with non-.NET payloads.
  Migrate existing shared install tasks through dotask management commands,
  avoiding competing short names. PTS acceptance must use that workflow.
- Use a generic Rust installer, without an Inno Setup dependency or Windows
  Installed Apps integration. Retain the existing bundled Windows command shims;
  consumers do not rebuild them. The installer must support automated uninstall.
- Distribute a platform/architecture-specific executable, same-basename YAML,
  and a payload directory or ZIP. YAML carries application metadata, version,
  inputs, defaults, and installation instructions. Resolve payload paths relative
  to the YAML. No dotask or language toolchain is required on the destination.
- `create-installer` builds a package and returns its artifact without installing;
  `install` invokes the creator and runs the artifact. Preserve custom project
  creators and the structured artifact contract.

### Inputs and desktop integration

Install and uninstall are unattended unless `--interactive` is specified. Missing
required information, invalid values, or unresolved conflicts stop with an
actionable error and nonzero exit status; redirected input never causes a prompt.
Named typed inputs own prompts and choices. Value precedence is CLI overrides,
explicit YAML values, environment-profile defaults, OS defaults, then common
defaults. Interactive answers may replace defaults. Environment profiles include
Linux desktop/distribution differences such as Omarchy.

Shortcuts are available to console and GUI applications. YAML declares which
options are allowed and their defaults; CLI overrides and interactive choices
must respect those permissions. Support Windows Start menu and desktop shortcuts,
Linux application/desktop entries where supported, and macOS desktop aliases.
Terminal behavior, arguments, working directory, and icons need explicit metadata.
Desktop integration must be tested with an actual GUI consumer, including WinExe.

Uninstall preserves settings by default. `--leave-settings` and
`--remove-settings` are mutually exclusive; removal is restricted to declared
application-owned settings, never arbitrary documents or unrelated user data.
An explicit unattended uninstall invocation does not require another confirmation
flag. The installed engine and records must suffice without the original payload.

### Installation layout and receipts

Keep management files in a visible `installer/` directory beneath the application
root. The uniform payload layout is `app/<version>-<build-id>/`:

```text
pts/
  installer/
    installer.exe
    installer.yaml
    installation.yaml
  app/
    1.2.0-abc123/
      installer.yaml
      installation.yaml
      <application files>
    1.3.0-def456/
      installer.yaml
      installation.yaml
      <application files>
```

The executable has no `.exe` suffix on Unix. The root management receipt is
mutable installation state: identity, resolved locations, active and previously
active builds, owned shortcuts/commands, and installed build inventory. Each
build keeps an immutable receipt and a snapshot of its installer YAML. Uninstall
uses the corresponding installed configuration together with actual ownership
records, rather than recomputing locations from defaults or assuming the newest
configuration describes all older builds. Reserved metadata names must not
collide with payload files. Schema 1 and legacy import are in the public reference.

Always using version directories replaces the earlier fixed-location layout
proposal. Retaining multiple builds and an application's
ability to run from a version-specific path are separate concerns. Directory
layout alone cannot make incompatible application paths or data formats work.
Keep application settings in their normal locations; do not redirect XDG/AppData
settings through links into individual builds.

Convenience commands and optional shortcuts live in the application root:
for example `pts.lnk`, `pts.exe` plus `pts.shim`, or a Unix command symlink and
desktop launcher/alias. There are independent local and external
launchers targeting the active application build directly. External shortcuts
are refreshed on installation, without chaining through local ones.
Both sets require ownership tracking, collision checks, and uninstall handling.

Encourage versioned installations. `--prune-old-versions`, a YAML default, or an
interactive choice enables pruning after successful installation and activation.
Keep the new active build and the last active build, not the next lower version
number. An identical reinstall must not shift that history. Preserve modified or
otherwise unsafe-to-remove builds and report them. Without pruning, retain older
builds. Existing owned dotask installations require an explicit compatibility
path; do not adopt directories managed by another installer automatically.

### Rollback is deferred

Do not implement `--rollback` or `--set-version` in the initial feature. Retaining
a previous build does not promise that it can safely be reactivated. The reasons
to defer are:

- Newer applications may migrate settings, databases, or documents incompatibly;
  changing executable paths does not undo those migrations.
- Running applications keep using their current build; switching launchers does
  not switch existing processes.
- Builds can differ in entry points, commands, icons, and shortcut definitions.
- Updating several launchers can be interrupted, leaving mixed activation state;
  recovery and ownership checks require a separate design.
- Retained builds can be missing or modified and must be validated before use.

Users may invoke an older installer, but that is a new installation attempt, not
a guaranteed data downgrade or automatic rollback. Automated failure recovery is
also distinct from a user-requested version switch. Deferring these new engine
features does not remove the current .NET engine's activation recovery behavior.

## Deferred

Service management is deliberately deferred. There is no service provider,
service binary validation, or service installation functionality in this first version.
Also deferred: generated C# properties for YAML keys, remote execution, caching
of completed tasks, runtime dependency scheduling, additional task languages,
third-party/authenticated remote sources, CLI self-update, parallel scheduling,
installation rollback/version-switch commands, a JSON installation interface, and global installation
or publishing as part of repository verification.

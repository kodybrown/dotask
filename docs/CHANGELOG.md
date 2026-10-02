# Changelog

Completed repository work is recorded before handoff. Append dated entries to
the end of `## Unreleased`; update an existing entry for follow-up changes to the
same task. Verification records distinguish automated checks from platform
acceptance, installation, and publication.

## Unreleased

### Repository maintenance

- Moved catalog generation, hashing, and validation into `.tasks/catalog.cs` and
  removed its single-use helper. Documented regeneration before committing
  shared-task changes; normal verification checks freshness without rewriting.
- Added optional Git whitespace checks to `git/check` and made `verify-docs`
  declare and execute that task dependency.
- Included both bundled Windows shim delivery assets and narrow Git ignore
  exceptions so global executable ignores do not omit them from fresh clones.
- Applied the repository's current formatting rules and regenerated catalog and
  shim hash records for the formatted task sources. Moved help-setting character
  escaping into an equivalent local function to make formatter output stable.
- Changed agent workflow to use the primary checkout directly for single-agent
  work; additional worktrees require an explicit request.

### 2026-09-18 Project initialization

- Added `dotask --init` to create missing root configuration and a task directory
  in the invocation directory, with optional `--use-dir` beneath that root.
- Preserves existing project files and refuses invalid configuration, path
  conflicts, and symlink destinations without SDK calls or downloads. Includes
  help, completion, and project setup documentation.
- Verification: the implementation's full `./build.sh` gate passed 250 tests on
  Linux, including 24 initialization cases. Native Windows/macOS acceptance
  remains pending. This entry backfills the already completed initialization.

### 2026-09-18 Repair missing installed launchers

- Reinstalling recreates deleted owned launchers for identical or changed
  builds, including either file in a Windows launcher pair. Modified and
  unowned commands remain protected; no force flag is needed.
- Recovery journals preserve ownership and original physical absence so
  interrupted repairs and removal of missing aliases remain recoverable.
- Verification: `./build.sh` passed 266 tests and all repository checks on Linux,
  including 16 added regressions. A subsequent bootstrap installation restored
  the reported missing command; all 223 installed files and permissions matched
  the host publish output. Native Windows/macOS acceptance remains pending.

### 2026-09-18 Align agent completion with PTS

- Selected PTS's shared terminal-completion module and made changelog updates
  mandatory before every completed-task handoff and in every agent commit.
  Defined dated append order, follow-up entries, local completion boundaries,
  and explicit handoff status; backfilled initialization and installer repair.
- Preserved direct work on `develop`, dotask bootstrap verification, and
  isolated installation tests. Removed an obsolete PTS-style installation path
  from the adapted build-artifact guidance.
- Adopted PTS's commit-before-handoff rule for scoped, verified implementation,
  documentation, and metadata changes, unless the user requests uncommitted work.
  Pushing and other external actions still require separate authorization.
- Verification: `./build.sh verify-docs` and `git diff --check` passed. All six
  selected modules are present; the four unadapted modules match PTS byte for
  byte.

### 2026-09-18 Compact official task source

- Renamed the official shared-task source to `_` throughout selection, cache and
  project paths, lock tracking, completion, reusable task calls, and documentation.
  Omitted sources still select official tasks; help labels them `Official tasks (_)`.
  No migration or compatibility alias is provided for the previous source name.
- Discover the reserved task-root `_` directory while continuing to exclude
  underscore-prefixed helpers, nested `_` directories, and symlinks. Private
  catalog discovery retains its existing helper exclusions.
- Updated shared sources and their repository copies together and regenerated
  the shared catalog. Added coverage for both add spellings, qualified completion,
  the help label, and the discovery exception.
- Verification: focused discovery and official-source tests passed (33 tests).
  `./build.sh` passed all 269 Release tests, formatting, documentation, catalog,
  and bundled shim checks; `git diff --check` passed. Windows and macOS
  acceptance not run.

### 2026-09-18 Declarative YAML task groups

- Added project-authored `.task` YAML groups to discovery, help, completion, and
  execution, sharing canonical names and cycle detection with C# targets.
  Ordered `run` steps accept explicit scalar parameters under `with`; optional
  steps skip only absent targets. Failures stop execution and retain child exit codes.
- Added `require_at_least_1_step` (default false), silent all-skipped success,
  strict YAML schema validation, and nested groups that count as executed steps.
  Groups require no compilation and do not implicitly forward CLI arguments.
- Added a runnable basic example and updated authoring, usage, design, and agent
  references. Pointed reusable-task test resources at canonical `shared-tasks/`
  sources after the checkout moved its copies; downloaded copies still contain
  older source-qualified calls and were preserved with their lock tracking.
- Verification: `./build.sh` passed all 303 Release tests (34 group cases),
  formatting, documentation, catalog freshness, and bundled shim hashes.
  `./build.sh --use-dir examples/basic/.tasks help greet` and the same command
  with `greet` passed; all 94 local links/anchors in changed Markdown resolved.
  `git diff --check` passed. Windows and macOS acceptance remains pending.

### 2026-09-18 Interactive task-group creation

- Added `--create-task` with metadata-only task search, canonical names, optional
  steps, parameter prompts with effective defaults and validation, manual absent
  task entry, step reordering, and `require_at_least_1_step` selection.
- Preview and validate YAML before an explicit final save. Preserve existing
  files/targets, reject symlink destinations, and leave no task file on
  cancellation or declined save. Redirected sessions receive actionable guidance;
  command help and completion remain available without a project.
- Added `:back` at every step-entry prompt to discard an unfinished step and
  return to the add-step choice while preserving completed steps, name, and
  description. Whole-wizard cancellation remains separate.
- Added wizard tests and updated CLI, authoring, and agent documentation.
- Verification: `./build.sh` passed all 326 Release tests (23 wizard cases),
  formatting, documentation, catalog, and shim checks. A Linux PTY session
  verified search, typed parameters, preview-before-write, final save, and
  immediate Ctrl+C cancellation (exit 130). All 83 local links/anchors in changed
  Markdown resolved; `git diff --check` passed. Native Windows/macOS acceptance
  remains pending.

- Follow-up verification: a Linux PTY session backed out of an accidental extra
  step at task selection and saved the completed step and description intact.
  All 28 local links/anchors in the updated authoring and usage references resolved.

### 2026-09-19 Temporary shared-task recovery state

- Remove recognized `.tasks/.dotask` recovery state after successful shared-task
  changes, rollback, or recovery. No-change management operations clean up older
  idle state and recognized orphan staging; dry runs remain read-only.
- Keep pending/failed recovery data and unrecognized contents. Cleanup uses only
  known files and nonrecursive directory removal; unrecognized transaction files
  block reuse rather than being deleted.
- Removed this checkout's pre-existing idle directory after verifying that it
  contained only the generated owner and ignore markers.
- Verification: all 40 focused shared-task tests and `./build.sh` passed;
  the full gate ran 331 Release tests plus formatting, documentation, catalog,
  and shim checks. `git diff --check` passed. Windows/macOS acceptance remains pending.

### 2026-09-20 Verbose output and concise target summaries

- Bare `dotask` and `dotask help` now show only Targets and the detailed-help hint.
  Global `--verbose` restores identity, description, Settings, and Target options;
  target-specific help remains detailed and all help remains metadata-only.
- Added stderr execution diagnostics, inherited by YAML groups and C# nested
  calls, including optional skips and completion. Shared management identifies
  its action and invocation directory. Task output and exit codes are preserved.
- Reserved the `verbose` option name, retained task-defined `-v` aliases, and
  updated completion, tests, and reference documentation.
- Verification: 93 focused tests and `./build.sh` passed; the full gate ran 340
  Release tests plus formatting, documentation, catalog, and shim checks. Live
  example help matched both layouts; all 79 checked local links/anchors resolved.
  `git diff --check` passed. Windows/macOS acceptance remains pending.

### 2026-09-20 Shared-task listing status

- Align descriptions in `--list` with compact `I` (Installed) and `M` (Matches
  cache) columns. Use ✓, X, ?, and — indicators with a legend for matches,
  differences, unknown/untracked copies, and absent tasks. Private tasks compare
  with their originals.
- Compare support files as well as entry points without changing project tracking
  or downloading task files. Keep listing available outside a project.
- Verification: `./build.sh` passed all 342 Release tests, formatting, documentation,
  catalog freshness, and shim checks. A source-built listing confirmed alignment;
  `git diff --check` passed. Windows/macOS acceptance remains pending.

### 2026-09-20 Installer-driven application installation

- Replace the shared .NET install task's direct publication/copy workflow with a
  required project `create-installer`, for both console and GUI applications.
  Add invocation-scoped installer results, platform validation, executable/MSI/
  POSIX script/.NET assembly launchers, and token-preserving `installer-args`
  overrides. Missing, ambiguous, failed, or resultless creators never fall back.
- Add dotask's project-owned creator and standalone installer package, preserving
  existing dotask installation ownership, immutable builds, and recovery records.
  Creation respects evaluated output paths and snapshots a distributable directory.
- Document installer-author responsibilities, custom task delegation, migration,
  argument defaults, and native acceptance limits. Preserve unrelated task changes.
- Verification: `./build.sh` passed 347 Release tests plus formatting, docs,
  catalog, shim, and whitespace checks. Real isolated Linux installation/update,
  repeat activation, installed version and greeting checks passed; evaluated
  output paths remain under `/tmp/_dotnet`. All 81 checked documentation links
  and anchors resolved. Windows/macOS and PTS GUI acceptance remain pending.
  The active user installation was not changed; nothing was published or pushed.

### 2026-09-21 Configurable final installer output

- Add `settings.installer-output` to dotask's creator, configured as
  `artifacts/installers` in this checkout. Relative paths start at the project
  root; absolute paths are supported. Each run preserves previous packages in
  separate OS/architecture and unique-ID directories, and returns the executable
  from the complete final copy. MSBuild output locations remain unchanged.
- Reject final destinations inside either publish directory, including aliases
  through directory links, before copying to prevent recursive packages.
- Verification: `./build.sh` passed all 347 Release tests and required checks
  using the committed catalog temporarily; the unrelated existing catalog edit
  was restored byte-for-byte afterward. Relative/absolute output, spaces/Unicode,
  executable payload, preservation of earlier packages, and symlink-overlap
  rejection passed live Linux checks. MSBuild paths remain under `/tmp/_dotnet`;
  18 local documentation links/anchors passed. Native Windows/macOS acceptance
  remains pending. No application was installed or pushed.

### 2026-09-24 Windows test and compiler path portability

- Normalize compiler source paths before matching the SDK project so portable
  Windows paths receive the library reference, startup helper, and output record.
  Add a direct-compilation regression without normalizing its input in the test.
- Normalize embedded test-resource names and fixture filesystem paths; respect
  platform help-output newlines. Preserve user build-output policy in synthetic
  projects and keep custom publish directories external. Handle read-only Git
  fixture files during cleanup without following symbolic links.
- Align C# formatter line endings with Git's LF checkout policy.
- Verification: `build.cmd` passed all 348 Release tests, formatting, docs,
  catalog, shim hashes, and whitespace checks on Windows; `build.cmd pack`
  produced the local NuGet package. Evaluated build/intermediate/publish paths
  remain under `C:\tmp\_dotnet`. Linux/macOS acceptance remains pending.

### 2026-09-24 Synchronize official task references

- Synchronize the repository's installed build, format, and verify tasks with
  their canonical sources so nested official calls use `_/`; refresh their
  tracking records and the restore dependency's catalog revision.
- Qualify the repository verification and documentation tasks' official calls
  so project-local names cannot intercept them. Keep same-source dependency
  declarations in their documented group/task format.
- Add a regression comparing every installed official C# task with its canonical
  source, and exercise verification with conflicting project-local task names.
- Verification: `build.cmd build` now succeeds with zero warnings or errors.
  All 21 focused repository-task tests passed; `build.cmd` passed all 349 Release
  tests plus formatting, docs, catalog freshness, shim hashes, and whitespace
  checks. All 13 installed official task copies match their canonical sources.
  Evaluated build paths remain under `C:\tmp\_dotnet`. Linux/macOS verification
  of this change remains pending; nothing was installed, published, or pushed.

### 2026-09-24 Record standalone installer design decisions

- Document the agreed Rust installer direction, reusable task boundary,
  unattended inputs, YAML receipts, uninstall/settings behavior, shortcut
  permissions, and previous-build retention policy. Distinguish layout and local
  launcher proposals from settled decisions and from implemented behavior.
- Record why version switching and rollback are deferred, including data
  migrations, running processes, entry-point differences, interrupted activation,
  and missing or modified builds. Link the design from the documentation index.
- Verification: `build.cmd verify-docs`, `git diff --check`, and checks of the
  changed documentation links/anchor passed. Documentation only; no runtime
  behavior, personal notes, installation, or published artifacts changed.

### 2026-09-24 Add reusable Rust installer and shared installer tasks

- Add the standalone Rust engine with YAML configuration and ownership receipts,
  directory/ZIP payloads, unattended execution, explicit interactive inputs,
  profile/default precedence, and validation before installation. Packages retain
  their engine and records for automated uninstall without the original payload.
- Use `installer/` and `app/<version>-<build-id>/`, stable local command launchers,
  independently targeted external launchers, and configurable desktop/Start menu
  shortcuts for console and GUI applications. Preserve settings by default and
  support explicit settings removal and previous-build-preserving pruning.
- Add ownership/conflict protection, activation
  retry records, and uninstall progress. Keep rollback/version switching deferred.
- Move shared installation into `_/dotask-installer/`, add generic packaging and
  uninstall tasks, migrate this checkout through task management, and delegate
  dotask packaging to the Rust engine.
- Include Rust tests/formatting/clippy in the required gate and CI, document the
  YAML schema, and preserve evaluated external MSBuild outputs.
- Emit redirected CLI output as UTF-8 after the full gate exposed lossy Windows
  code-page conversion of Unicode project names; the focused initialization suite
  verifies the correction.
- Remove installer backward compatibility: no legacy JSON import, migration flag,
  compatibility receipt fields, or .NET installation engine/API remain. Preserve
  existing unrecognized directories and launchers, with a fresh-destination
  diagnostic. Move native console/shim acceptance onto the Rust engine and test
  that unsupported formats and migration flags cannot mutate an old installation.
- Verification: `build.cmd` passed 306 .NET and 16 Rust Release tests plus all
  required checks. The shared-task integration installs, launches, and uninstalls
  a real WinExe fixture. A source-created dotask package passed isolated native
  Windows install/version/uninstall checks. Changed documentation links/anchors
  and `git diff --check` passed; evaluated .NET paths remain under `C:\tmp\_dotnet`.
  After removing compatibility, a fresh `build.cmd create-installer` package
  passed `--validate` and rejected the removed migration flag without installing.
  Native Linux/macOS, interactive desktop, and PTS acceptance remain pending.
  No active installation or PTS files changed; nothing was published or pushed.

### 2026-10-02 Scaffold the Rust CLI in a shared Cargo workspace

- Add `src/dotask-cli`, producing a native `dotask` development preview with
  help/version output and explicit failure for unsupported task/management
  requests. Preserve the working C# CLI, C# authoring library, shared tasks,
  bootstrap, and installation entry points.
- Add the root Cargo workspace, move the installer's lockfile and release
  profile to the root, and preserve all pinned installer dependencies. Add the
  `rust-cli` build/check task with external output and require its tests,
  formatting, and clippy checks in repository verification and CI.
- Record the agreed CLI transition and task-language design separately from
  available commands. Document preview build/run commands and correct the
  repository's Rust toolchain prerequisites.
- Verification: `build.cmd` passed 306 .NET Release tests, 16 Rust installer
  tests, 3 Rust CLI tests, and all formatting/docs/catalog/shim/whitespace gates
  on Windows x64. `build.cmd rust-cli --verify`, the focused repository gate
  regression, native preview help/version, 84 documentation links/anchors, and
  `git diff --check` passed. Evaluated .NET and Cargo outputs remain external.
  Native Linux/macOS acceptance remains pending. No active installation changed;
  nothing was published or pushed.

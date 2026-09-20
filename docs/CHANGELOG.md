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

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

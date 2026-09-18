# Changelog

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

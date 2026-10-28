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
- Isolate the installer workflow test's shared-task cache and private-task
  directory. Assert that installation populates its fixture-owned catalog so
  an older user catalog cannot hide the installer tasks or receive test data.
- Verification: `build.cmd` passed 306 .NET Release tests, 16 Rust installer
  tests, 3 Rust CLI tests, and all formatting/docs/catalog/shim/whitespace gates
  on Windows x64. `build.cmd rust-cli --verify`, the focused repository gate
  regression, native preview help/version, 84 documentation links/anchors, and
  `git diff --check` passed. Evaluated .NET and Cargo outputs remain external.
  Reproduced the reported installer test failure with a temporary copy of an
  older catalog; the focused `RustInstallerWorkflowTests` and complete
  `build.cmd` then passed with that stale parent cache after the isolation fix.
  The real user catalog and stale reproduction catalog remained byte-identical.
  Native Linux/macOS acceptance remains pending. No active installation changed;
  nothing was published or pushed.

### 2026-10-02 Execute existing C# tasks from the Rust CLI

- Add native project discovery, task selection, execution sessions, YAML group
  sequencing, and nested-call dispatch. Preserve C# task source and APIs, settings
  snapshots, parameter binding, structured installer results, process arguments,
  standard streams, and task exit codes. Add basic metadata-only project/target
  help and explicit rejection of unsupported management/completion commands.
- Add the separate `Dotask.CSharpHost` with a versioned file protocol, sharing
  Roslyn metadata, YAML, option/requirement validation, and isolated SDK compilation
  code. Stage it from evaluated `PublishDir` beside the native preview without
  packaging or invoking the managed CLI dispatcher.
- Generalize internal nested-call context to a CLI executable and argument prefix.
  Add process-group/job cancellation and preserve full Windows exit codes.
  Emit redirected C# task output as UTF-8 for lossless native pipelines.
- Verification: `build.cmd` passed 316 .NET Release tests (including 10 native
  execution cases), 16 Rust installer tests, 3 Rust CLI smoke tests, and all
  formatting/clippy/docs/catalog/shim/whitespace checks on Windows x64. The focused
  `RustCliTests` suite and documented native greeting command also passed. Native
  Ctrl+C tests cover direct/nested tasks, uncooperative descendants, and context
  cleanup. Evaluated .NET and Cargo outputs remain external. Native Linux/macOS
  acceptance remains pending; no active installation changed and nothing was
  published or pushed.

### 2026-10-02 Implement native CLI management, configuration, help, and completion

- Implement project initialization, shared-task list/save/add/sync/remove,
  completion for all four shells, detailed help and verbose summaries, and the
  interactive YAML group wizard in Rust. Move configuration/group parsing,
  option binding, and requirement validation into the native CLI; keep the
  .NET support host limited to Roslyn metadata and isolated C# compilation.
- Preserve shared-task dependency handling, hashes, lockfiles, ownership,
  local-edit protection, reviewed merges, and recovery journals. Verify
  interoperability with the C# reference and preserve lockfile bytes on no-op
  sync. Keep help/completion metadata-only and initialization SDK-free.
- Honor Cargo's evaluated target directory in build tasks, support staging,
  installer packaging, and native tests. Preserve `.cargo/config.toml` and
  `rustfmt.toml`; format both Rust workspace members with the supported settings.
- Verification: `build.cmd` passed 407 .NET Release tests and 25 Rust tests on
  Windows x64, plus formatting/clippy/docs/catalog/shim/whitespace checks.
  Differential help/completion/YAML checks, reference revision/journal
  interoperability, a real terminal wizard save/cancel check, the native greeting,
  65 documentation links/anchors, and evaluated external output paths passed.
  Stable rustfmt warns about nightly-only options. Native Linux/macOS and remote
  catalog acceptance remain pending; the C# CLI remains the packaged application.
  No active installation changed, and nothing was published or pushed.

### 2026-10-05 Document how to continue the Rust transition

- Add an indexed continuation runbook with Linux prerequisites and acceptance
  commands, isolated native/shared-task checks, macOS CI and manual acceptance
  boundaries, and the ordered packaging, Rust SDK, variant, and task-duplication
  work. Keep future interfaces distinct from current CLI behavior.
- Verification: `build.cmd verify-docs`, 35 relative links/anchors, syntax-only
  checks of all 10 Bash blocks, and `git diff --check` passed. Linux/macOS commands
  are documented for later execution; this documentation change does not add
  platform acceptance evidence.

### 2026-10-05 Author Rust tasks and package the native CLI

- Add `.rs` task discovery, leading YAML metadata, and the bundled Rust task SDK.
  Keep help/completion compiler-free; compile immutable source/support/SDK
  snapshots in an external cache. Preserve typed arguments, nested C#/Rust calls,
  settings snapshots, failures, cancellation, and installer-result isolation.
- Convert repository `installer-engine`, `rust-cli`, `create-installer`, `verify`,
  `verify-docs`, `catalog`, and `shim` tasks and Rust build support to Rust. Add a
  local Rust `pack` target that returns the native standalone installer package.
  Preserve maintained shared C# tasks, library, catalog, and ownership contracts.
- Bootstrap the native runner with staged SDK and C# support. Package the native
  CLI and support from fresh evaluated publish inventories, excluding stale files.
  Normalize retained-uninstaller identity through Windows junction paths so
  self-uninstall can remove owned files while preserving unowned files.
- Update authoring, usage, installation, design, agent guidance, and the Windows
  continuation plan for the native packaging cutover.
- Verification: `build.cmd` passed 416 .NET Release tests and 27 Rust tests on
  Windows x64, plus formatting/clippy/docs/catalog/shim/whitespace checks.
  `build.cmd pack` in both self-contained modes and relocated temporary
  install/reuse/update/uninstall acceptance passed, including mixed-language execution, Unicode arguments,
  self-contained C# help without `dotnet` on PATH, and user-file preservation.
  Both LLVM-rebuilt shim binaries were unchanged; evaluated output paths remain
  external and the Bash launcher passed syntax checking. Linux/macOS acceptance
  remains deferred. No active installation changed; nothing was published or
  pushed.

### 2026-10-05 Use common task headers and ship only application helpers

- Read versioned ordinary-comment YAML headers for C# and Rust tasks directly
  in the native CLI, including help/completion and private catalogs without
  language toolchains. Convert repository, canonical shared, example, and test
  task headers; preserve code documentation and existing task/helper APIs.
- Delegate C# compilation directly to the installed .NET SDK, retaining isolated
  external caches, locking, invocation snapshots, SDK directives, nested calls,
  and cancellation. Remove the separate managed support host and Roslyn package
  dependency; keep the C# CLI as the behavior reference.
- Ship only the native command, C# helper DLL, and Rust helper source crate.
  Remove self-contained packaging and its project default. Preserve installer
  ownership, receipts, immutable builds, update recovery, and retained uninstall.
  Keep original shared tracking baselines and private user tasks unchanged.
- Verification: `build.cmd` passed 419 .NET Release tests and 31 Rust tests on
  Windows x64, plus formatting/clippy/docs/catalog/shim/whitespace gates.
  `build.cmd pack` and relocated temporary install/reuse/update/uninstall
  acceptance passed, including Unicode, mixed-language calls, toolchain-free C#
  help, and unowned-file preservation. The exact four-file application payload
  totals 3.56 MiB; a fresh single-build installation including management files
  measured 6.09 MiB. Both LLVM-rebuilt shims were byte-identical; evaluated output
  paths, 100 relative links/anchors, Bash syntax, and `git diff --check` passed.
  Linux/macOS acceptance remains deferred. No active installation changed;
  nothing was published or pushed.

### 2026-10-05 Measure Windows C# Native AOT and Rust CLI sizes

- Record an isolated Windows x64 Release experiment from committed baseline
  `1df4d14`, preserving executable byte counts, MB/MiB, ratios, deployment
  companions, debug-symbol sizes, artifact hashes/paths, and reproduction commands.
- The strict C# AOT publish fails JSON trimming/dynamic-code diagnostics; the
  warning-preserving measurement publish emits 17,403,392 bytes but crashes on
  sample discovery/execution. Record the additional managed-library location
  incompatibility without changing source or claiming AOT support.
- The working full-extraction self-contained C# executable is 90,726,248 bytes;
  the static-CRT Rust executable is 3,717,632 bytes. With installed toolchains,
  retained non-debug payloads are 17,306,045 bytes for framework-dependent C# and
  20,417,652 bytes for Rust with this baseline's C# support and bundled Rust SDK.
- Verification: help/version/discovery/greeting smoke tests pass for all three
  working variants; AOT help/version pass and its two crashes are recorded.
  `build.cmd verify-docs`, changed relative links, evaluated external .NET output
  paths, native import inspection, and `git diff --check` passed. Documentation
  only; original measurements remain attributed to `1df4d14`, before the
  common-header/minimal-helper packaging in `7b28895`. Accepted local integration
  preserves the report and external artifacts; no install, publish, push, or
  changes to the other task's implementation.
- Document the non-force Git cleanup fallback after the app archive tool rejects
  the chat's primary checkout; preserve matching baseline example sources and
  confirm all measured executable hashes remain unchanged. Rebased/integrated
  documentation gates and 26 relative links/anchors pass.

### 2026-10-05 Rename the C# helper assembly to Dotask.dotnet

- Emit `Dotask.dotnet.dll` and update native/reference task compilation,
  bootstrap staging, installer packaging, bootstrap fixtures, and current
  guidance. Preserve the `DoTask` namespace, task APIs, and package ID.
- Verification: a clean external Release rebuild and `build.cmd` passed all
  419 .NET tests, 31 Rust tests, and required formatting/clippy/docs/catalog/
  shim/whitespace gates on Windows x64. `build.cmd pack` and relocated temporary
  install/reuse/update/uninstall acceptance passed with the exact four-file
  payload, the new DLL, and no old DLL. Output-path evaluation, Bash syntax,
  mixed-language execution, Unicode, and `git diff --check` passed.
  No active installation changed; nothing was published or pushed.

### 2026-10-05 Add console installer defaults and Windows PATH prompts

- Default the console installer to interactive execution, configurable through
  `installer.yaml` and explicit `--interactive` / `--non-interactive` overrides.
  Use ordered path questions, Y/N defaults, a location summary and confirmation,
  and print `Canceled` on Ctrl+C, EOF, or declined confirmation.
- Always create a command in the chosen application root. Make the additional
  shim optional, default No, and suggest the selected root's parent. Permit
  parent-directory shims while protecting managed builds and unowned files.
  Configure labels/defaults through YAML; ignore the user's `BIN` convention.
- Offer Windows user PATH addition for the selected command directory, skipping
  that question when already on persistent PATH. Preserve existing entries and
  registry string types, notify Windows, and retain PATH entries on uninstall.
  Update unattended installer/shared-uninstall guidance and regenerate its
  canonical catalog without rewriting project ownership baselines.
- Verification: `build.cmd` passed 419 .NET and 39 Rust tests plus all required
  formatting/clippy/docs/catalog/shim/whitespace checks. Real console prompts,
  Ctrl+C, final cancellation, root-only and parent-shim layouts passed; native
  PATH writes were tested in a disposable registry key. `build.cmd pack` and
  relocated temporary install/reuse/update/uninstall acceptance passed, including
  mixed-language tasks, Unicode, and unowned-file preservation. Evaluated output
  paths remain external. No active installation or user PATH changed; nothing
  was published or pushed.

### 2026-10-05 Allow launcher location updates and configurable menu shortcuts

- Allow updates to move, enable, or disable an additional command. Use actual
  receipt-owned launcher paths, lock old and requested directories, refuse
  unowned destinations/modified files, and remove only obsolete owned launchers.
  Preserve activation recovery and require identical destinations on retry.
- Add safe configurable shortcut names through YAML input templates and
  `--shortcut-name`, including interactive validation/retry. Windows Start Menu
  links optionally nest as `name/name.lnk`, default No; retain flat desktop/local
  and Linux menu entries. Track only installer-created menu folders and remove
  them only when empty. Dotask itself declares no menu shortcut.
- Verification: `build.cmd` passed 419 .NET and 46 Rust tests and every required
  formatting/clippy/docs/catalog/shim/whitespace gate. Real console naming/nesting,
  native Shell target inspection/execution, and owned-folder cleanup passed.
  `build.cmd pack`, a temporary upgrade from the earlier pre-prompt package,
  command relocation/disable, and relocated mixed-language package acceptance
  passed. No active installation or user PATH changed; nothing was published or
  pushed. Linux/macOS acceptance remains deferred.

### 2026-10-05 Polish the installer console layout

- Add application, settings, and summary sections with separators spanning the
  current visible console width minus one column. Indent prompts/results/errors,
  wrap banner descriptions, and include cancellation under the summary. Preserve
  plain unattended output and existing installation/ownership behavior.
- Add optional installer application copyright metadata and supply DoTask's
  display name, copyright, and description in its package. Document the creator's
  explicit mapping to installer YAML, shortcut permissions versus defaults, and
  the second shim's existing always-permitted behavior.
- Verification: `build.cmd` passed 419 .NET and 48 Rust tests and all required
  gates. Real Windows console success/error/Ctrl+C layouts, width checks, and
  metadata rendering passed. `build.cmd pack`, relocated package acceptance,
  and upgrade/command relocation from the earlier package passed. Evaluated
  build outputs remain external. No active installation or user PATH changed;
  nothing was published or pushed. Linux/macOS acceptance remains deferred.

### 2026-10-06 Match package and installed names to application build metadata

- Preserve dotask's major/minor prefix and default its public version to
  `major.minor.yyMM.ddhh`, with embedded UTC `YYDDD-HHMM`, optional Git revision,
  and an explicit dirty marker. Assign the minute before compilation, advance
  an occupied minute with a warning, and verify binary/package metadata equality.
- Create named installer directories directly, and use the same installed build
  name without the application prefix. The generic engine and shared creator use
  the packaged application's own version and optional Git information, support
  arbitrary version labels, and encode unsafe directory characters. Preserve
  internal integrity fingerprints, identical package/build reuse, collision
  refusal, existing receipts, and immutable build ownership.
- Add structured package results for the shared creator and local version/Git/
  stamp overrides. Keep technical Cargo/helper-library versions independent.
  Regenerate the canonical catalog without rewriting tracking baselines.
- Verification: `build.cmd` passed 419 .NET and 52 Rust tests plus every required
  gate. Calendar/date rollover, custom version/no-Git, matching named directories,
  same-minute warning/advance, payload collision protection, shared C# creation,
  and relocated mixed-language install/reuse/update/uninstall acceptance passed.
  Output paths remain external and Bash syntax passes. No active installation or
  user PATH changed; nothing was published or pushed. Linux/macOS remains deferred.

### 2026-10-06 Configure installer creation and organize helpers by language

- Put dotask-specific installer metadata, prompts, and defaults in
  `.tasks/installer.yaml`, wired through nested `settings.installer`. Keep the
  local Rust creator responsible for app builds and staging; delegate packaging
  to the maintained shared creator and forward its structured artifact.
- Read partial source YAML through the installer engine without prompting or
  installing, then fill dynamic metadata in a temporary configuration. Update
  shared creator settings and regenerate its catalog without changing ownership
  tracking baselines.
- Ship the helper DLL at `sdk/dotnet/Dotask.dotnet.dll` and Rust source crate
  under `sdk/rust/`. Update native language handlers, bootstrap staging, build
  tasks, authoring guidance, and package documentation together.
- Verification: `build.cmd` passed 419 .NET and 53 Rust tests plus all required
  gates. Packaging via the local Rust/shared C# chain and temporary named
  install/reuse/update/uninstall acceptance passed, including exact four-file
  payloads and installed Rust-to-C# calls using the new helper locations.
  Evaluated outputs remain external; Bash syntax and document links passed.
  No active installation or user PATH changed; nothing was published or pushed.
  Linux/macOS acceptance remains deferred.

### 2026-10-07 Configure installer build steps and preserve nested runtime settings

- Separate source installer YAML into application metadata, ordered `build-steps`,
  and destination `runtime` configuration. Run producer tasks, gather their
  expected files, and fail on producer errors, missing files, or payload conflicts.
  Preserve the same nested runtime mapping in distributed YAML; reject source
  build steps and old root-level runtime fields in the destination installer.
- Replace shared installer tasks with Rust under `simple-installer`, move assembly
  into a separate build-machine executable, and remove the shipped installer's
  packaging command. Add optional ZIP/7z tasks preserving the top-level directory
  and publishing completed archives without overwriting an existing output.
- Configure SDK selections in YAML and prepare them through `build-sdks`. Keep
  bootstrap native and minimal, forward all arguments including an empty list,
  use `build` and `build-installer`, and remove the local `pack` task. Keep NuGet
  tasks in the consumer catalog. Read application version/Git naming settings
  during installer creation, support UTC version tokens including ordinal day,
  and refuse an existing version directory without advancing the timestamp.
- Support Rust shared catalog entries and source-prefixed support files. Refresh
  the catalog and generate tracking for the reviewed, identical repository task
  copies through the native shared manager. Preserve the user's Rust verification
  placeholders and run SDK integration tests from a Rust-authored direct SDK task.
- Verification: `build.cmd verify` passed all 420 .NET Release tests and 65 Rust
  tests, formatting/clippy, docs, catalog, shim, and whitespace gates on Windows
  x64. Temporary application creation, matching embedded build metadata, ZIP/7z
  directory structure, collision refusal, GUI launch/shortcuts, retained uninstall,
  and bootstrap argument forwarding passed. Evaluated .NET and Cargo outputs
  remain external. No active installation, user PATH, publication, or push changed.
  Linux/macOS acceptance remains pending.

### 2026-10-07 Extract doinstall into an independent repository

- Move the standalone installer, builder, runtime schema/design ownership,
  receipt/recovery logic, native shim sources/assets, and installer behavior
  tests into the independent doinstall repository. Remove the installer Cargo
  member and private build/shim tasks from dotask; its workspace now contains
  only the CLI and Rust SDK.
- Keep Rust shared adapters under doinstall and move technology-neutral artifact
  launching to installer/install. Build an explicitly configured external checkout
  through its own Cargo development tool, with optional source/environment overrides.
  Keep producer/gather/archive orchestration in task YAML and use external tools
  for SDK adapter integration. Refresh catalog/tracking for reviewed identical copies.
- Pin CI's doinstall checkout to b3150eef78c342e6b81b0d4424ae86c432d50d3b.
  The new repository remains local until separately pushed; remote CI cannot
  fetch that pin until doinstall is published. No source download occurs implicitly
  in local build tasks. Preserve direct checkout workflow and user verification
  placeholders.
- Normalize portable Rust SDK paths under Windows verbatim roots while preserving
  UTF-16 filenames and raw process arguments. Add a focused file-creation regression
  for the staging path issue exposed by full external-tool package creation.
- Verification: build.cmd verify passed all 419 .NET Release tests and 15 Rust
  tests plus formatting/clippy/docs/catalog/whitespace gates. Doinstall independently
  passed 52 Rust tests and its shim integrity/PE checks. Temporary installer creation,
  matching embedded build metadata, install/launch/retained uninstall, and external
  GUI adapter/shortcut tests passed. Cargo outputs remain external. Shim delivery
  bytes remain unchanged and their integrity test now lives in doinstall. No active
  installation, user PATH, publication, or push changed; native Linux/macOS/ARM64
  acceptance remains pending.

### 2026-10-07 Remove the retained C# executable

- Remove `src/Dotask.Cli`, its solution/test references, obsolete root project
  setting, unused managed bootstrap hook, and retired friend assemblies. Retain
  `src/Dotask` as `Dotask.dotnet.dll` and preserve C# consumer task support.
- Move canonical completion scripts into the Rust CLI without changing their
  bytes. Run all CLI fixtures and nested calls against immutable native runner
  snapshots; retain 23 fixed pre-removal help/completion/YAML output baselines.
- Migrate discovery/configuration/binding, help wrapping, wizard, transaction,
  compile-all published C# task, process, and concurrent compilation coverage
  into native tests and SDK-built helper probes. Remove duplicate reference
  suites and test-only dependencies on executable internals.
- Preserve optional null project identity, report full-name collisions with
  their file-level diagnostic, and clean recognizable orphan staging on empty
  no-op syncs while preserving dry-run and unknown-file protections.
- Prepare source C# SDKs from the bootstrap checkout even with a consumer
  `--use-dir`, keeping settings and call chains independent and preserving
  preparation failures. Launchers remain minimal and forward every argument;
  SDK file selections remain in YAML. Refresh current development/usage docs.
- Verification: `build.cmd verify` passed 250 .NET Release tests and 34 Rust tests,
  rustfmt/clippy/docs/catalog/whitespace gates on Windows x64. The separate .NET
  formatting check, 83 local Markdown links/anchors, bare-launcher listing,
  source greeting and nested-write examples passed. Evaluated .NET/Cargo outputs
  remain external. Native Linux/macOS/ARM64, live HTTPS, and interactive terminal
  acceptance remain pending. Doinstall/Inno task sources and the active user
  installation were untouched; nothing was published or pushed.

### 2026-10-09 Select C# and Rust task implementations

- Support coexisting C#, Rust, and YAML task files with explicit extension-qualified
  full names/shortcuts. Keep extensionless execution unambiguous, preserve exact-name
  precedence and same-language collision errors, and distinguish implementations
  in nested calls, existence queries, optional execution, groups, and cycle detection.
- Merge base-name and implementation-specific defaults case-insensitively, with
  qualified defaults taking precedence. Use the same effective defaults in help,
  binding and the group wizard. Keep help/completion metadata-only.
- Add group selections such as `--add git`, explicit files, and `--lang csharp|rust`
  to shared list/save/add/sync/remove and local completion. Filter selected roots;
  declared dependencies retain their implementation. Reject ambiguous dependencies.
- Keep catalog/lock schemas and revision serialization. Qualify coexisting catalog
  entries, preserve existing recorded keys/revision identities by their owned entry
  files, and preserve tracking bytes/comments on no-op sync. Prevent adoption,
  overwriting another implementation, malformed/double entry ownership, unintended
  sibling installation during sync, and removal of required/local-edited files.
- Add canonical `git/check.cs` alongside the Rust implementation and regenerate the
  catalog. Its C# behavior checks Git availability and optional staged/unstaged
  whitespace. Existing installed copies and repository tracking stay unchanged;
  doinstall/Inno sources and their catalog entries are untouched.
- Verification: `build.cmd verify` passed 259 .NET Release tests and 34 Rust tests
  on Windows x64, including mixed-language calls/cycles, qualified defaults,
  ownership, no-op sync, filtered updates/removal, exact dependencies, source
  language changes, and catalog ambiguity. Rustfmt/clippy/docs/catalog/whitespace
  and separate .NET formatting passed. Native Linux/macOS/ARM64, live HTTPS and
  interactive-terminal acceptance remain pending. No active installation,
  publishing or push occurred.

### 2026-10-09 Select a task implementation with --lang

- Add execution/help `--lang csharp|rust` before or after a target, with separated
  or equals syntax. Resolve only matching implementations; reject missing,
  ambiguous and conflicting filename/language selectors before task side effects.
  Filter bare project help and completion using the same selection rules.
- Reserve `lang` in task option metadata and consume the selector in the common
  CLI parser used by shared management. Do not pass it to task parameters, persist
  it in YAML, or inherit it in nested calls, dependencies, or group children.
  Directory names remain unknown targets rather than implicit execution groups.
- Preserve the fixed pre-removal completion baseline for existing suggestions;
  independently test the additive selector. Refresh usage/authoring guidance.
- Verification: `build.cmd verify` passed 269 .NET Release tests and 34 Rust tests
  on Windows x64, rustfmt/clippy/docs/catalog/whitespace gates, and the separate
  .NET whitespace-format check. Focused execution/help/completion/shared tests
  passed 101 cases. The requested `git/check --lang rust --whitespace` source
  invocation and 83 local Markdown links/anchors passed. Native Linux/macOS/ARM64,
  live HTTPS and interactive-terminal
  acceptance remain pending. Shared sources, doinstall/Inno adapters, tracking and
  the active installation are unchanged; nothing was published or pushed.

### 2026-10-11 Declarative process tasks and a toolchain-independent Git check

- Extend `.task` files with typed options and direct `exec` steps containing an
  executable, argument list and project-root-relative cwd. Mix commands with
  existing task calls and guard either action with a boolean parameter. Preserve
  literal arguments, streams, failure exit codes and process-tree cancellation;
  no shell, interpreter, language compiler or SDK preparation is implicit.
- Distribute YAML tasks through the shared catalog and management commands with
  the existing ownership/hash protections. Replace both reusable Git code files
  and the repository's installed Rust copy with `git/check.task`, retaining Git
  availability and optional staged/unstaged whitespace checks. Keep `.task` as
  the extension and document explicit replacement for existing consumer copies.
- Isolate native CLI smoke-test cwd and caches so shared-task probes cannot
  mutate the checkout. Keep help/completion metadata-only and preserve existing
  task-call regression coverage.
- Verification: `build.cmd verify` passed 286 .NET Release tests and 34 Rust tests
  on Windows x64, rustfmt/clippy/docs/catalog/whitespace gates. Separate .NET
  whitespace formatting passed. Standalone Git help and whitespace execution
  passed with only the native executable, Git on PATH, and no SDK files.
  Final `build.cmd verify-docs`, `git diff --check` and 86 local Markdown
  links/anchors passed; evaluated MSBuild/Cargo output paths remain external.
  Native Linux/macOS/ARM64, live HTTPS and interactive-terminal acceptance remain
  pending. Doinstall/Inno sources and catalog entries were untouched; no active
  installation, publication or push occurred.

### 2026-10-12 Declarative text normalization task

- Replace the reusable `text/fixeol.cs` with `text/fixeol.task`, invoking the app
  directly without compiling task code or preparing C#/Rust helpers. Preserve
  the three recursive passes, UTF-8 encoding, LF/CRLF option, batch-file CRLF,
  `Makefile` selection and all generated/task/output exclusions.
- Require `fixeol` on PATH: absence now fails rather than warning and skipping.
  Preserve failed-process exit codes and stop remaining passes. Document explicit
  replacement of old tracked consumer copies; no ownership migration occurs.
  Keep `dotnet/format`'s optional target lookup and `--verify` behavior, with
  regression coverage calling a YAML normalization task.
- Verification: `build.cmd verify` passed 290 .NET Release tests and 34 Rust tests
  on Windows x64, rustfmt/clippy/docs/catalog/whitespace gates. All six focused
  text/format tests, .NET whitespace formatting, `git diff --check` and 32 local
  Markdown links/anchors passed; evaluated MSBuild/Cargo outputs remain external.
  Real fixeol 3.21 acceptance passed in temporary projects for UTF-8
  conversion, LF/CRLF, `Makefile`, batch CRLF and exclusions from a nested cwd,
  using a standalone native dotask with no SDK files. Native Linux/macOS
  acceptance remains pending. No active installation, publication or push occurred.

### 2026-10-12 Separate doinstall tooling and declarative assembly

- Replace the reusable and tracked `doinstall/assemble.rs` adapter with
  `assemble.task`, executing prebuilt tools without a Rust/.NET task compiler.
  Default the builder to PATH, accept explicit paths, and require the actual
  matching runtime executable for packaging rather than a command shim. Keep
  dynamic build/gather, source-build and artifact-launching tasks in Rust.
- Make dotask's own installer creation consume separately supplied tools through
  options/settings and build values; remove automatic doinstall builds from its
  source installer template. Keep consumer tooling out of dotask's app payload.
  Require a prebuilt pair for adapter verification, with explicit development/CI
  preparation. Pin CI to the newest remotely available doinstall revision,
  db11b9e3401e770ab4d491b10aa79ff5501403e1; latest local runtime acceptance used
  65a2d2b. Dotask owns the optional shared adapter and add/sync lifecycle.
- Update authoritative installation/agent/design guidance, repository policy,
  shared catalog, identical installed copies and tool-managed tracking. Consumer
  implementation replacement remains explicit and preserves local ownership.
- Verification: build.cmd verify passed 292 .NET Release tests and 34 Rust tests
  on Windows x64, rustfmt/clippy/docs/catalog/whitespace gates. Separate .NET
  whitespace formatting and 72 local Markdown links/anchors passed. Focused
  adapter tests cover PATH/explicit builders, Unicode/spaces, nested cwd, help,
  missing tools, collision refusal, and assembly without SDK files/toolchains.
  Consumer build/gather/install/launch/retained-uninstall integration passed.
- Created and validated dotask's installer with an unavailable source checkout
  and separately staged prebuilt tools; verified the runtime hash and payload
  exclusions. Installed dotask temporarily, used its command to add the shared
  adapter and create a second application's installer, then installed, launched
  and uninstalled both applications with user PATH unchanged. Output policies
  remain external. Native Linux/macOS/ARM64 and interactive-terminal acceptance
  remain pending; no active installation, publication or push occurred.

### 2026-10-12 Complete local dotask installer workflow

- Add checkout-local `create-dotask-installer.rs`, invoked with
  `build.cmd create-dotask-installer` (Unix: `./build.sh create-dotask-installer`).
  Build and temporarily stage the selected doinstall checkout, verify doinstall,
  run dotask's full gate against that exact pair, then create and return dotask's
  installer. Stop on failures and never launch the resulting installer. Forward
  optional source, application-version and build-stamp selections. No shared task
  or catalog changes, project settings rewrites, or global environment mutation.
- Add `--doinstall-tools` to local `verify` and `test-sdks`, taking precedence over
  `DOINSTALL_TOOL_DIR`, so the workflow can pass its staged pair through invocation
  parameters. Document the one-command workflow and retain the prebuilt-only
  `create-installer` entry point for independent-tool users.
- Verification: the complete `build.cmd create-dotask-installer` workflow passed
  on Windows x64, including doinstall's 70 Rust tests and dotask's 292 .NET Release
  and 34 Rust tests, formatting/clippy/docs/catalog/whitespace gates, and actual
  installer creation. Package validation, runtime hash/payload checks, final
  verify-docs, 49 local Markdown links/anchors and git diff --check passed. The
  missing-source check stopped before later stages and removed its temporary
  tools. Temporary tools were also removed after success; evaluated MSBuild/Cargo
  outputs remain external. Native Linux/macOS/ARM64
  acceptance remains pending. No active installation, publication or push occurred.

### 2026-10-13 Declarative archives and staged process output

- Replace canonical and installed `archive/create-zip.rs` / `create-7z.rs` with
  `.task` process definitions and remove their Rust support helper. Keep source,
  output and archiver options, the named top-level folder, new/outside-source
  destinations, failure exit codes and publication without replacing an existing
  or concurrently created archive. Regenerate the catalog and use tool-managed
  removal/addition to update tracked copies; consumer replacements remain explicit.
- Add generic, bounded `path-parent` / `path-name` references for existing paths,
  and process `output-file` declarations supplying a staged argument. Validate
  metadata without filesystem execution; retain literal/native argument values.
  Resolve output boundaries through linked parents before creating directories,
  require an ordinary produced file, and publish on the destination filesystem.
  Keep the implementation independent of archive formats and language compilers.
- Document authoring, hard-link requirements, compiler-free archive execution,
  and source/copy replacement guidance. Preserve explanatory archive comments in
  the YAML tasks and generic native implementation.
- Verification: build.cmd verify --doinstall-tools C:/tmp/_rust/doinstall/target/release
  passed 311 .NET Release tests and 39 Rust tests on Windows x64, rustfmt/clippy,
  docs/catalog/whitespace gates. All 35 focused archive/process tests, separate
  .NET whitespace formatting, 43 local Markdown links/anchors, final verify-docs,
  git diff --check and evaluated external MSBuild/Cargo paths passed.
  Real 7-Zip 26.02 created/extracted ZIP and 7z from a nested invocation using a
  standalone native CLI with no SDK files; Unicode/spaces, named folder contents,
  collision refusal, staging cleanup, and direct/junction source-boundary refusal
  passed. Native Linux/macOS/ARM64 acceptance remains pending. No active
  installation, publication or push occurred.

### 2026-10-13 Native file actions and workflow results

- Add native YAML read/write/render/replace/copy/directory actions with named
  execution-local results, typed boolean guards, explicit result/environment
  references, and selected-value forwarding to nested tasks. Keep execution
  independent of a language SDK and avoid an expression/interpolation language.
- Guard changes to existing files with observed SHA-256, stage writes on the
  destination filesystem, preserve supported text encoding/EOL/final-newline
  policies, skip identical writes, and reject linked paths and unsafe rewrites.
- Add explicit process environment, bounded accepted UTF-8 stdout capture, and
  owned-process-tree timeouts. Validate captured output before publishing staged
  files; require the outer controller to own nested Unix workflow timeouts.
- Document the primitive contracts, configuration ownership boundary, and native
  server-workflow use. Restore existing Unicode text after reproducing the
  WeekendTech MCP write corruption and receiving a scoped file-tool exception.
- Verification: build.cmd verify --doinstall-tools
  C:/tmp/_rust/doinstall/target/release passed 311 .NET Release tests, 51 Rust
  tests, formatting, clippy, documentation, catalog, and whitespace checks on
  Windows x64. Native fixtures cover no-SDK execution, changed/no-op rendering,
  guards, JSON results, invalid capture publication, timeouts, and metadata-only
  POSIX-mode help. Evaluated MSBuild outputs remain under C:/tmp/_dotnet.
  Native Linux/macOS/ARM64 acceptance remains pending.

### 2026-10-14 Existing-installer selection and guided setup

- Make shared `installer/install` run an existing `installer.installer_file`
  literal path or host/version template without invoking a creation task.
  Select numeric highest versions, reject equal-version ambiguity, preserve exact
  argument tokens, and honor native Windows EXE activation manifests.
- Add explicit Rust/C# successful-build recording with shared, atomic per-host
  state and SHA-256 validation under ignored `artifacts/.dotask/installers/`.
  The doinstall creator records only after its complete packaging sequence passes;
  invocation results and user configuration remain separate.
- Add bounded project discovery and terminal setup when configuration is absent.
  Show examples and candidates, confirm use/save, infer a starting template or
  offer an editable placeholder. Preserve comments/BOM/EOL/final-newline state;
  refuse unsupported YAML, concurrent edits, linked state and legacy-config
  conflicts. Redirected execution fails with guidance without changes or launch.
- Consolidate local creation in `create-dotask-installer`, remove the redundant
  local `create-installer` wrapper, and use the complete workflow in CI. Update
  installer/SDK/agent references, shared catalog, identical installed sources
  and tool-managed tracking; doinstall remains independently installed.
- Verification: `build.cmd create-dotask-installer` passed doinstall's 100 Rust
  tests, dotask's 312 .NET Release and 59 Rust tests, formatting/clippy/docs/
  catalog/whitespace gates and actual packaging on Windows x64. Eight focused
  .NET tests, nine SDK unit tests, separate .NET whitespace formatting and 82
  local Markdown links/anchors passed; evaluated outputs remain external.
  Package validation, temporary install/launch, installed C#/Rust recording
  through a junction path, parameterless consumer install, and retained uninstall
  passed without active-install or user-PATH changes. Real terminal discovery,
  template save/use and placeholder setup preserved comments/BOM/CRLF/final
  newline state. A sequential rerun resolved an overlapping smoke-check executable
  lock. Native Linux/macOS/ARM64, elevated interactions and remote CI remain
  pending. No active installation, publication or push occurred.

### 2026-10-14 Target-scoped task settings

- Support `targets.<full-name>.settings` beside option `defaults` for local and
  shared tasks. Resolve shared values, base-name settings and extension-qualified
  settings through one native implementation for C#, Rust and YAML tasks,
  including setting requirements, process/file references and forwarded values.
- Merge mappings recursively with case-insensitive keys; replace arrays, scalars
  and nulls. Keep original shared settings separate in the execution transport
  so nested calls resolve their own overrides from the immutable snapshot,
  without inheriting caller-specific settings or rereading changed YAML.
- Display configured target settings in verbose project help. Update the usage,
  authoring, design and AI references with configuration layout and resolution
  rules; CLI option defaults remain separately validated and explicitly overridden.
- Verification: `build.cmd verify --doinstall-tools C:/tmp/_rust/doinstall/target/release`
  passed on Windows x64: 314 .NET Release tests, 63 Rust/native tests, formatting,
  clippy, documentation, catalog and whitespace gates. Focused native YAML tests
  also passed; coverage includes cross-language calls, source-qualified shortcuts,
  qualified settings, immutable snapshots, required/null settings and native
  process/file references. Scoped .NET whitespace formatting passed. Evaluated
  .NET bin/obj/publish and Cargo outputs remain external. No active installation,
  publishing, deployment or push was performed; native Linux/macOS/ARM64 acceptance
  for this change remains pending.

### 2026-10-16 Support self-extracting doinstall shared workflows

- Add a declarative assemble-self-extracting task using separately supplied
  runtime, builder, and target launcher-template tools, without a language SDK
  or archiver. Require result-file on both assembly tasks and consume the
  builder's actual FilePath in the Rust creator instead of guessing installer.exe.
- Support producer-only packaging filename templates and launcher-template tool
  selection. Preserve omitted optional runtime lists during expansion. Record
  successful artifacts only after all build/archive steps succeed; missing results
  and later failures preserve the preceding build record. Add no compatibility
  loaders, filename fallbacks, or migrations.
- Update the local source template and output selector, AI/installer/agent
  guidance with complete recipes, staging metadata, prebuilt test-tool requirements,
  and the doinstall CI pin. Regenerate the shared catalog and synchronize installed
  task copies/tracking using shared management. Retained uninstall names stay
  internal and independent of delivery filenames.
- Verification: build.cmd verify --doinstall-tools C:/tmp/dotask-se-tools passed
  317 .NET Release and 63 Rust/native tests on Windows x64, formatting, clippy,
  documentation, catalog freshness, and whitespace gates. Tests covered SDK-free
  assembly, custom names, extraction/validation, GUI install/launch/uninstall,
  failure-safe recording, and omitted runtime lists. Both documented recipes,
  metadata help, scoped .NET formatting, and 31 local links/anchors passed.
  Evaluated Cargo and .NET outputs remain external. Tests used disposable roots
  without changing active installs or user PATH. Native Linux/macOS/ARM64 adapter
  acceptance and remote CI remain unverified; the pinned doinstall dependency
  must be published before remote CI can fetch it. No active installation,
  publication, or push was performed.

### 2026-10-16 Composable HTTP and JSON project-page workflows

- Add native `.task` HTTP GET with typed query/header references, bounded
  same-origin Link pagination, whole-action timeout/cancellation, and complete
  JSON results. Add JSON filtering/projection, explicit text templates, and
  sequential foreach bodies with lexical items/results and ordinary task calls.
- Add `file: {operation: frontmatter}` with creation-only defaults, fill-empty
  values, explicit field updates, and existing identity checks. Preserve unknown
  metadata, comments, Markdown body, encoding, line endings, final-newline state,
  and no-op timestamps through guarded atomic writes. Refuse unsupported edits.
- Add the runnable GitHub/Eleventy project-page example using plain repository
  filenames and task-owned GitHub mappings. Refresh imported fields while keeping
  summaries, screenshots, visibility, and notes editorial. Document action
  contracts, scope/limits, manual renames, and per-file failure behavior.
- Verification: `build.cmd verify --doinstall-tools C:/tmp/dotask-se-tools` passed
  on Windows x64: 317 .NET Release and 75 Rust/native tests, formatting, clippy,
  documentation, catalog, and whitespace gates. Coverage includes eight native
  HTTP/workflow integration tests and four frontmatter unit tests. A live GitHub
  refresh created 58 public non-fork pages; another refresh preserved authored
  content, bytes, and no-op mtime. Local Markdown link/anchor checks passed.
  Native Linux/macOS/ARM64 acceptance remains pending. No active installation,
  publishing, deployment, or push was performed.

### 2026-10-16 Find installed doinstall tools through the builder

- Query the selected builder's --print-tool-paths JSON when the shared creator
  needs unspecified runtime/template paths. Respect explicit options/settings,
  require absolute results, and preserve command failure diagnostics. A builder
  command shim on PATH now suffices without per-project tool-path configuration.
  Do not parse shims/receipts, scan version directories, or fall back to old tools.
- Document installation through doinstall's own self-extracting package and
  simplify the consumer recipe. Update the external CI dependency pin, shared
  catalog, and installed task/tracking using tool-managed sync. Keep installer
  implementation and the native query entirely in the independent product.
- Verification: build.cmd verify --doinstall-tools C:/tmp/doinstall-self-install-tools
  passed 319 .NET Release and 75 Rust/native tests on Windows x64, formatting,
  clippy, documentation, catalog and whitespace gates. Directory/self-extracting
  GUI workflows passed with PATH-only builder discovery and explicit overrides.
  Temporary installed-tool acceptance created and validated a consumer launcher
  without tool-path settings, then uninstalled the tools; persistent user/machine
  PATH was unchanged. Scoped .NET formatting and 35 local documentation links/
  anchors passed. Cargo and evaluated .NET outputs remain external. Native Linux
  acceptance is unverified; macOS/ARM64 tests are deferred. No active installation,
  publication or push occurred; remote CI requires publishing the new dependency.

### 2026-10-16 Deliver dotask as a self-extracting installer

- Switch dotask's source installer recipe to the existing declarative
  self-extracting assembly task. Forward the staged launcher template alongside
  the matching builder/runtime, and select the resulting `-se` launcher through
  the configured existing-installer pattern.
- Document single-file delivery and retained extraction. Keep the build and
  verification entry point `build.cmd create-dotask-installer` unchanged.
- Verification: `build.cmd create-dotask-installer` passed both full gates on
  Windows x64, including 319 .NET Release and 75 dotask Rust/native tests,
  formatting, clippy, documentation, catalog, and whitespace checks. The output
  contained only the launcher; configured selection, validation, retained
  extraction, temporary installation/launch/uninstall, and both SDK payloads
  passed. Persistent user/machine PATH was unchanged. Evaluated .NET and Cargo
  outputs remain external. Native Linux/macOS/ARM64 acceptance is unverified;
  no active installation or push occurred.

### 2026-10-18 Seed blank Markdown bodies from downloaded text

- Added native HTTP text responses and optional 404 handling with an explicit
  found flag. Other failures still stop the workflow; text and optional requests
  cannot paginate. Known legacy text can opt into Windows-1252 fallback without
  reinterpreting valid UTF-8 or inserting replacement characters.
- Added frontmatter body-empty results and body-if-empty content seeding. Any
  non-whitespace body, including comments and code, is preserved. Optional
  strip-first-h1 removes only the first parsed Markdown ATX/Setext H1; other
  Markdown remains literal. Identity/hash guards, atomic writes, encoding,
  line endings, final-newline policy, and no-op timestamps still apply.
- Updated the YAML reference and agent guide. Verification on Windows x64:
  build.cmd verify with explicitly selected prebuilt doinstall tools passed
  319 .NET Release and 81 Rust/native tests, formatting, clippy, documentation,
  catalog freshness, and whitespace checks. Native Linux/macOS/ARM64 acceptance
  remains unverified. No active installation, publication, or push occurred.

### 2026-10-20 Publish reusable Rust console-tool tasks

- Promote the product-independent build/run/test/format/Clippy/version tasks to
  shared-tasks/rust, with source-relative dependencies, explicit helper/README
  ownership and a regenerated mixed YAML/Rust catalog. Keep thin consumer wrappers
  and product-specific packaging/release policy separate; routine tasks preserve
  package versions and explicit version preparation defaults to patch.
- Document installation and updates through ordinary add/save/sync commands,
  offline execution, managed-copy preservation and canonical source maintenance.
  Verify canonical Rust entry points as part of the repository formatting gate.
- Move generic task acceptance into native dotask tests using the checkout CLI
  and SDK. Install actual catalog dependencies in disposable projects, exercise
  offline build/run/test and explicit version increments, preserve argument
  boundaries/exit codes, refuse overwriting edits during upstream updates, and
  stop on Clippy/private-content failures. Four source-helper regressions now
  run in Cargo tests instead of remaining uncompiled task-only tests.
- Verification: build.cmd verify with the checksum-verified doinstall v0.1.1
  portable tools passed on Windows x64: 319 .NET Release and 86 Rust/native
  tests, formatting, strict Clippy, documentation, catalog freshness and Git
  whitespace checks. Native test fixtures covered source-byte/version
  preservation and UTF-8/UTF-16 private scans. Cargo target/build and evaluated
  .NET output/intermediate/publish directories remain external. No active
  installation, persistent PATH or package version was changed. Native
  Linux/macOS/ARM64 task acceptance remains unverified locally.

### 2026-10-20 Select native CI shells explicitly

- Replace the existing matrix.shell expression rejected by GitHub's workflow
  validator with literal cmd/bash steps selected by runner.os. Retain the same
  native platform matrix, source preparation and verification/installer commands.
- Verification: checksum-verified Actionlint 1.7.11 and git diff --check passed.
  The preceding full Windows gate passed 319 .NET and 86 Rust/native tests;
  canonical task sources, SDKs and catalog are unchanged by this workflow fix.

### 2026-10-21 Align local tasks and build identity with the shared Rust flow

- Adopt the managed Rust group behind local build/run/test/format/Clippy/version
  entry points. Test runs the shared native gate followed by required dotask
  verification for SDKs, integration, task formatting, docs and catalog. Add an
  optional project verify hook to shared Rust test; absence alone is skipped,
  failures propagate, and wrappers can disable it to forward their own options.
- Make the CLI's committed Cargo version authoritative. Display SemVer plus
  human UTC build metadata and full UTC time, retaining source identity in the
  internal build-info response. Explicit update-version prepares patch/minor/major
  versions; ordinary builds, tests and packaging preserve version files.
- Keep bootstrap in its own external Cargo profile. Record inspected executable
  hashes, retain build flags/time during tests, inspect final test-built binaries,
  and reject changed bytes before run/staging. Preserve both SDKs and the license.
- Package the inspected snapshot using explicit prebuilt doinstall tools or
  builder discovery; preserve deliberate --source development and existing CI
  coverage. Create fresh local installer outputs under evaluated external Cargo
  output, record their actual FilePath and let local install use that record.
- Verification on Windows x64: create-dotask-installer with checksum-verified
  doinstall v0.1.1 tools passed 87 Rust and 319 .NET Release tests, strict Clippy,
  formatting, docs, catalog freshness and whitespace checks. Native acceptance
  covered temporary install, packaged Rust/C# SDK execution, license, uninstall,
  unchanged persistent PATH, run output, explicit version preview, tool-source
  conflicts and source-local output refusal. Actionlint 1.7.11 passed. Evaluated
  Cargo target/build and .NET output/intermediate/publish paths remain external.
  Keep package version 0.1.0; native Linux/macOS/ARM64 acceptance is not claimed.

### 2026-10-21 Publish native bundles from verified main pushes

- Add pinned Windows Server 2022/Linux Ubuntu 22.04 x64 release jobs, retaining
  push/PR verification and the explicit macOS source installer gate. Main pushes
  publish the committed stable SemVer after every gate; manual runs upload tested
  artifacts only. Pin Rust/.NET/actions, keep builds read-only, and let only the
  publisher upload assets to a draft and publish it with GitHub's required tag
  at the exact tested commit. Reject reused versions, incomplete drafts and
  transport failures; published same-commit reruns leave existing assets intact.
- Pin doinstall v0.1.1's published native tools archives and their reviewed
  SHA256 hashes. Verify before extraction/execution and again before assembly;
  preserve explicit source development and refuse unpinned development deliveries
  at the publication collector. Never bundle doinstall tools in dotask's payload.
- Add a Cargo-only release driver under .tasks for native portable ZIP/tar.gz
  and self-extracting deliveries with identical native/Rust SDK/.NET SDK/license
  payloads, complete dependency notices, source/toolchain provenance and SHA256SUMS.
  Preserve Linux executable modes and enforce the glibc 2.35 compatibility baseline.
  Package inspected build bytes, keep output external, and record the returned
  installer only after temporary native task/package/install/uninstall acceptance.
- Remove private .NET helper PDB paths, remap source paths, and scan expanded
  archive/installer content in ASCII and UTF-16 with closed file inventories.
  Fix clean-runner SDK prerequisite ordering and isolate standalone Unix timeout
  probes from inherited repository process groups, isolate console-control tests,
  and use literal native CI shells for quoted Windows paths. Document release
  preparation, manual artifact review and pinned filekeep consumption without
  source builds.
- Verification: full gates passed 96 Rust/319 .NET Release tests on Windows x64
  and 98 Rust/319 .NET on native Ubuntu 22.04 x64 in Docker with init, plus strict
  Clippy, formatting, docs, catalog and whitespace. Both platforms passed native
  portable and installed Rust/C#/YAML execution, downstream packaging, artifact
  recording, temporary uninstall and unchanged persistent PATH/profile checks.
  Real combined artifact collection/checksums and dirty-source publication refusal
  passed, along with Actionlint 1.7.11 and fake-publisher failure/idempotence tests.
  Version remains 0.1.2; hosted Actions/macOS acceptance is not claimed. No
  promotion, push, remote tag or release publication was performed.

### 2026-10-23 Precompiled installer creator

- Replace the source-executed doinstall/create-installer Rust task with a thin
  declarative adapter to dotask-doinstall-create-installer, built and inspected
  with dotask. Stage it through bootstrap, SDK test snapshots, source installers,
  and Windows/Linux x64 release archives/installers.
- Resolve the helper beside the actual running CLI, preserving context, nested
  calls, settings/defaults, failure codes, artifact returns and successful-build
  recording. Keep build/gather/version/collision behavior and independently
  supplied doinstall tools. Missing helpers fail without compilation/downloads.
- Document native distribution and explicit protected replacement of tracked
  Rust task copies; remove the old shared source/support files.
- Verification: Windows build.cmd test passed 98 Rust/323 .NET Release tests,
  strict Clippy, formatting, docs/catalog/whitespace; all four compiler-free
  creator cases passed again with an explicit post-assembly failure check.
  Complete Windows portable/self-extracting acceptance verified creator execution
  through an installed shim, artifact recording, temporary install/uninstall and
  unchanged persistent PATH. Debian 13 x64 passed 82 native tests, strict Clippy,
  compiler-free sample packaging/validation/install/uninstall, artifact-result
  transport and unchanged profiles; its .NET-dependent test was excluded.
  Ubuntu release/macOS acceptance was not repeated. No active installation,
  package version, remote branch, tag or published release was changed.
- Harden the cancellation fixture's readiness protocol: publish complete PID
  markers atomically and always cancel/await the child before disposal, including
  an incomplete-marker parse failure. Preserve strict fixture deletion so leaks
  remain visible. Both cases passed 20 consecutive focused runs (40 cases).
  Full Windows build.cmd test passed 98 Rust/324 .NET Release tests plus
  strict Clippy, formatting, docs, catalog and whitespace checks.

### 2026-10-23 Prepare dotask 0.1.3

- Explicitly update the native CLI package and matching lockfile entry from
  0.1.2 to 0.1.3, retaining independent SDK versions. Update current release
  asset names and the pinned-consumer example to the prepared version.
- Verification: Windows build.cmd test passed 98 Rust and 324 .NET Release
  tests with no skips, plus strict Clippy, formatting, documentation, catalog
  freshness and whitespace checks, using the existing checksum-pinned
  doinstall v0.1.1 prebuilt tools. No push, publication or active installation
  change was performed.

### 2026-10-23 Add first-class Go tasks and bundled SDK

- Recognize Go entry files and --lang go throughout discovery, metadata, help,
  completion, requirements, defaults, extension-qualified identity, ambiguity,
  nested calls and protected shared-task management. Metadata remains usable
  without Go. Shared Go support imports survive source/group installation and
  synchronization without adopting or overwriting local edits.
- Bundle the standard-library sdk/go module with source staging, portable archives
  and installers. Provide typed context/parameters/settings, portable paths,
  process capture/execution and exit-code errors, optional/nested calls, and
  installer artifact creation/return/recording/discovery/launching. Native
  installer operations preserve existing locks, hashes, unknown-state protection
  and Windows activation behavior without Rust/Cargo or .NET on the consumer.
- Compile with a stable installed Go 1.22+ compiler and local toolchain selection;
  never download toolchains or modules implicitly. Isolate consumer manifests,
  workspaces, build flags and Cgo. Keep generated modules, source/SDK snapshots,
  object caches and atomically published executables external. Cache identity
  includes source/support/SDK bytes, compiler/target metadata and build settings;
  snapshot locks protect concurrent builds and interrupted work.
- Add a runnable Go example, native consumer fixtures, cross-language/cancellation
  regressions, SDK tests, required test-go verification and pinned CI provisioning.
  Document authoring, distribution and explicit prebuilt-tool acceptance.
- Verification on Windows x64 with installed Go 1.26.6 and existing checksum-pinned
  doinstall v0.1.1 tools: build.cmd test --doinstall-tools PATH passed 106 Rust
  cases (including the explicitly invoked installer fixture), 328 .NET Release
  tests and Go SDK tests, strict Clippy, formatting, docs, catalog and whitespace.
  All 49 focused integration cases passed; Actionlint 1.7.11 and the documented
  Unicode greeting passed. Evaluated Cargo and .NET outputs remain external.
- Dedicated restricted-PATH acceptance exercised Go orchestration, nested Go
  staging, the precompiled creator, real self-extracting assembly, artifact
  return/record/find/run and preservation of the previous record after a final
  packaging failure. Rust/Cargo/.NET traps were never invoked; consumer manifests
  and source stayed unchanged. Real local portable and self-extracting dotask
  deliveries passed packaged Go/Rust/C#/YAML execution, downstream creation,
  temporary install/uninstall, inventory/privacy scans and persistent PATH checks.
  Version remains 0.1.3. Native Linux/macOS acceptance is unverified locally;
  gostatic, active installations and remote publication were not changed.

### 2026-10-27 Shared .NET version preparation

- Added canonical dotnet/update-version.cs with patch by default, explicit patch,
  minor/major and dry-run options, configurable project path, original encoding,
  BOM, whitespace and comments preserved. Conditional/inherited/ambiguous versions,
  conflicting increments and assembly-version component overflow are rejected.
  The task performs no build, restore or Git operations and does not alter NuGet
  dependency lockfiles. Documented consumer wrappers and regenerated the catalog.
- Added executable shared-task regression cases for patch/minor/major, dry-run,
  UTF-8/UTF-16 BOM/CRLF preservation, lockfile preservation and rejected definitions.
- Verification: build.cmd test with prebuilt DoInstall tools passed the complete
  Windows native/Go gates, all 338 .NET tests, formatting, docs, catalog and
  whitespace checks. No application version was changed or published.

### 2026-10-27 Utah calendar versions for .NET projects

- Changed the shared .NET version task to major.minor.yyMM.ddHH in America/Denver
  with 24-hour time and DST, independent of the execution machine's timezone.
  Default/explicit refresh retains major/minor; optional minor/major increments
  keep the calendar suffix. Three-part numeric versions migrate on explicit use;
  same-hour refreshes are no-ops, including the repeated autumn DST hour.
- Kept encoding/BOM/EOL preservation, validation and no-build/no-Git boundaries.
  Added deterministic Utah DST, leap-day/year-boundary and increment regressions,
  and executable calendar-migration and repeated-refresh coverage.
- Verification: all 22 focused calendar/shared-task cases and the complete
  build.cmd test gate passed, including 350 .NET tests, native/Go acceptance,
  formatting, documentation, catalog and whitespace on Windows x64.

### 2026-10-27 Prepare dotask 0.1.4 publication

- Explicitly advanced dotask-cli from 0.1.3 to 0.1.4 using the version task,
  updating the manifest and corresponding Cargo lock entry together. Includes
  the pending Go/installer/release workflow and shared Utah .NET version task.
- Verified availability and checksum of the pinned published DoInstall v0.1.1
  Windows tools before release verification and main promotion.
- Verification: the complete build.cmd test gate passed for 0.1.4, including
  all 350 .NET tests, native/Go installer acceptance, documentation, whitespace
  and shared catalog freshness checks on Windows x64.

### 2026-10-27 Fix Unix release packaging lint

- Compile the GLIBC symbol regex once before inspecting the native executables.
  This preserves the Ubuntu compatibility check and fixes the Linux-only Clippy
  failure found by the first 0.1.4 release run; no lint or release gate is bypassed.
- Verification: formatting, strict Clippy and all nine release-helper tests
  passed on Windows; strict Clippy and seven unit tests passed on WSL Linux.
  The WSL publisher shell acceptance requires jq, absent from that environment;
  the unchanged publisher acceptance passed on Windows and hosted Ubuntu.

### 2026-10-27 Harden canceled nested-target request cleanup

- Allow a bounded Windows grace period when deleting owned request directories
  after child-process termination. Waiting for a killed process-tree root does
  not guarantee that descendant DLL handles have closed; transient sharing errors
  no longer replace successful cancellation cleanup with access-denied errors.
- Keep persistent cleanup failures visible and retain cancellation/child-stop
  assertions. Add deterministic transient-lock and persistent-lock regressions.
- Advance only the macOS DoInstall source pin to the reviewed command-link fix
  on its approved develop history. Keep macOS required for collection/publication
  and leave checksum-pinned published Windows/Linux tools unchanged.
- Include the follow-up macOS PTY ABI test fix in source pin 9dd2e27. Native
  Linux PTY/Clippy, full Windows DoInstall verification and all-target Apple ARM64
  compile/strict Clippy passed; native macOS runtime acceptance stays required.
- Verification: the complete build.cmd test gate passed on Windows x64 with
  all 352 .NET tests, native/Rust/Go and pinned-tool installer acceptance,
  publisher simulation, strict lint/format, documentation, catalog and whitespace.
  Evaluated .NET bin/obj/publish paths remain under C:\tmp\_dotnet.

### 2026-10-27 Support preview calendar version preparation

- Accept three-/four-part numeric Version with -preview.N. Preserve preview by
  default, add --increment-preview (starts at 1), and clear preview on minor/major
  increments. Refuse conflicting flags, invalid suffixes and counter overflow.
- Preserve AssemblyVersion, update existing FileVersion numerically, and update
  existing InformationalVersion while retaining custom suffixes or $(Version)
  expressions. Leave absent optional properties to SDK defaults; document explicit
  AssemblyVersion/FileVersion for stable binding and independent file numbering.
- Validate all edited properties before writing. Preserve comments (including
  commented-out version tags), whitespace, EOLs and UTF-8/UTF-16 BOMs without XML
  serialization. Refresh the shared catalogue and authoritative task documentation.
- Verification: all 46 focused calendar/version tests and the complete Windows x64
  build.cmd test gate passed, including all 376 .NET tests (zero failures/skips),
  native/Rust/Go and pinned-tool installer acceptance, publisher simulation,
  strict lint/format, documentation, catalogue freshness and Git whitespace.
  Evaluated .NET and Cargo outputs remain external under C:\tmp.

### 2026-10-27 Share guarded .NET tests and explicit formatting scope

- Move test process-tree safeguards into shared support: filtering, project/
  solution override, no-build, configurable timeout/memory cap and single-run
  identity, fresh build workers, exit propagation and descendant cleanup.
  Consumers supply their limits through YAML; no PTS paths/names are hard-coded.
- Enforce Windows aggregate Job Object caps and Linux cgroup/systemd limits;
  refuse unsupported configured caps rather than silently dropping protection.
- Allow format.solutions to select explicit workspaces and normalize-text=false
  to protect vendored dependencies from broad optional normalization.
- Add real process/lock/memory and shared-entry forwarding regressions plus
  multi-solution formatting coverage; update catalogue and authoring guidance.
  Verification: eight focused cases and the complete Windows x64 build.cmd test
  gate passed, including all 383 .NET tests (zero failures/skips), native/Rust/Go,
  pinned-tool installer acceptance, publisher simulation, strict lint/format,
  documentation, catalogue freshness and Git whitespace. Linux/macOS runtime
  acceptance was not performed. Evaluated outputs remain external.

### 2026-10-27 Make native path fixtures reflect macOS filesystem rules

- Resolve the system temporary parent before no-links file-action tests, keeping
  fixture-created links subject to the unchanged runtime refusal checks.
- Always test Unicode canonical-name preservation through a command alias, and
  retain invalid-byte preservation on supporting filesystems. Accept only Apple's
  specific EILSEQ rejection when an invalid UTF-8 fixture name cannot be created;
  other fixture errors still fail. No runtime guard or macOS gate is relaxed.
- Verification: complete Windows build.cmd test passed with all 352 .NET tests,
  native/Rust/Go and pinned-tool installer acceptance, publisher simulation,
  lint/format, documentation, catalog freshness and whitespace. Native Linux
  fixture/declarative/HTTP checks, strict Clippy and linked-temp simulation passed.

### 2026-10-27 Limit release CI to Windows and Linux

- Remove the macOS job and its collection/publication dependency, as requested.
  Keep both Windows/Linux verification, native packaging and temporary installer
  acceptance gates, pinned tools, checksums and exact-source publication intact.
- Add a workflow regression requiring only the Windows/Linux native matrix and
  no macOS job; update current release, installation and authoring guidance.
  Preview-version work on develop remains excluded from 0.1.4.
- Verification: all nine release-driver/publisher tests, Rust formatting,
  build.cmd verify-docs and whitespace checks passed. Application runtime and
  pinned Windows/Linux tool inputs are unchanged from the passing native gates.

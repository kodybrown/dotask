# Verification

## Project initialization (2026-09-18)

`dotask --init` creates missing project configuration and a task directory in the
invocation directory without upward discovery, SDK calls, or downloads. Existing
configuration, task contents, and lock files are preserved. Custom task
directories stay within the new project root. Initialization preflights legacy
or invalid configuration, path-type conflicts, and symlink destinations, and
publishes configuration without replacing an existing destination.

On Linux with SDK 10.0.401, the full `./build.sh` gate passed **250 tests**, solution
and task formatting, documentation/Git whitespace, catalog freshness, and shim
hashes. The 24 initialization cases cover fresh/repeated setup, YAML-safe names,
parent-project isolation, existing user files, custom/invalid paths, conflicts,
symlinks, concurrent initializers, cancellation, read-only help/completion, and
an unusable SDK host with unavailable shared-task sources.

The published CLI also initialized and listed a temporary project with spaces
in its path, preserved the configuration on a rerun, and provided initialization
help and Bash completion. All 86 local documentation links/anchors resolved;
evaluated build outputs remain under the user's `/tmp/_dotnet` policy. No active
installation or shell profile was modified. Native Windows/macOS acceptance
remains pending the CI matrix; link-creation tests do not run on Windows where
they may require additional privileges.

## Single-checkout workflow and formatting (2026-09-18)

The complete `./build.sh` gate now passes on Linux with SDK 10.0.401: **226 tests**,
solution/task formatting, documentation/Git whitespace, catalog freshness, and
bundled shim hashes. This resolves the formatting failure recorded below by
applying the current `.editorconfig` throughout the solution and task sources.
Help-setting character escaping was moved into an equivalent local function to
avoid a switch-expression/lambda layout that alternated between formatter passes.
The final formatter check reported zero changed files.

All 13 shared sources are byte-identical to their project task copies, with a
regenerated catalog. Rebuilding the shims reproduced the original binary hashes;
only the formatted build-task hash changed in the shim record. Both binaries are
included as delivery assets. The 82 local documentation links/anchors resolve,
and evaluated build outputs remain under the user's `/tmp/_dotnet` policy.
Native Windows/macOS acceptance remains pending the CI matrix.

Agent instructions now use the primary checkout directly for single-agent work;
additional worktrees require an explicit user request.

## Catalog generation and Git task delegation (2026-09-18)

Catalog generation now lives entirely in `.tasks/catalog.cs`, with the CLI
metadata parser still included as its shared parsing implementation. The former
single-use `.tasks/_support/TaskCatalog.cs` was removed. Tests invoke the actual
catalog task and cover deterministic output, exact-byte hashes, dependency and
path validation, legacy sidecar diagnostics, and read-only freshness checks.
`verify-docs` declares and explicitly calls `git/check` with `Whitespace = true`;
plain `git/check` continues to check only tool availability.

On Linux with SDK 10.0.401, all **226 tests** passed through `./build.sh`, including
20 focused repository-task cases. Coverage includes missing Git-task dependencies,
child exit-code propagation, staged and unstaged whitespace errors, unchanged
files/index entries, and plain Git checks outside repositories. Documentation/Git
checks, catalog freshness, and shim hashes passed separately using the bootstrapped
CLI. All 82 local documentation links/anchors resolved.

The full gate stopped at formatting: the expanded `.editorconfig` from commit
`0b543dd` exposes formatting diagnostics throughout existing solution files.
Changed C# files were formatted and checked separately; this is not a fully
passing source gate. No repository-wide formatting was applied.

The fresh worktree also exposed two missing delivery assets: the initial commit
omitted the bundled Windows shim executables because a global Git ignore excludes
`*.exe`. Rebuilding them with `./build.sh shim` reproduced both recorded SHA-256
hashes exactly. Both assets are included with this change, with narrow repository
ignore exceptions; shim sources and recorded hashes are unchanged. Native
Windows/macOS acceptance remains pending the CI matrix. Build outputs remain
under the user-configured `/tmp/_dotnet` location.

## Task layout and shared standards (2026-09-18)

`dotnet/install.cs` now contains its .NET publishing/query methods; the two
`DotNetInstall.cs` helper files and their include/dependency declarations were
removed. The bootstrap MSBuild hook moved to `.tasks/misc/bootstrap.targets`,
with CLI imports and launcher fixtures updated. Discovery remains an explicit
code-extension allowlist; non-code files do not become targets.

The shared format/install/pack sources are canonical and byte-identical to their
project copies under `.tasks/dotnet/`. The older root formatter was removed.
Formatting covers the solution and the selected tasks root (including custom
directories and other groups); `BuildContext.TaskDirectory` exposes that root.
The new shared pack task builds the configured project into local NuGet packages,
with configurable output directory and CLI executable.

The full `./build.sh` gate passed **222 tests** on Linux with SDK 10.0.401, plus
solution/task formatting, documentation/Git whitespace, the 13-task catalog, and
bundled shim hashes. Focused integration checks exercised the relocated bootstrap
with custom publish paths and rebuild/failure cleanup, one-file installation,
real NuGet packaging, and formatting verification/write modes with optional-target
failure propagation. The prior `IDE0011` failures recorded below were resolved
by applying the repository's standard brace/whitespace formatting. Native shim
sources and binaries were unchanged. Build output still follows the user policy
under `/tmp/_dotnet`, and 82 local documentation links/anchors resolved.
Native Windows/macOS execution remains pending the CI matrix.

## Shared-task dependencies in XML (2026-09-18)

Task authors now declare companions with `<requires task="group/task" />` and
`<requires file="group/_support/Helper.cs" />` in entry-point XML documentation.
The official catalog generator, private-task indexing, and help use the same
metadata parser. The two official `.task.json` sidecars were removed and the
catalog regenerated. Old private sidecars produce migration instructions without
altering the originals. Distribution metadata never automatically executes tasks.

On Linux with SDK 10.0.401, the final `./build.sh` run passed **219 tests**. Coverage
includes XML class/Main selection, helper hashes/ownership, dependency copying,
help display, explicit execution, invalid metadata/paths, sidecar migration
diagnostics, and qualified task names. Verification fixtures were updated to use
the source-qualified paths and `dotnet/format` path used by the current tasks.

The full gate then stopped at existing `IDE0011` missing-brace diagnostics in six
installation source/test files: the repository verification task was changed
during this work to call the broader `dotnet/format` target. Those files and that
task change were preserved. This is not a fully passing source gate. Catalog,
shim hashes, documentation/Git whitespace, and the whitespace-only format check
were checked separately. Build/install help displayed the XML companions, and
all 32 local links/anchors in the changed reference guides resolved. Native
Windows/macOS execution remains pending the CI matrix.

## Temporary installation locks (2026-09-18)

Installation locks now use `FileOptions.DeleteOnClose`. Success, handled failure
and cancellation remove the app-root and command-directory lock files as their
exclusive handles close. An older retained file is reused and removed on the
next installation; a failed contender does not delete the active owner's file.

The full `./build.sh` gate passed **208 tests** on Linux with .NET SDK 10.0.401,
plus formatting, documentation/Git whitespace, catalog and shim-hash checks.
Focused tests exercise independent-process contention, repeated handoff, normal
cleanup, cancellation, and reacquisition after forcibly terminating the owner.
Native Windows/macOS execution of these cases remains pending the CI matrix.

## Versioned current-user installation (2026-09-18)

On Linux x64 with .NET SDK 10.0.401, `./build.sh` passed **205 tests**, solution
whitespace, required documentation/Git whitespace, the 12-task shared catalog,
and bundled shim source/build-recipe/binary hashes. All official shared tasks,
including `dotnet/install` and its support source, compile in the suite.

Installation coverage includes fresh/repeated installs, same-version changed
builds, resource and permission fingerprints, immutable old builds, command
ownership, unowned destination conflicts, interrupted activation/recovery,
modified recovery state, cancellation, competing installers, unsafe identities
and paths, missing entry points, command aliases, and directory overlap checks.
The real shared-task integration test publishes a console fixture with a custom
`PublishDir`, then checks arguments, Unicode/spaced paths, current directory,
environment, stdin/stdout/stderr and exit status through the installed command.
It verifies repeat installation, a resource-only update and preservation of the
active application after a compilation error.

The real dotask CLI was also installed and updated in explicit temporary app/bin
directories. Its installed `--version` and basic greeting example passed. The
source build output policy remains under `/tmp/_dotnet`; no bin/obj/publish
override was introduced. Testing did not change the real global-tool installation,
shell profiles, user PATH, or existing shortcuts.

Both Windows shim architectures were cross-compiled with LLVM 22.1.8 and their
PE headers/resources were checked. They import only Kernel32 and need no managed
runtime. This is **not native Windows acceptance**. Windows-specific integration
branches cover the shipped shim's sidecars, argument and stream forwarding,
full-width exit codes and isolated console Ctrl+C; those branches do not run on
Linux. Native Windows x64 and macOS execution remain pending the CI matrix;
Windows ARM64 still needs its own native acceptance run. Linux-musl selection is
implemented but was not exercised on this glibc host.

See [installation](INSTALLATION.md) for the supported API, defaults and deferred
rollback/pruning/uninstall commands. The older NuGet-tool checks below remain
historical evidence for the optional tool-package route.

## Build-to-install documentation (2026-09-18)

The README now puts current-user installation immediately after the build step,
with platform-specific PATH instructions, an installed example, and explicit
same-version uninstall/reinstall steps. An isolated `--tool-path` smoke test on
Linux with .NET SDK 10.0.401 verified `--source ./artifacts/packages`, version
`0.1.0`, and `--no-http-cache`. Both installation and reinstallation produced
package bytes identical to the freshly built package. The installed command's
version and the documented greeting example passed.

The documentation gate and all 78 local Markdown links/anchors passed. No real
user-wide installation, shell-profile edit, or `dt` shortcut change was made.
This was a documentation update; the full source test suite was not rerun.

## Bootstrap and repository tasks (2026-09-18)

The current source gate is `./build.sh` on Linux/macOS or `.\build.cmd` on
Windows. It compiles this checkout, stages a temporary runner from the evaluated
publish output, and runs the required repository `verify` task in Release.
The Makefile, Make modules, and Python catalog generator have been removed.

On Linux with .NET SDK 10.0.401, the launcher passed the complete **179-test**
suite, solution whitespace checks, required documentation/Git whitespace checks,
and shared catalog verification. The 11 official tasks still compile in the suite.
New coverage includes required verification stages, option forwarding and early
failure, catalog generation/drift checks and invalid manifests, and real launcher
execution with a custom publish directory and paths containing spaces.
Launcher tests check default arguments, empty/spaced arguments, exit-code
preservation, staging cleanup, and refusal to run stale outputs after a failed build.
They also clean and republish the original outputs while the staged runner is alive.

A fresh source copy in a path containing spaces passed the same complete gate,
invoked from its parent directory. Its PATH retained the SDK and Git while
failure shims blocked installed `dotask`, `dt`, Make, and Python commands.
This proves the bootstrap does not depend on any of those tools. Separate Linux
probes verified missing-SDK diagnostics, publish-failure exit codes, and cleanup.
Repository output paths still follow the optional user-level MSBuild policy.

`./build.sh pack` produced the CLI NuGet package; its MIT metadata and included
license text were checked. The C# catalog generator's output matches the former
generator byte-for-byte for the current catalog. Local Markdown links/anchors
were also checked separately from the documentation task.

GitHub Actions now invokes these same launchers for verification and packaging
on Linux, Windows, and macOS. Native Windows batch execution and macOS acceptance
remain pending; this Linux run is not evidence that those CI jobs have passed.
No commit, push, public package publication, or global installation was performed.

## Shared-task distribution preview (2026-09-17)

Before the bootstrap migration, the Linux source verification passed with
**164 tests**, zero build warnings/errors, whitespace formatting, required docs,
and a fresh generated-catalog check. Coverage includes root configuration and
legacy migration, recursive/source-qualified names, selective HTTP downloads,
SHA-256 rejection, private originals, file/support ownership, required companions,
local edits and deletions, missing/corrupt tracking, dry runs, concurrent adds,
manual merge revision checks, rollback, and interrupted-operation recovery.
All 11 official starter tasks were compiled from their catalog sources.

A fresh-copy execution test removed the local cache and private originals, then
compiled/executed a source-qualified nested task call from the copied project.
Normal help/completion remain local. The built CLI's 30-target project-help smoke
check took 152–173 ms across five invocations (166 ms median) on this Linux host;
these observations are not cross-platform latency guarantees.

The installed `dt` preview was refreshed from the verified package. Smoke checks
covered project/CLI help, the repository's prerequisite orchestrator, local-catalog
listing, brace selections, required dependency installation, sync dry run, safe
removal, refusal to remove edited tasks, and execution after shared caches and
private originals were removed. The basic example's greeting and nested write
tasks also passed after migrating its configuration to the project root.

The online catalog and source files are prepared locally under `shared-tasks/`.
They have not been published to GitHub by this work. HTTP behavior is verified
with a controlled handler; live online-cache acceptance awaits publication.
Third-party sources, private GitHub authentication, other languages, and CLI
self-update remain deferred. Windows/macOS acceptance for these new features is
still pending; no remote CI run or release publication is claimed.

The first preview is developed on Linux with .NET SDK 10.0.401. Local checks
exercise real SDK compilation and child processes, not just parser mocks.

An earlier source verification passed: **137 tests**, a build with zero warnings
and errors, whitespace formatting, and documentation checks. A local .NET tool
package has also been installed into an isolated temporary tool directory and
used to execute the example with nested calls.

The automated suite covers discovery and overrides; XML metadata; YAML types,
duplicates, project identity, and defaults; case-insensitive binding; alias and make-style arguments;
project summaries and CLI-only help; help and completion without compilation;
paths with spaces; real package/project/include
references; compiler errors; nested execution and cycles; process argument
round-trips; cancellation; and filesystem operations.

Grouped filenames (`dotnet run.cs`) are covered by full/short-name resolution,
exact-name precedence, ambiguous aliases, duplicate full names, reserved commands,
help and completion, and real SDK compilation/execution. Both invocation forms
share full-name YAML defaults and runtime identity. Nested calls and cycle checks
also resolve aliases through the same catalog. Both the target list and detailed
help use short names alone when available; detailed help identifies the actual
task file on a `Source:` line beneath the description.

The refreshed packaged preview was also checked on Linux: short-only target
listing, full/short detailed help and completion, and execution of the renamed
repository `dotnet build.cs` through `dotask build` passed. The complete
`dotnet run.cs` example from the authoring guide was copied byte-for-byte into
two temporary projects with different application paths in YAML. Both ran from
subdirectories, one through `run` and one through `dotnet-run`, using the same
full-name defaults. Seven fresh packaged summary invocations after this change
had a 164 ms median (156–206 ms range); no targets were compiled by those summaries.

The reusable `dotnet check.cs` checks that `settings.solution` points to an existing
file and queries SDK, MSBuild, and formatter versions. An integration test embeds
the actual task file and runs it beside an invalid project and marker-writing
test/format targets: it succeeds without evaluating the project or invoking those
targets. `dt check` also passed in this
checkout. Temporary Linux fixtures with a controlled executable verified version
query arguments, failure diagnostics, and empty-version handling. These checks
do not validate NuGet dependencies.

Optional target calls have nine additional integration tests covering current
catalog lookup, full names and aliases, ambiguity, missing targets, parameter and
configuration forwarding, original child exit codes, pre-execution errors,
cycles, cancellation, and temporary-file cleanup. `TargetExistsAsync` finds
targets without compiling or running them, including targets with invalid
metadata or C# bodies. `ExecTargetIfExistsAsync` distinguishes `NotFound` from
`Failed`, including a child that exits with code 1.

The tests embed the actual `dotnet verify.cs` source and verify check/test/format
ordering, configuration forwarding, formatting in verification mode, skipping
missing targets, stopping on failures, and failing when no checks are available.
The refreshed installed preview's `dt verify -c Release` ran the SDK/tool checks
and all 125 tests in this checkout, then preserved exit code 2 from the formatting
target. That target also checks `.tasks`, where existing user edits have
brace-layout whitespace errors. Those edits were left intact; this local
orchestration run is not a clean formatting acceptance.
The installed preview also passed a temporary-project smoke check of the exact
optional-target example from `docs/TARGETS.md`, both with and without a test
target. The reusable verification target succeeded with just that test target
present and reported the other two checks as skipped.

Run the complete gate with `./build.sh` (Windows: `.\build.cmd`), as described in
the repository README. The GitHub Actions matrix schedules the same
tests on Windows, Linux, and macOS once this repository is hosted and CI runs.

## Acceptance boundaries

- Linux checks are local evidence only. Windows and macOS execution still need
  their CI/manual acceptance; adding a workflow is not a completed remote run.
- Completion was exercised on Linux using Bash 5.3, Zsh 5.9.2, Fish 4.9.2, and
  PowerShell 7.6.6. Checks included target names, separated/equals option values,
  directory overrides, and paths containing spaces. Bash has an automated adapter
  regression test; other shell checks were manual/native completion probes.
  These shell checks do not establish Windows or macOS runtime acceptance.
- The package is a local preview; no public feed publication, global installation,
  repository commit, or push is implied by verification.
- Services remain deferred.

## Help without compilation

Normal listing, `help`, and selected-target help read metadata and YAML without
invoking the SDK or creating a compilation cache. CLI-only `--help`/`-h` returns
usage before discovery or project reads; tests cover invalid YAML, a missing
explicit task directory, and invocation outside a project. Summary tests cover
optional identity, fallback names, shared settings, target options, and directory
overrides, with no CLI usage mixed in. Regression
tests cover an unusable SDK host, a nonexistent package, compiler errors in target
bodies, skipped execution requirements/required arguments, effective YAML defaults,
and metadata/default diagnostics. Execution still reports real compiler errors.

The current help layout passed all 34 focused help/layout/shell tests and the full
137-test source gate. Regression checks cover merging compatible options despite
different descriptions/defaults, keeping incompatible contracts separate,
showing only shared inline defaults, settings order/quoting, and the detailed-help
hint. Summary target lists and default breakdowns are omitted; individual target
help retains its exact description and default. The summary and target-help usage
examples matched actual output exactly, and all 58 local Markdown links/anchors
passed.

Wrapping tests cover 60/96/120 columns, narrow/unknown widths, aligned continuation
lines, long labels/tokens, intact surrogate pairs, and unwrapped completion data.
Linux pseudo-terminal checks against the refreshed installed preview verified that
the project summary, target help, and CLI help fit 96 and 60 columns. At 59 columns,
the summary matched redirected output exactly without wrapping. The checks also
confirmed that `tasks` appears last and settings paths without spaces are unquoted.

The sectioned target-help layout's earlier smoke checks covered full/short help
forms, invocation from a subdirectory, custom task
directories, omitted empty sections, remarks, and capabilities. `Source:` retained
the real filename and project-relative path beneath the target description,
aligned with that description even for long target names. Its tighter spacing
and single-quoted paths containing spaces matched the updated documentation
example exactly; paths without spaces stayed unquoted, including earlier checks
with custom task directories.
Both `dt build --help` and equivalent full-name help were exercised.

The refreshed local tool package passed the greeting and nested-call examples.
Measured after the project-summary/CLI-help split on Linux with .NET SDK 10.0.401,
invoking the packaged tool directly in seven fresh processes per case, with
stdout/stderr captured:

| Invocation                                                         | First measured run | Median | Range      |
| ------------------------------------------------------------------ | ------------------ | ------ | ---------- |
| `dotask`: repository's five targets                                | 160 ms             | 168 ms | 158–174 ms |
| `dotask`: new temporary project with thirty representative targets | 171 ms             | 167 ms | 164–179 ms |
| `dotask --help`: CLI usage only                                    | 19 ms              | 18 ms  | 18–20 ms   |

The thirty files were copies of the repository's target sources with distinct
filenames and the same shared settings. There is no metadata cache and no target
compilation in any case. Fresh processes do not imply a cold OS filesystem
cache. These are local observations, not cross-platform latency guarantees or
timing assertions in the test suite. The previous five-target help took about
3.3 seconds on repeated runs because it compiled each target sequentially.

Measure project summaries with an installed `dotask`/`dotask help` or
`dotnet /path/to/dotask.dll`. CLI-only `dotask --help` skips project reads entirely.
`dotnet run --project ...` adds SDK/project-launch overhead unrelated to help.

## Documentation verification (2026-09-17)

The usage and AI-assistant documentation was checked against CLI, metadata,
configuration, and library implementation. On Linux with .NET SDK 10.0.401:

- `dotnet build dotask.slnx -c Release --nologo` passed with zero warnings/errors.
- The former documentation gate, Git whitespace checks, and an additional local
  Markdown link/heading-anchor check passed. Its replacement, `verify-docs`, checks
  required documents and Git whitespace; it does not validate links or execute examples.
- The README/basic-example commands passed: listing, detailed help, defaults,
  aliases, case-insensitive choices, values with spaces, literal option-like
  values, rejected choices (exit 1), nested calls, and expected output-file text.
- The complete C# target and YAML were extracted from `docs/TARGETS.md` and run
  with a temporary console application. Both root invocation and upward discovery
  from a subdirectory passed in a project path containing spaces.
- The documented local-only NuGet configuration and tool install, uninstall,
  and reinstall commands passed in an isolated temporary directory. No global
  installation or shell-profile changes were made.
- All four shell scripts were emitted successfully; the documented Bash loader
  was exercised and returned `Debug`/`Release` completion candidates. This pass
  did not repeat interactive Zsh/Fish/PowerShell or other-OS acceptance.

These were documentation smoke checks. No runtime source changed, and the full
68-test suite was not rerun for this documentation update. Evaluated repository
build/intermediate/publish paths remained outside the source checkout.

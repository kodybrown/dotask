# dotask Agent Notes

## Project Identity

- Product and command: `dotask`
- C# namespace: `DoTask` (including `DoTask.Cli` and `DoTask.Runtime`)
- Project type: .NET 10 CLI and target-authoring library
- Integration checkout: the primary checkout on `develop`, located with `git worktree list --porcelain`
- Integration branch: `develop`
- Worktree mode: work directly in the integration checkout (single agent)
- Root Markdown policy: canonical-restricted

Resolve the integration checkout from the current repository's Git metadata
before using it. Do not create additional worktrees unless explicitly requested.

<!-- powercode-agent-modules:start -->
## Shared Agent Modules

Read these modules once per task, in order. This root takes precedence.

1. `AGENTS.Workflow.md`
2. `AGENTS.CodeStyle.md`
3. `AGENTS.Documentation.md`
4. `AGENTS.BuildArtifacts.md`
5. `AGENTS.DotNetVerification.md`
<!-- powercode-agent-modules:end -->

## Workflow

Work directly in the integration checkout on `develop`; only one agent works on
dotask at a time. Preserve unrelated changes and leave implementation uncommitted
until requested. Commit, integration, cleanup, publishing, installation, and
pushing are separate gates.
Preserve the user-owned notes in the parent directory. Work as a single agent.
Use ordinary filesystem tools, apply_patch, Git, and .NET tools as appropriate.

## Usage Documentation For Agents

Before creating or changing consumer tasks, read `docs/AI-ASSISTANTS.md` and the
relevant sections of `docs/TARGETS.md`. `docs/USAGE.md` describes actual CLI
behavior, including metadata-only help and its exit-code limitations.
Use `examples/basic/README.md` for runnable examples and expected output.

When changing CLI or library behavior, update its authoritative reference and
check the AI guide for stale examples or assumptions. Do not invent undocumented
helpers, lifecycle hooks, or commands. Distinguish runtime implementation from
the platform acceptance recorded in `docs/VERIFICATION.md`.

## Design Contracts

- Discovery walks upward for root `.dotasks.yaml` or `.tasks`; `--use-dir` is an
  exact override. Root configuration anchors projects even without task files.
- `--init` initializes the invocation directory without upward discovery, SDK
  calls, or downloads. Preserve existing configuration/tasks/tracking; refuse
  legacy/invalid configuration, path-type conflicts, and symlink destinations.
  Custom task directories must stay beneath that root; the override is not saved.
- One C# file per target; XML documentation owns target metadata. No target
  registration in YAML is required.
- Discovery only accepts supported code extensions (currently `.cs`); documents,
  YAML, and MSBuild `.targets` files are not tasks. The bootstrap copy hook lives
  in `.tasks/misc/bootstrap.targets`.
- Author reusable task changes in `shared-tasks/`. Keep the format/install/pack
  copies under `.tasks/dotnet/` and `.tasks/git/check.cs` identical to their canonical sources.
- Recursive task paths define full names; shared copies include source/group/task.
  Exact names precede unique shortcuts; old space-grouped filenames remain supported.
  Use source-qualified paths for nested shared-task calls, YAML defaults, and identity.
- Shared management copies files into projects and tracks original SHA-256 hashes in
  `.dotasks-lock.yaml`. Never overwrite/adopt untracked files or remove local edits.
  Private task originals are user data. See `docs/SHARED-TASKS.md`.
- Share metadata between help, validation, and shell completion. Help reports
  metadata/configuration errors; compilation errors are reported on execution.
- Help and completion never compile, restore packages, or execute target code.
- Bare `dotask` and `dotask help` show project identity, settings, and targets;
  project/target help reads YAML and checks effective defaults.
- CLI-only `--help`/`-h` skips project discovery and reads; target completion skips project configuration; management completion stays local.
- Keep YAML declarative and keep services deferred.
- Application installation takes published files through `InstallationDefinition`;
  .NET publishing stays inside the shared `dotnet/install.cs` task. See `docs/INSTALLATION.md`.
  Preserve installation ownership, immutable builds, and activation recovery journals.
  Test with explicit temporary install/bin roots; do not modify the user's active install.
- Use an ambient `BuildContext.Current`, ordinary composition, and no DI container.
- Target existence and optional execution use the CLI catalog. Only an absent
  target is `NotFound`; ambiguity and target failures must not be silently skipped.
- Preserve arbitrary process arguments. Normalize only explicitly portable paths.
- Keep `Program.cs` thin and failure/cancellation behavior explicit.

## Build And Verification

- Source changes: `./build.sh` on Linux/macOS; `build.cmd` on Windows.
  Both bootstrap the current checkout and run the required `verify` target in Release.
- Documentation-only changes: `./build.sh verify-docs` (Windows: `build.cmd verify-docs`); check relative links/anchors
  and run changed command or complete target examples where practical.
- Focused tests may use `dotnet test dotask.slnx --filter ...` directly.
- The launchers accept dotask arguments, resolve their own checkout, and preserve
  failures. No installed dotask, Make, or Python is required. CI uses the same launchers.
- Keep bootstrap logic limited to publishing and staging a temporary runner from
  the evaluated `PublishDir`. All build/test/format/docs/catalog/package operations
  belong in `.tasks`. Never run repository rebuilds from the live build output.
- `verify` requires every stage. The reusable `dotnet/verify` task's optional
  checks are not the repository verification gate.
- Regenerate `shared-tasks/catalog.json` with `./build.sh catalog` after editing
  shared sources/support files, after final formatting and before committing the
  sources and index together. `./build.sh catalog --verify` checks without rewriting it.
- `verify-docs` declares and executes `git/check` with `Whitespace = true` after
  checking required documents; plain `git/check` only checks tool availability.
- All changes: `git diff --check`.
- The required gate also checks bundled shim hashes. Changes to `src/Dotask.Shim`
  require `./build.sh shim` with LLVM, review of both native delivery assets and
  hashes, and native Windows acceptance. Ordinary builds/installs use bundled assets.
- Verify evaluated output paths after build-tooling changes.
- Repository builds import the optional user-level MSBuild output policy.
- Runtime target compilation is deliberately isolated from a consumer's build
  properties/targets and uses its own per-user external cache. Do not confuse
  target compilation with the repository's own output policy.
- Real cross-platform acceptance requires Windows, Linux, and macOS runs; local
  Linux tests alone are not evidence of the other operating systems working.

## Documentation And Canonical Sources

`docs/README.md` is the index; `docs/USAGE.md` owns the CLI usage contract,
`docs/TARGETS.md` owns the authoring contract, and `docs/DESIGN.md` owns design
decisions. `docs/AI-ASSISTANTS.md` gives consumer-task workflows and reusable agent
instructions. `docs/CHANGELOG.md` records approved
completed changes when committing. Shared agent modules are included copies from
PowerCode; the build-artifact and .NET verification modules have been adapted for
this repository's bootstrap workflow. No separate checkout is required to use
this repository. Other project files are project-owned.

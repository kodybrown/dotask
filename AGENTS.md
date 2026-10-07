# dotask Agent Notes

These instructions apply to every task in this repository.

## Project Identity

- Product and command: `dotask`
- C# namespace: `DoTask` (including `DoTask.Cli` and `DoTask.Runtime`)
- Project type: Rust CLI and task SDK, .NET 10 reference CLI and C# support/library
- Integration checkout: the primary checkout on `develop`, located with `git worktree list --porcelain`
- Integration branch: `develop`
- Task worktree root: `../worktrees/ai` relative to the integration checkout
- Task branch prefix: `codex/`
- Worktree mode: `direct-local-unless-requested` (single agent)
- Root Markdown policy: canonical-restricted

Resolve the integration checkout from the current repository's Git metadata
before using it. Do not create additional worktrees unless explicitly requested.

<!-- powercode-agent-modules:start -->
## Shared Agent Modules

At task start, after reading this root file and before inspecting or changing
other repository files, read these files once in order:

1. `AGENTS.Workflow.md`
2. `AGENTS.TerminalCompletion.md`
3. `AGENTS.CodeStyle.md`
4. `AGENTS.Documentation.md`
5. `AGENTS.BuildArtifacts.md`
6. `AGENTS.DotNetVerification.md`

Reuse loaded modules on follow-up prompts in the same task. Re-read them only
after the checkout, branch, module selection, or module contents change, or after
an interrupted module load. This root takes precedence over shared modules.
If a selected module is missing or unreadable, report the problem before other
repository work.
<!-- powercode-agent-modules:end -->

## Workflow

Work directly in the integration checkout on `develop`; only one agent works on
dotask at a time. Preserve unrelated changes. Commit scoped, verified changes
before handoff, including implementation, documentation, and metadata changes,
unless the user explicitly requests that they remain uncommitted. Include a new
or materially updated changelog entry in every agent-created commit. Do not
commit known-broken work unless the user requests a checkpoint.
Commit directly to `develop` in the normal single-checkout workflow. If an
isolated task worktree was requested, commit to its task branch; integration and
cleanup require explicit user acceptance and a request. Pushing, publishing,
installation, and deployment require separate authorization.
Preserve the user-owned notes in the parent directory. Work as a single agent.
Use ordinary filesystem tools, apply_patch, Git, and .NET tools as appropriate.

## Terminal Completion Facts

- Changelog path: `docs/CHANGELOG.md` relative to the current checkout.
- Changelog trigger: `every-agent-commit`, plus every completed repository task
  before handoff, including tasks left uncommitted at the user's request.
- Changelog date basis: completion date for an uncommitted task; commit date when
  committing. Preserve historical entries and their dates.
- Integration lock: `../.locks/integration` relative to the integration checkout;
  only relevant when an explicitly requested disposable worktree is integrated.
- User-maintained notes: machine-local notes in the checkout's parent directory;
  do not edit them unless explicitly requested.

Add a dated entry with the initial completed implementation, documentation, or
tooling change. Every agent-created commit must include a new or materially
updated entry. Update the same entry for follow-up changes to behavior, scope,
or verification rather than duplicating the original outcome. Do not wait for a
commit request to document completed work, and do not treat `docs/VERIFICATION.md`
as a substitute for the changelog.

Append entries at the end of `## Unreleased` using `### YYYY-MM-DD Title`, in
ascending date order and completion order within a date. Record completed
outcomes and exact verification, including acceptance still pending. Exclude
deferred, unimplemented, unrelated, and non-dotask work. Omit the current commit's
hash from an entry included in that commit.

Use the selected terminal-completion module for explicit completion requests
and its standalone approval phrases. Those authorize scoped local completion,
not pushing, publishing, installation, deployment, or deleting user-owned work.
For direct work on `develop`, skip worktree integration and cleanup. If a
disposable worktree was explicitly requested, acquire the integration lock only
while synchronizing and integrating; update from local `develop`, resolve and
reverify, fast-forward the integration checkout, prove task-tip ancestry, rerun
the applicable gate, and release the lock. Clean up only the accepted disposable
worktree and branch, without force, after successful integration.

Before final handoff, check the scoped diff, changelog, required documentation,
verification results, and Git status. Report whether changes are committed,
integrated, installed, published, or pushed; do not conflate those states.

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
- One entry file per callable task; common ordinary-comment YAML headers own
  C# and Rust task metadata.
  Declarative `.task` YAML groups compose existing targets without registration.
- Discovery accepts `.cs` and `.rs` targets and `.task` YAML groups; ordinary documents,
  `.yaml` files, and MSBuild `.targets` files are not tasks. Windows native bootstrap
  staging lives in `.tasks/misc/prepare-bootstrap.ps1`.
- Author reusable task changes in `shared-tasks/`. Keep installed copies under
  `.tasks/_/`, including `doinstall/`, identical to their canonical sources.
- Recursive task paths define full names; shared copies include source/group/task.
  The official source is `_`, the default for omitted sources; only the task-root
  `_` directory is exempt from underscore-prefixed discovery exclusions.
  Exact names precede unique shortcuts; old space-grouped filenames remain supported.
  Use source-qualified paths for nested shared-task calls, YAML defaults, and identity.
- Shared management copies files into projects and tracks original SHA-256 hashes in
  `.dotasks-lock.yaml`. Never overwrite/adopt untracked files or remove local edits.
  Private task originals are user data. See `docs/SHARED-TASKS.md`.
- Share metadata between help, validation, and shell completion. Help reports
  metadata/configuration errors; compilation errors are reported on execution.
- Help and completion never compile, restore packages, or execute target code.
- Bare `dotask` and `dotask help` show targets and the detailed-help hint;
  `--verbose` adds project identity, settings, and combined target options;
  project/target help reads YAML and checks effective defaults.
- CLI-only `--help`/`-h` skips project discovery and reads; target completion skips project configuration; management completion stays local.
- Keep YAML declarative and keep services deferred.
- Shared `installer/install` requires a `create-installer` returning an
  `InstallerArtifact`, then launches it. It never falls back to copying files.
  The external doinstall runtime uses YAML receipts and preserves
  existing ownership; payload selection stays in `.tasks/installer.yaml`. See `docs/INSTALLATION.md`.
  Preserve installation ownership, immutable builds, and activation recovery journals.
  Test with explicit temporary install/bin roots; do not modify the user's active install.
- Use an ambient `BuildContext.Current`, ordinary composition, and no DI container.
- Target existence and optional execution use the CLI catalog. Only an absent
  target is `NotFound`; ambiguity and target failures must not be silently skipped.
- Preserve arbitrary process arguments. Normalize only explicitly portable paths.
- Keep `Program.cs` thin and failure/cancellation behavior explicit.

## Build And Verification

- Source changes: `./build.sh verify` on Linux/macOS; `build.cmd verify` on Windows.
  Bare launchers bootstrap the checkout and list tasks; all arguments are forwarded.
- Documentation-only changes: `./build.sh verify-docs` (Windows: `build.cmd verify-docs`); check relative links/anchors
  and run changed command or complete target examples where practical.
- Focused tests may use `dotnet test dotask.slnx --filter ...` directly.
- The launchers accept dotask arguments, resolve their own checkout, and preserve
  failures. No installed dotask, Make, or Python is required. CI uses the same launchers.
- Keep bootstrap limited to building and staging a temporary native runner.
  Rust tasks use the source SDK; C# execution prepares the helper through the
  Rust `build-sdks` task using evaluated `PublishDir`. All build/test/format/docs/catalog/package operations
  belong in `.tasks`. Never run repository rebuilds from the live build output.
- `verify` requires every stage. The reusable `dotnet/verify` task's optional
  checks are not the repository verification gate.
- Regenerate `shared-tasks/catalog.json` with `./build.sh catalog` after editing
  shared sources/support files, after final formatting and before committing the
  sources and index together. `./build.sh catalog --verify` checks without rewriting it.
- `verify-docs` declares and executes `git/check` with `Whitespace = true` after
  checking required documents; plain `git/check` only checks tool availability.
- All changes: `git diff --check`.
- Doinstall owns installer, builder, shim sources/assets, and behavior verification
  in its independent repository. Do not reintroduce those implementations here.
  Canonical shared `_/doinstall/*` tasks wrap its tools; `_/installer/install`
  launches generic artifacts. The source checkout for development is selected by
  `settings.doinstall.source`; prebuilt tools can be supplied to assemble directly.
- The gate tests dotask's Rust CLI/SDK with Rust 1.95+, rustfmt, and clippy, then
  SDK and external-tool adapter integration. `test-sdks` prepares the configured
  external tool pair through the shared build wrapper. Independent doinstall
  tests belong to its own `build.cmd verify`/`./build.sh verify` gate. Keep outputs
  external and query each project's evaluated Cargo target directory.
- The Rust CLI reads common YAML headers without language toolchains and runs
  Rust tasks, C# tasks and YAML groups. C# compilation invokes the installed
  .NET SDK directly with isolated MSBuild hooks and the maintained helper DLL.
  Rust tasks use `src/dotask-sdk` through external source snapshots. Ship only
  the native command, `sdk/dotnet/Dotask.dotnet.dll` and the Rust helper source
  crate under `sdk/rust/`;
  never bundle language runtimes, SDKs, Roslyn or a managed support host.
  Keep the C# CLI as the behavior reference. See `docs/DESIGN.md`.
- Cargo output is configured in `.cargo/config.toml`; build tasks, staging,
  packaging, and native tests query `cargo metadata` for its evaluated target
  directory. Preserve the user's Cargo and rustfmt settings.
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
instructions. `docs/CHANGELOG.md` records completed repository work before
handoff under the terminal-completion rules above. Shared agent modules are
included copies from PowerCode; the build-artifact and .NET verification modules have been adapted for
this repository's bootstrap workflow. No separate checkout is required to use
this repository. Other project files are project-owned.

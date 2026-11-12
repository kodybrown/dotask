# dotask Agent Notes

Reusable agent policy comes from the global instruction collection. The facts,
contracts, and exceptions below are specific to dotask.

## Project Facts and Exceptions

- Product/command: `dotask`. C# namespace: `DoTask`, including `DoTask.Runtime`.
- Project type: Rust CLI, Rust/Go task SDKs, and a .NET 10 C# task helper library.
- Work as a single agent. Preserve user-maintained notes in the main checkout's
  parent directory; do not edit them unless explicitly requested.
- Root Markdown layout: `canonical-restricted`.
- Integration lock: `../.locks/integration` relative to the Git-verified main
  checkout, only for integration of an explicitly requested disposable worktree.
- Changelog: `docs/CHANGELOG.md`. Also record every completed repository task
  before handoff when the user requests leaving it uncommitted. Use completion
  date for those tasks and commit date when committing; retain completion order
  within a date. `docs/VERIFICATION.md` is not a substitute for the changelog.
- `docs/README.md` is the documentation index; `docs/DESIGN.md` owns design
  decisions. Other project files remain project-owned.
  Building the CLI and running the fast checks require no separate source checkout.
- Current native installer task: `create-dotask-installer`. Do not assume the
  preferred generic task name exists before a separately scoped migration.
- Repository task/help reference: `build.cmd help` or `./build.sh help`.

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
  C#, Rust and Go task metadata.
  Declarative `.task` YAML tasks compose targets or execute apps directly, with
  typed options, exact scalar/path references, staged process file output, and
  simple boolean conditions. No
  implicit shell, general expression language or task-code compilation is involved.
- Discovery accepts `.cs`, `.rs` and `.go` targets and `.task` YAML groups; ordinary documents,
  `.yaml` files, and MSBuild `.targets` files are not tasks. Windows native bootstrap
  staging lives in `.tasks/misc/prepare-bootstrap.ps1`.
- Author reusable task changes in `shared-tasks/`. Keep installed copies under
  `.tasks/_/`, including `doinstall/`, identical to their canonical sources.
- Recursive task paths define full names; shared copies include source/group/task.
  The official source is `_`, the default for omitted sources; only the task-root
  `_` directory is exempt from underscore-prefixed discovery exclusions.
  Exact names precede unique shortcuts; old space-grouped filenames remain supported.
  Use source-qualified paths for nested shared-task calls, YAML defaults, and identity.
- Coexisting C#, Rust, Go, and YAML implementations use extension-qualified names;
  extensionless calls must be unambiguous. Defaults for a qualified name override
  base-name defaults, and nested calls/cycle detection distinguish implementations.
  CLI `--lang csharp|rust|go` selects the requested target/help implementation only;
  it is reserved and never inherited by nested calls or interpreted as directory execution.
- Shared management supports group selections and `--lang csharp|rust|go` filters
  for selected roots. Declared dependencies retain their implementation; sync
  updates only tracked variants and preserves their existing ownership identities.
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
- Shared `installer/install` launches an existing `installer.installer_file`
  path/template and never invokes a creation task. Missing configuration offers
  guided setup using explicit successful-build records or bounded discovery.
  Record successful builds through the SDK only after all packaging steps pass;
  keep records separate from user configuration. It never copies application files.
  The external doinstall runtime uses YAML receipts and preserves
  existing ownership; payload selection stays in `.tasks/installer.yaml`. See `docs/INSTALLATION.md`.
  Preserve installation ownership, immutable builds, and activation recovery journals.
  Test with explicit temporary install/bin roots; do not modify the user's active install.
- Use an ambient `BuildContext.Current`, ordinary composition, and no DI container.
- Target existence and optional execution use the CLI catalog. Only an absent
  target is `NotFound`; ambiguity and target failures must not be silently skipped.
- Preserve arbitrary process arguments. Normalize only explicitly portable paths.
- Keep `main.rs` thin and failure/cancellation behavior explicit.

## Build And Verification

- Source changes: `./build.sh test` on Linux/macOS; `build.cmd test` on Windows.
  This is the routine fast Rust core/CLI smoke/SDK gate, plus docs/catalog.
  Run relevant focused language/SDK tests for affected integrations. Use
  `test-all` for cross-language execution, bootstrap, build/release tooling,
  installer or SDK-layout changes, before release promotion, or when explicitly
  requested; do not run the complete suite for every small CLI/docs change.
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
  Bootstrap runners are temporary physical copies of publish output outside
  the checkout; do not redirect repository outputs to stage a runner.
- `test-all` is the complete gate: shared Rust build/tests followed by the required
  project `verify` task. CI must call `test-all` explicitly. `test` uses the
  isolated unoptimized bootstrap profile and skips compiler integration probes;
  it cannot establish release-build acceptance. `verify` owns extra SDK/integration,
  task formatting, documentation and catalog checks; it must never call `test`
  or `test-all`. Standalone
  `verify` requires a successful inspected native build. Shared `rust/test`
  optionally calls a consumer's `verify`; dotask invokes it explicitly to
  forward tools/configuration, with the shared hook disabled to avoid duplication.
- Regenerate `shared-tasks/catalog.json` with `./build.sh catalog` after editing
  shared sources/support files, after final formatting and before committing the
  sources and index together. `./build.sh catalog --verify` checks without rewriting it.
- `verify-docs` declares and executes `git/check` with `Whitespace = true` after
  checking required documents; plain `git/check` only checks tool availability.
- Doinstall owns installer, builder, shim sources/assets, and behavior verification
  in its independent repository. Do not reintroduce those implementations here.
  Canonical shared `_/doinstall/*` tasks wrap its tools; `_/installer/install`
  launches generic artifacts. Dotask's local installer and full verification build
  tools from `settings.doinstall.source` by default; release CI supplies its pinned
  prebuilt bundle. Never bundle these tools in dotask's app payload.
  Dotask owns optional task adapters, not doinstall-specific native CLI behavior.
- The gate tests dotask's Rust CLI/SDK with Rust 1.95+, rustfmt, and clippy, then
  SDK and external-tool adapter integration. Go verification requires an installed
  stable Go 1.22+ compiler and gofmt. Required `test-go` runs the bundled SDK tests
  and the explicit prebuilt-tool native Go installer acceptance. `test-sdks` requires prebuilt
  runtime, builder, and launcher-template tools
  through `--doinstall-tools` or `DOINSTALL_TOOL_DIR`; the full gate prepares and
  forwards one staged tool directory. Independent doinstall
  tests belong to its own `build.cmd verify`/`./build.sh verify` gate. Keep outputs
  external and query each project's evaluated Cargo target directory.
- For self-extracting consumer installers, follow the build/gather recipe in
  docs/INSTALLATION.md. Use the declarative assemble-self-extracting task and
  the builder's result FilePath; never guess delivery names. Keep source-only
  packaging templates outside distributed runtime YAML.
- The Rust CLI reads common YAML headers without language toolchains and runs
  Rust tasks, C# tasks, Go tasks and YAML groups. C# compilation invokes the installed
  .NET SDK directly with isolated MSBuild hooks and the maintained helper DLL.
  Rust tasks use `src/dotask-sdk` through external source snapshots. Ship only
  the native command and dotask-doinstall-create-installer helper,
  `sdk/dotnet/Dotask.dotnet.dll` and the Rust helper source
  crate under `sdk/rust/` and the standard-library Go module under `sdk/go/`;
  never bundle language runtimes, SDKs, Roslyn or a managed support host.
  Maintain native CLI regression coverage and C# task support. See `docs/DESIGN.md`.
- Cargo output is configured in `.cargo/config.toml`; build tasks, staging,
  packaging, and native tests query `cargo metadata` for its evaluated target
  directory. Preserve the user's Cargo and rustfmt settings.
- Repository builds import the optional user-level MSBuild output policy.
- Keep development scratch and test inputs beneath the cleanup-excluded
  `%TEMP%/_rust` or `%TEMP%/_dotnet` directories (C:/tmp is a junction to
  `%TEMP%` on this machine). The launchers set a protected process-local temp
  root inherited by child tools. For direct Cargo/.NET test invocations, set
  TEMP/TMP (Windows) or TMPDIR (Unix) to a protected root first. Do not change
  persistent user/machine temp settings.
- Runtime target compilation is deliberately isolated from a consumer's build
  properties/targets and uses its own per-user external cache. Do not confuse
  target compilation with the repository's own output policy.

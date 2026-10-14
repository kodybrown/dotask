# Continue the Rust transition

Current scope is **Windows x64**. Linux and macOS acceptance are deferred at the
user's request. [DESIGN.md](DESIGN.md#rust-cli-transition) owns architectural
contracts; [VERIFICATION.md](VERIFICATION.md) owns dated acceptance evidence.

## Current implementation

- The Rust CLI runs C# tasks, Rust `.rs` tasks, and YAML groups, including nested
  calls in both directions, immutable settings snapshots, failures, cancellation,
  and invocation-local installer results.
- `src/dotask-sdk` supplies Rust task helpers. Metadata is leading ordinary-comment YAML;
  help and completion never compile or execute a task. Generated Cargo manifests,
  source/SDK snapshots, lockfiles, and outputs stay in an external per-user cache.
  See [Rust authoring](TARGETS.md#rust-tasks).
- Repository orchestration uses Rust tasks and declarative groups: `build`,
  `build-sdks`, `test-sdks`, `create-dotask-installer`, `verify`, `verify-docs`,
  `catalog`, and `check`.
  Shared C# tasks and the C# authoring library remain maintained.
- `build.cmd` and `build.sh` minimally bootstrap a temporary native runner and
  forward all arguments. No arguments list tasks; `verify` explicitly verifies.
  `build-sdks` prepares YAML-selected helper files. Repository operations remain tasks.
- `create-dotask-installer.rs` verifies both projects and delegates packaging to
  `_/doinstall/create-installer`, whose build steps build native binaries and publish the C# helper DLL
  from evaluated `PublishDir`. A fresh payload contains native `dotask[.exe]`,
  `sdk/rust/`, and `sdk/dotnet/Dotask.dotnet.dll`. The existing Rust
  builder creates packages and its runtime preserves immutable builds, receipts, and
  activation recovery.
- Installer creation uses `settings.installer.config` and `output`; stable
  metadata/options live in `.tasks/installer.yaml`. The local workflow explicitly
  stages external doinstall tools before verification and shared creation.
  `settings.installer.installer_file` independently selects an existing package
  for `install`; it never rebuilds. Successful creation records its artifact
  under ignored `artifacts/.dotask/installers/` without editing project settings.
- Public Rust versions preserve major/minor and default to
  `major.minor.yyDDD.HHmm`, with a frozen UTC `YYDDD-HHMM` stamp and optional Git
  revision. Packages and installed build names match apart from the app prefix.
  `--app-version` and `--build-stamp` override local creator choices;
  the generic engine accepts other version conventions and missing Git metadata.
  Existing version/build destinations are refused instead of overwritten.
- The C# executable is removed. Native process tests, fixed pre-removal output
  baselines, and Rust unit tests maintain regression coverage. The C# helper
  and consumer tasks remain supported. `create-dotask-installer` packages the native app.
- Shared catalogs, lockfiles, and installed shared task copies retain their
  schema and ownership rules for C# and Rust entries. Coexisting language
  variants, `--lang`, group selections, and extension-qualified task calls are
  implemented. Existing recorded identities remain stable when a sibling appears.

## Windows verification and package acceptance

Read [AGENTS.md](../AGENTS.md) and its selected modules. Work as one agent on
`develop`, preserve unrelated changes and the user's Cargo/rustfmt configuration,
commit scoped verified work with a changelog entry, and do not install into the
active user roots, publish, or push without separate authorization.

From the source checkout:

```powershell
.\build.cmd verify --doinstall-tools C:/Tools/doinstall
.\build.cmd help create-dotask-installer
```

The full gate checks all Rust workspace members, standalone Rust task formatting,
.NET tests, documentation, shared catalog freshness, external adapter integration,
and Git whitespace. Check C# formatting separately with `dotnet format dotask.slnx
whitespace --verify-no-changes`. `create-dotask-installer` creates a package without
installing it. Copy the entire printed package directory outside the checkout before testing
it. Use explicit fresh temporary install/bin roots and `--non-interactive --set
add-to-path=false` for unattended acceptance; never use active defaults or modify
the user's PATH. Test console prompts separately from a staged installer copy.

Package acceptance must exercise first installation, identical-build reuse,
changed-build activation, installed native arguments/streams, C# and Rust tasks,
nested calls, launcher ownership, and uninstall. Verify help/completion for both languages with all toolchain paths absent.
Rust execution needs Cargo/Rust and C# execution needs the selected .NET SDK.
Neither toolchain is bundled. Query Cargo's output path rather than guessing.
Verify evaluated .NET output paths for the helper and tests after tooling changes.

## Remaining work

1. Verify live HTTPS shared-catalog/file downloads with a fresh isolated cache;
   current management tests use local fixtures. Record unavailable publication
   separately from runtime failure; do not publish merely to satisfy acceptance.
2. Add reusable Rust-toolchain build/test/format tasks as concrete consumers need
   them; keep shared copies identical to their canonical sources.
3. Broaden Rust authoring APIs only as concrete tasks require them. Per-task
   arbitrary crate dependencies are not supported by the initial SDK.
4. Obtain Linux/macOS and Windows ARM64 acceptance when those platforms become
   relevant. Existing CI definitions do not establish successful execution.
5. Active installation and public release remain separate authorized actions.

Record exact commands, tool versions, counts, package acceptance, and limitations
in [VERIFICATION.md](VERIFICATION.md). Update the changelog for completed work;
implementation, acceptance, installation, publication, and pushing are distinct.

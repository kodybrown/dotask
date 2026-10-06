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
- Repository orchestration is Rust: `installer-engine`, `rust-cli`,
  `create-installer`, `pack`, `verify`, `verify-docs`, `catalog`, and `shim`.
  Shared C# tasks and the C# authoring library remain maintained.
- `build.cmd` and `build.sh` stage a native runner, `sdk/rust/` sources, and `sdk/dotnet/Dotask.dotnet.dll`
  helpers into a fresh temporary directory. Repository operations remain tasks.
- `create-installer.rs` builds native binaries and publishes the C# helper DLL
  from evaluated `PublishDir`. A fresh payload contains native `dotask[.exe]`,
  `sdk/rust/`, and `sdk/dotnet/Dotask.dotnet.dll`. The existing Rust
  installer creates packages and preserves immutable builds, receipts, and
  activation recovery. `pack.rs` uses the same creator.
- Installer wiring uses `settings.installer.config` and `output`, with optional
  `engine`. Stable metadata/options live in `.tasks/installer.yaml`. The local
  wrapper stages the executable, helper DLL and SDK, fills dynamic fields, and
  calls the existing shared creator for actual packaging.
- Public Rust versions preserve major/minor and default to
  `major.minor.yyMM.ddhh`, with a frozen UTC `YYDDD-HHMM` stamp and optional Git
  revision. Packages and installed build names match apart from the app prefix.
  `--app-version`, `--git-hash`, and `--build-stamp` override local creator choices;
  the generic engine accepts other version conventions and missing Git metadata.
  Repeated requested minutes advance with a warning before the app is compiled.
- The C# CLI remains the behavior reference. Explicit `dotnet/pack` packages
  that reference; the local `pack` target packages the native application.
- Shared catalogs, lockfiles, and installed shared task copies retain their
  current C# schema and ownership rules. Rust variants, `--lang`, and
  extension-qualified task calls remain future work.

## Windows verification and package acceptance

Read [AGENTS.md](../AGENTS.md) and its selected modules. Work as one agent on
`develop`, preserve unrelated changes and the user's Cargo/rustfmt configuration,
commit scoped verified work with a changelog entry, and do not install into the
active user roots, publish, or push without separate authorization.

From the source checkout:

```powershell
.\build.cmd
.\build.cmd pack
.\build.cmd help create-installer
```

The full gate checks all Rust workspace members, standalone Rust task formatting,
.NET tests, C# formatting, documentation, shared catalog freshness, bundled shim
hashes, and Git whitespace. `pack` creates an installer package without installing
it. Copy the entire printed package directory outside the checkout before testing
it. Use explicit fresh temporary install/bin roots and `--non-interactive --set
add-to-path=false` for unattended acceptance; never use active defaults or modify
the user's PATH. Test console prompts separately from a staged installer copy.

Package acceptance must exercise first installation, identical-build reuse,
changed-build activation, installed native arguments/streams, C# and Rust tasks,
nested calls, launcher ownership, and uninstall. Verify help/completion for both languages with all toolchain paths absent.
Rust execution needs Cargo/Rust and C# execution needs the selected .NET SDK.
Neither toolchain is bundled. Query Cargo's output path rather than guessing.
Verify evaluated .NET output paths for all three projects after tooling changes.

## Remaining work

1. Verify live HTTPS shared-catalog/file downloads with a fresh isolated cache;
   current management tests use local fixtures. Record unavailable publication
   separately from runtime failure; do not publish merely to satisfy acceptance.
2. Add shared Rust catalog variants and exact installed-file tracking without
   silently adopting untracked files or adding legacy migrations. Implement
   extension-qualified selection and `--lang csharp|rust` consistently across
   help, completion, defaults, nested calls, and cycle detection.
3. Duplicate a useful shared set beginning with `git/check.rs`, then Rust-toolchain
   tasks under `shared-tasks/rust/`. Retain C# implementations and keep installed
   copies identical to canonical sources. Regenerate the catalog after formatting.
4. Broaden Rust authoring APIs only as concrete tasks require them. Per-task
   arbitrary crate dependencies are not supported by the initial SDK.
5. Obtain Linux/macOS and Windows ARM64 acceptance when those platforms become
   relevant. Existing CI definitions do not establish successful execution.
6. Active installation and public release remain separate authorized actions.

Record exact commands, tool versions, counts, package acceptance, and limitations
in [VERIFICATION.md](VERIFICATION.md). Update the changelog for completed work;
implementation, acceptance, installation, publication, and pushing are distinct.

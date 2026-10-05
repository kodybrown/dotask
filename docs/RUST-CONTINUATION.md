# Continue the Rust transition

This is the continuation runbook as of **2026-10-05**, starting from implementation
commit `bfca6b4` on `develop`. It describes verification and the next implementation
steps; it does not record Linux or macOS acceptance as completed.
The user plans to verify Linux next. macOS access is not yet confirmed.

[DESIGN.md](DESIGN.md#rust-cli-transition) owns the agreed architecture and language
selection rules. [VERIFICATION.md](VERIFICATION.md) owns completed acceptance
evidence. Update those references when decisions or results change, and update
this runbook as stages finish.

## 1. Current state and boundaries

| Area | State at the starting commit |
| --- | --- |
| Rust CLI, `src/dotask-cli` | Runs C# tasks and YAML groups; implements discovery, configuration, binding, help, completion, initialization, shared management, and the group wizard. |
| C# support, `src/Dotask.CSharpHost` | Reads C# metadata and compiles C# tasks through private protocol version 2. Staged beside the native CLI in `release/csharp`. |
| C# CLI, `src/Dotask.Cli` | Still bootstraps repository builds and is the packaged application. Retained as the behavior reference. |
| C# authoring library, `src/Dotask` | Maintained public task API. Keep it and the existing C# tasks. |
| Rust installer, `src/dotask-installer` | Implemented; no replacement installer project is needed. |
| Rust task execution and SDK | Not implemented. `.rs` task discovery, language variants, and `--lang` are future work. |
| Verified baseline | Windows x64: 407 .NET tests and 25 Rust tests, full repository gate, native greeting, and terminal wizard save/cancel. See the dated verification record. |
| Remaining acceptance | Native Linux/macOS, live remote catalog, and the eventual native package. Older Linux results for the C# CLI do not validate this replacement. |

Preserve the user's `rustfmt.toml` and `.cargo/config.toml`. Cargo currently writes
to `/tmp/_rust/dotask/target` on Unix and `C:\tmp\_rust\dotask\target` on Windows;
the latter is beneath the user's `C:\tmp` junction. Build tasks query Cargo's
evaluated target directory. Stable rustfmt warns about nightly-only options and
applies the supported settings; changing toolchains or formatting policy is a
separate decision.

Read [AGENTS.md](../AGENTS.md) and its selected modules before continuing work.
The current repository workflow is one agent directly on `develop`, preserving
unrelated changes. Commit scoped verified work with a changelog entry. Pushing,
publishing, and changing the active installation need separate authorization.

## 2. Linux acceptance

Use a Linux checkout containing `bfca6b4` or a descendant. The implementation was
committed locally on Windows; confirm it has actually reached the Linux checkout
instead of assuming remote `develop` contains it. These commands do not transfer
or push commits. Use the same Bash session for the following blocks.

### Prepare the checkout and capture the environment

Replace `/path/to/dotask` with the Linux checkout location:

```bash
cd /path/to/dotask
dotask_repo="$(pwd -P)"
git status --short --branch
git log -3 --oneline
git merge-base --is-ancestor bfca6b4 HEAD
```

Stop if the ancestry check fails. Account for any local changes before testing;
do not reset or discard them. Prerequisites are Git, the .NET SDK selected by
`global.json` (10.0.300 or a later .NET 10 feature band), Rust 1.95+, rustfmt,
clippy, and the host linker. A .NET runtime alone cannot compile C# tasks.
Initial restores need package-network access. No installed dotask is required.

```bash
dotask_acceptance="$(mktemp -d "${TMPDIR:-/tmp}/dotask-acceptance.XXXXXXXX")"
printf 'Acceptance files: %s\n' "$dotask_acceptance"
{
  git rev-parse HEAD
  git status --short --branch
  uname -a
  dotnet --version
  rustc -Vv
  cargo --version
  cargo fmt --version
  cargo clippy --version
} > "$dotask_acceptance/environment.txt" 2>&1
cat "$dotask_acceptance/environment.txt"
cargo metadata --locked --format-version 1 --no-deps
for project in src/Dotask/Dotask.csproj src/Dotask.Cli/Dotask.Cli.csproj \
  src/Dotask.CSharpHost/Dotask.CSharpHost.csproj tests/Dotask.Tests/Dotask.Tests.csproj; do
  dotnet msbuild "$project" -p:Configuration=Release \
    -getProperty:BaseOutputPath,BaseIntermediateOutputPath,PublishDir
done
```

Check every prerequisite result. Cargo's `target_directory` should be external.
All three MSBuild paths must resolve under `/tmp/_dotnet`, following the machine's
user-level `~/Directory.Build.props` and `~/Directory.Build.targets`. If that
policy is missing, arrange it before building; do not overwrite existing user
files or add source-local output overrides. The repository imports that policy
when present and does not supply a replacement for a fresh machine.

### Run the complete gate

```bash
(
  set -euo pipefail
  ./build.sh 2>&1 | tee "$dotask_acceptance/verify.log"
  git diff --check
)
```

Require exit code 0. The gate builds both Rust packages, stages the C# support
host, runs Rust and .NET tests, and checks formatting, clippy, documentation,
catalog freshness, and shim hashes. At the starting revision the Windows baseline
is 407 .NET and 25 Rust tests; report actual host counts and skipped cases rather
than treating that number alone as acceptance. The native tests include direct
and nested cancellation, descendant cleanup, arguments, defaults, shared-task
ownership, recovery, and C# interoperability.

If a case fails, keep its diagnostics and fix or reproduce that case first. For
example, after the native artifacts have been built:

```bash
dotnet test dotask.slnx -c Release --filter FullyQualifiedName~RustCliTests
```

After a source fix, rerun `./build.sh`. Record environmental prerequisites
separately from implementation failures.

### Exercise the actual native executable

The wrapper currently launches the C# CLI, so use the native binary directly for
these checks. The path below matches the checked-in Cargo configuration. If
`CARGO_TARGET_DIR` or another Cargo setting changes it, use the `target_directory`
reported above. Keep `csharp/` beside the executable; `cargo build` alone does not
stage that support. `./build.sh rust-cli` rebuilds and stages the complete preview.

```bash
dotask_native=/tmp/_rust/dotask/target/release/dotask
(
  set -euo pipefail
  test -x "$dotask_native"
  test -f "$(dirname "$dotask_native")/csharp/Dotask.CSharpHost.dll"
  "$dotask_native" --version
  "$dotask_native" --help
  cp -R "$dotask_repo/examples/basic" "$dotask_acceptance/basic"
  cd "$dotask_acceptance/basic"
  "$dotask_native"
  "$dotask_native" --verbose
  "$dotask_native" help hello
  "$dotask_native" hello --name Rust --configuration Release
  "$dotask_native" greet
  "$dotask_native" write
  cat artifacts/example.txt
  if "$dotask_native" hello --configuration invalid; then
    echo 'ERROR: invalid configuration was accepted' >&2
    exit 1
  else
    test "$?" -eq 1
  fi
)
```

Expect `Hello from dotask, Rust!`, host `Linux`, and configuration `Release` for
the explicit greeting. `greet` prints the YAML-group greeting; `write` makes nested
C# calls and writes `Written by dotask.` in the temporary copy. Help should show
the effective `Developer` name default. Invalid configuration must report the
allowed choices without running the greeting. See the
[basic example](../examples/basic/README.md) for the complete expected output.

### Check initialization and local shared management

Use isolated cache/private roots so these tests do not alter normal project or
shared-task state. This block uses the repository catalog, not the network:

```bash
(
  set -euo pipefail
  mkdir "$dotask_acceptance/local-project" "$dotask_acceptance/private"
  cd "$dotask_acceptance/local-project"
  export DOTASK_CACHE_HOME="$dotask_acceptance/local-cache"
  export DOTASK_PRIVATE_TASKS="$dotask_acceptance/private"
  export DOTASK_ONLINE_TASKS="$dotask_repo/shared-tasks"
  "$dotask_native" --init
  "$dotask_native" --init
  "$dotask_native" --list 'git/*'
  "$dotask_native" --add git/check --dry-run
  "$dotask_native" --add git/check
  "$dotask_native" _/git/check
  cp .dotasks-lock.yaml before-sync.yaml
  "$dotask_native" --sync --dry-run
  "$dotask_native" --sync
  cmp before-sync.yaml .dotasks-lock.yaml
  "$dotask_native" --remove git/check --dry-run
  "$dotask_native" --remove git/check
  test ! -e .tasks/_/git/check.cs
)
```

Expect idempotent initialization, successful Git detection, byte-identical tracking
after no-op sync, and removal of the tracked task. The automated suite covers
local-edit conflicts, dependency handling, untracked-file refusal, and journals;
do not delete recovery data to force a failing case to pass.

### Check live catalog downloads separately

The configured public source is the catalog and files under `shared-tasks/` on
`kodybrown/dotask`'s `main` branch. Its publication status must be checked at test
time. Use a fresh cache and remove the local-source override:

```bash
(
  set -euo pipefail
  mkdir "$dotask_acceptance/remote-project"
  cd "$dotask_acceptance/remote-project"
  export DOTASK_CACHE_HOME="$dotask_acceptance/remote-cache"
  export DOTASK_PRIVATE_TASKS="$dotask_acceptance/private"
  unset DOTASK_ONLINE_TASKS
  "$dotask_native" --init
  "$dotask_native" --list 'git/*'
  "$dotask_native" --save git/check
  "$dotask_native" --add git/check
  "$dotask_native" _/git/check
  "$dotask_native" --sync
  "$dotask_native" --remove git/check
) > "$dotask_acceptance/remote.log" 2>&1
dotask_remote_exit=$?
cat "$dotask_acceptance/remote.log"
printf 'Remote check exit: %s\n' "$dotask_remote_exit"
```

If the public catalog is unavailable, record its diagnostic and leave live download
acceptance pending. A local-source pass does not cover HTTPS transport. Do not
publish anything merely to satisfy this check. See
[the source contract](SHARED-TASKS.md#official-catalog-and-unpublished-preview).

### Check terminal behavior

From a real terminal, without redirecting input/output:

```bash
cd "$dotask_acceptance/basic"
"$dotask_native" --create-task
```

Create a uniquely named group containing `hello`, set a name parameter, inspect
the preview, and save. Run the resulting group by its name. Start another wizard
and try `:back` while entering a step, then `:cancel`; verify no unfinished group
was written. Start once more and press Ctrl+C; `echo $?` immediately afterward
should report 130. Do not pipe scripted answers into the wizard.

For Bash completion, open an isolated interactive shell:

```bash
PATH="$(dirname "$dotask_native"):$PATH" bash --noprofile --norc
```

Inside that shell, run `command -v dotask` to confirm the native path, then:

```bash
source <(dotask completion bash)
```

Try Tab after `dotask he`, `dotask hello --na`, and
`dotask hello --configuration `. Expect target, option, and choice suggestions.
Exit that shell afterward. Record which shells were actually exercised; generating
Zsh/Fish/PowerShell scripts is not equivalent to interactive acceptance in them.
No profile edits are necessary. See [completion](USAGE.md#shell-completion).

Keep the temporary acceptance directory until results are recorded. If repeating
the procedure, allocate a fresh directory. Finish with `git -C "$dotask_repo"
status --short` and explain any changes. None of these manual checks should modify
the active installation or repository task copies.

## 3. macOS when a machine is available

Run the same Bash procedure on macOS, recording OS version, CPU architecture, SDK,
Rust toolchain, commit, and shell. Expect `MacOS` in the greeting. Test cancellation,
executable permissions, symlink launchers, and the eventual packaged layout on
that host; Windows and Linux results do not prove them.

The existing [Verify workflow](../.github/workflows/verify.yml) already has a
`macos-latest` job that runs `./build.sh` and `./build.sh pack`. After an authorized
push or PR, inspect the job for the exact tested commit, host architecture, and
results. This can supply automated macOS evidence without a personal Mac. It does
not establish terminal wizard, interactive shell, Finder/trust, or active-install
acceptance. Its `pack` step currently produces the C# CLI's NuGet package; it does
not exercise the standalone Rust installer or a native CLI payload.

If macOS access remains unavailable, keep it explicitly **pending** and continue
preparing implementation and tests. Before distributing a Rust replacement,
either obtain macOS evidence or ask for an explicit decision to limit that release
to verified platforms. Do not infer that decision from lack of a Mac.

## 4. Packaging and bootstrap cutover

After reviewing platform results, make the native executable the delivered
`dotask`. This is a new behavioral task; it is not performed by this runbook.

1. Build a fresh payload from the Cargo output and the C# support host's evaluated
   `PublishDir`. Include native `dotask[.exe]`, its `csharp/` support files, and the
   maintained C# authoring library. Verify no dependency on a checkout-local path,
   installed reference CLI, or live build output remains. Preserve the current
   runtime packaging options; C# execution still requires an appropriate SDK.
2. Change [.tasks/create-installer.cs](../.tasks/create-installer.cs) to package
   that payload with the existing Rust installer and `InstallerArtifact` contract.
   Review the current [dotnet/pack task](../shared-tasks/dotnet/pack.cs),
   [repository settings](../.dotasks.yaml), build launchers, CI, and install tests
   so no distribution path silently keeps delivering the old CLI. Keep bootstrap
   scripts limited to building/staging the runner; repository operations stay tasks.
   Cut over bootstrap only after native execution of those tasks is verified.
3. Use explicit temporary install/bin roots for package tests. Exercise first
   install, same-build reuse, changed-build activation, launcher arguments and
   streams, C# execution and nested calls, recovery, and uninstall ownership.
   Test a relocated package outside the checkout. Follow
   [INSTALLATION.md](INSTALLATION.md); do not replace YAML receipts, bypass the
   installer with direct copies, or add legacy migration paths.
4. Rerun the full gate and packaged smoke tests on supported release hosts. Update
   README, usage, design, AI guidance, installation, verification, and changelog
   where the advertised entry point changes. Keep `src/Dotask` and all C# tasks.
   Retire reference CLI code only after its verification role is resolved.
5. Report code completion, platform acceptance, packaging, active installation,
   and publication separately. Committing a cutover does not install or release it.

## 5. Rust authoring, variants, and task duplication

Proceed after the replacement CLI milestone. These are planned interfaces, not
commands supported by the current executable.

1. Add the bundled helper crate at `src/dotask-sdk`. Define and document its task
   entry, metadata, typed parameters/settings, process helpers, and invocation
   protocol. Each task has one `.rs` entry file; generate its Cargo manifest and
   build artifacts in an external cache and reference bundled crate source.
   Use the Rust toolchain to compile a native program; there is no Rust runtime
   analogous to the .NET runtime. Settle dependency/version and cache invalidation
   rules before exposing the authoring API. Help/completion must remain metadata-only.
2. Prove Rust-to-Rust, Rust-to-C#, and C#-to-Rust calls, including parameters,
   configuration snapshots, optional target lookup, ambiguity, cycles, exit codes,
   cancellation, and supported structured results. Start with small fixture tasks
   before porting the shared catalog.
3. Add `.rs` discovery and generated catalog variants based on actual source files.
   Track exact installed files and original hashes. Add extension-qualified calls
   and `--lang csharp|rust` filtering; keep extensionless calls unambiguous and apply
   the same identity rules to nested calls. No project-wide language setting.
   Plan any persisted-schema change explicitly; protect existing tracking and do
   not silently adopt files or introduce compatibility/migration machinery.
4. Duplicate a small useful set first: `shared-tasks/git/check.rs` beside
   `check.cs`, then Rust-specific build/test tasks under `shared-tasks/rust/`.
   Consumers receive `.tasks/_/git/check.rs` and `.tasks/_/rust/build.rs`, etc.
   Keep `_/dotnet/*` named for the .NET toolchain. Language selects an implementation,
   not the task group's name. Continue until the agreed shared-task parity set is
   duplicated, retaining the C# implementations and documenting inapplicable cases.
5. Verify planned installation behavior: `dotask --add git --lang csharp` selects
   C# variants, `--lang rust` selects Rust variants, and `dotask --add '_/git/*'`
   adds all available implementations. Explicit `git/check.rs` selects that file.
   `--sync` updates tracked variants; adding the group again discovers new variants.
   With both variants installed, use explicit extensions when the short name is
   ambiguous. Preserve local-edit, dependency, journal, and no-overwrite protections.
6. Keep installed copies under `.tasks/_/` identical to canonical `shared-tasks/`
   sources. Regenerate the catalog after final formatting with `./build.sh catalog`
   (Windows: `.\build.cmd catalog`), run the full gate, and commit sources, catalog,
   tests, reference docs, and changelog together.

## 6. Record results and hand off

Add a dated entry to [VERIFICATION.md](VERIFICATION.md) with the exact commit,
OS/architecture, tool versions, commands, exit codes, test counts/skips, terminal
checks, remote-catalog result, and relevant log locations. Label missing checks
pending. Add the completed outcome and verification to
[CHANGELOG.md](CHANGELOG.md); do not record planned work as implemented.

For documentation-only updates, run `./build.sh verify-docs` or
`.\build.cmd verify-docs`, check changed relative links/anchors, and run
`git diff --check`. For source changes, use the complete host gate. Preserve
user-owned configuration and unrelated changes; follow the current agent workflow
for the scoped local commit.

A future continuation request can be:

> Read AGENTS.md and docs/RUST-CONTINUATION.md. Confirm the current branch, commit,
> and changes. Review the supplied Linux/macOS acceptance results against
> docs/VERIFICATION.md, reproduce any failures, and update the evidence. Keep
> unavailable macOS checks pending. Identify the next uncompleted gate in this
> runbook before implementing it. Preserve C# tasks/library and the user's Cargo
> and rustfmt settings; do not install, publish, or push without authorization.

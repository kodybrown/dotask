# Reusable Rust tasks

Canonical sources live in dotask's `shared-tasks/rust/` catalog. Install them
from a consuming project's root with normal dotask management commands:

```sh
dotask --save rust
dotask --add rust
dotask --sync --dry-run
```

Use the whole group without `--lang rust`: the public operations are YAML tasks
and their helpers are Rust. Dependencies include `_support/Rust.rs` and this
README. Commit the installed `.tasks/_/rust/` files and `.dotasks-lock.yaml`.
Keep thin project entry points at task root; users need not call shared paths
directly. For example, `.tasks/build.task` can contain:

```yaml
requires: [{kind: task, value: _/rust/build}]
steps: [{run: _/rust/build}]
```

Review updates with `dotask --sync --dry-run`, then `dotask --sync`. Edit these
sources in dotask, regenerate its catalog, and publish `main` before syncing
consumers. Installed copies are managed files; local edits stop sync rather
than being overwritten. Existing handwritten copies must first be backed up
and moved aside explicitly; `--add` never adopts untracked files.

No reusable source hardcodes a product, repository, SDK path, or sibling checkout.
Rust task compilation uses the SDK bundled with installed dotask. Cargo-only
development and CI remain available.

| Shared task | Contract |
| --- | --- |
| `build` | Format, Clippy, locked Cargo build, remap private paths, scan executables |
| `run` | Build once and execute the exact scanned binary with literal arguments |
| `test` | Build once, run locked release-profile tests, then optional project `verify` |
| `format` | Cargo formatting; `--check` refuses changes |
| `clippy` | All-target Clippy; warnings fail |
| `update-version` | Explicit version increment, patch by default; no build or Git operations |

## Development

Build/run/test preserve the package version and do not invoke update-version.
They accept `--manifest` and `--package`. Build/run accept `--profile` (default
release); test uses release. A virtual workspace requires an explicit package
for building or versioning. Generic tests default to all workspace packages.

Formatting may modify Cargo source. Shared task sources are excluded by default
so consumer projects do not rewrite catalog-owned files. Leave `format-tasks`
false in consumer wrappers; only source authors should opt in while maintaining
the originals. Cargo formatting still formats ordinary project Rust sources.

Run accepts `--bin` (otherwise Cargo default-run or the only compiled binary)
and `--args`, a JSON array of literal strings. For example, a project wrapper
can forward `--args '["--help"]'`. The installed dotask CLI has no arbitrary
trailing-argument forwarding; do not split a shell command string instead.
Quotes, spaces, empty arguments and Unicode retain their argument boundaries.
The application exit code propagates to the caller. The exact inspected
executable is launched directly, avoiding an extra Cargo run with different flags.

Build supplies `RUST_BUILD_TIMESTAMP` (Unix seconds) and records human UTC
metadata beneath Cargo's external target directory. Project build scripts must
consume that contract to embed the date in their tools. The date does not change
SemVer ordering or edit the package version. Successful executable paths are
recorded from Cargo's compiler-artifact messages only after every scan passes.
Keep evaluated Cargo target/build directories outside source.

Test calls a project task named `verify` after Rust tests when it exists. Only
absence is skipped; ambiguity, invalid metadata and a failing hook stop the task.
Keep `verify` for extra product checks and never call `test` from it: that is a
cycle. Do not declare the optional hook as a required shared dependency. Use
`--verify false` when a project wrapper calls its required hook explicitly to
forward project-specific parameters. Tests retain the build's compiler flags
and timestamp and inspect final executables after successful tests (dev-dependencies
can change Cargo feature unification). Run checks the recorded hash before execution.

## Explicit version preparation

After tests pass, invoke the project's update-version task:

- No selector, or `--increment`: increment patch.
- `--increment-minor`: increment minor and reset patch.
- `--increment-major`: increment major and reset minor/patch.
- `--dry-run`: preview without writing.

Only the selected package's literal manifest version and local Cargo.lock entry
are changed. Surrounding formatting, BOM, EOLs, comments and dependency versions
are retained. Mutually exclusive selectors and inherited/workspace versions are
refused. The task validates Cargo metadata but never builds, tests, commits,
promotes, tags, pushes, publishes, or updates generated build-date records.

Review and commit the resulting version files along with intended code and
changelog changes, then fast-forward main and push through the project's normal
process. CI must verify that final commit without incrementing the version.
Publication belongs to the project's main-push workflow; there are no shared
publish/tag-release tasks.

## Project policy and packaging

Thin project wrappers select binaries and any extra verification such as shim
integrity and whitespace. Product-specific create-<app>-installer tasks own the
payload, matching tools, licenses, archive inspection and native acceptance.
They never run the installer against an active installation automatically.

Native CI matrices, compatibility baselines and release permissions remain
project policy. Pattern scans complement closed inventories and archive checks;
they do not recognize every possible secret. The low-level `cargo.rs` helper is
also cataloged so dependency installation and support-file ownership are explicit.
Source requirements are catalog-relative (`rust/...`); runtime task calls use
the installed source prefix (`_/rust/...`). Rust 1.95+, Cargo, rustfmt, Clippy
and a native linker are required. No .NET SDK or Python is needed for this group.

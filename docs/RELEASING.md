# Native releases

Dotask follows the shared console tool flow:
maintainers test, explicitly prepare a version, commit, fast-forward `main`, and
push. A push to `main` (including a merge) verifies and publishes that committed
version. There is no tag trigger or local publish task. GitHub Releases requires
an internal `vMAJOR.MINOR.PATCH` tag; the publishing job creates it at the exact
tested commit. Never manually tag to start a release.

## Deliveries and pins

For version `0.1.2`, the stable public names are:

| Platform | Portable CI bundle | Interactive installer |
| --- | --- | --- |
| Windows x64 | `dotask-v0.1.2-windows-x64-portable.zip` | `dotask-v0.1.2-windows-x64-setup.exe` |
| Linux x64 | `dotask-v0.1.2-linux-x64-portable.tar.gz` | `dotask-v0.1.2-linux-x64-setup` |

Each release also contains `SHA256SUMS`, `RELEASE-METADATA.json`, and
`dotask-vVERSION-PLATFORM-x64-metadata.json` for each platform. The archives have
no enclosing directory: extract into a dedicated directory and retain the whole
layout. Payloads contain the native executable, `sdk/dotnet/Dotask.dotnet.dll`,
`sdk/rust/Cargo.toml`, `sdk/rust/src/{lib,installers}.rs`, MIT license, readme,
dependency notices, and `RELEASE.json`. Portable and installer payload hashes
must match. Dotask's app payload contains no doinstall binaries, language runtimes,
or build tools.
The installer also carries doinstall's published license/dependency notices.
After downloading the Linux setup executable, use `chmod +x` before running it.

Rust 1.95.0, .NET SDK 10.0.402, and action commit SHAs are pinned. Windows releases
are built/tested on `windows-2022`; Linux on `ubuntu-22.04` x64, glibc 2.35. Linux
assembly rejects another baseline or ELF requirements newer than glibc 2.35.
The tar archive preserves executable modes, with normalized owner IDs/times.
Consumers need Rust/Cargo 1.95+ for Rust tasks and .NET SDK 10.0.300+ for C# tasks;
metadata and YAML-only tasks need neither. macOS retains a separate verification
gate and does not publish a native bundle yet.
Its existing explicit-source installer workflow remains available and creates a
development self-extracting installer after the full gate, without release assets.

[The reviewed doinstall pin](../.tasks/_release/doinstall.json) is v0.1.1, source
commit `208b9a915db1efe1404b421d9070c534aec8bafb`:

| Archive | Expected SHA256 |
| --- | --- |
| `doinstall-v0.1.1-windows-x64-tools.zip` | `1f80fcf8289e071a48654e58755cc6d5a15753ebf8e06b4990df07610021455d` |
| `doinstall-v0.1.1-linux-x64-tools.tar.gz` | `34b254bb9eaad821ea8907f579766c1f35f8c9507cb9f7b33591b95b7406a9cb` |

The release driver downloads that exact asset over HTTPS, checks the embedded
expected hash **before extracting or executing**, and compares the tools to
the archive again before packaging. Keep the verified archive beside `tools/`.
No `latest` resolution or doinstall source build is used for release platforms.
To update the pin, review the published release and both archives, replace the
version/commit/hashes together, then rerun both native gates. The macOS-only
source pin must be reviewed separately until a macOS bundle is published.

## Local review and release procedure

Run from the integration checkout on `develop` (PowerShell):
The full gate's publisher simulation requires Bash and `jq`; Windows uses Git Bash.

```powershell
cd C:\Users\kodyb\Projects\dotask
# All generated paths must be fresh and outside the checkout.
$bundle = Join-Path $env:TEMP ('dotask-tools-' + [guid]::NewGuid().ToString('N'))
cargo run --locked -p dotask-release -- prepare --output $bundle
$env:DOINSTALL_TOOL_DIR = Join-Path $bundle tools
.\build.cmd test
$output = Join-Path $env:TEMP ('dotask-release-' + [guid]::NewGuid().ToString('N'))
.\build.cmd create-dotask-installer --reuse-build --output $output
```

Linux uses the same Cargo commands with `./build.sh test` and
`./build.sh create-dotask-installer --reuse-build --output "$output"` on the
declared Ubuntu baseline. Bare `create-dotask-installer` builds/lints/stages first;
`--reuse-build` requires the existing inspected executable hash and reuses it.
Both modes assemble and test temporary installations, SDK discovery, Rust/C#
and YAML task execution, downstream packaging, and uninstall. They never alter
persistent PATH or active installations. `--source ../doinstall` preserves the
explicit local source-development workflow; unpacked tools without their pinned
archive can also produce development packages. Such packages cannot publish.
Only after all acceptance passes is the installer recorded for `build.cmd install`.

Normal build/run/test/package tasks never change SemVer. Before a **new** release,
run `build.cmd update-version` (patch by default; minor/major are explicit), review
the diff, rerun `test`, and commit. The native version must be stable
`MAJOR.MINOR.PATCH`, with no leading zeros, prerelease, or build metadata. UTC
build time is displayed separately. The workflow rejects a version already
assigned to a different commit. SDK library versions remain independent.

After review and authorization, with a clean checkout and no divergent `main`:
An optional `git push origin develop` runs CI/artifact review without publication
before promoting the first release. Manual dispatch is available once this
workflow exists on the repository's default branch.

```powershell
git switch main
git merge --ff-only develop
git push origin main
git switch develop
```

Pushes and pull requests run all gates. Use **Actions → Verify and release → Run
workflow** on a selected branch for artifact-only review; even manual runs on
`main` never publish. The `release-ready` artifact contains the combined assets
and checksums. Build jobs have only `contents: read`; only the final publisher
has `contents: write`. It downloads the already tested files, rechecks checksums,
uploads everything to a draft, and publishes only after uploads succeed. It
never replaces assets or moves an existing tag. A rerun for the same published
commit is a no-op. Failed/incomplete drafts require inspection before retrying;
authentication/network failures stop publication rather than implying absence.

Native binaries use path remapping; the .NET helper disables embedded PDB paths.
Assembly scans every expanded portable and self-extracting file for build paths
and credential patterns, enforces closed inventories, and records exact source,
compiler, dependency, and payload identities. These checks supplement review of
curated payloads and notices; they do not prove arbitrary input contains no secrets.

## Minimal pinned filekeep consumer

After the first dotask release, review its `SHA256SUMS` and **commit the chosen
archive hash in filekeep's workflow**. A real dotask archive hash cannot be known
before that build is published. The placeholder below deliberately fails closed.
This example runs filekeep's existing Rust installer task; it does not install
dotask or doinstall, modify PATH, or compile either tool from its source checkout.
The runner needs filekeep's supported Rust toolchain; add the .NET SDK only for
C# tasks.

```bash
set -euo pipefail
dotask_version=0.1.2
dotask_sha256=REPLACE_WITH_REVIEWED_64_HEX_HASH
[[ "$dotask_sha256" =~ ^[0-9a-f]{64}$ ]] || { echo 'Pin the published dotask hash first' >&2; exit 1; }
tools=$(mktemp -d)
curl --fail --location --proto '=https' --output "$tools/dotask.tar.gz" \
  "https://github.com/kodybrown/dotask/releases/download/v${dotask_version}/dotask-v${dotask_version}-linux-x64-portable.tar.gz"
curl --fail --location --proto '=https' --output "$tools/doinstall.tar.gz" \
  'https://github.com/kodybrown/doinstall/releases/download/v0.1.1/doinstall-v0.1.1-linux-x64-tools.tar.gz'
printf '%s  %s\n' "$dotask_sha256" "$tools/dotask.tar.gz" \
  34b254bb9eaad821ea8907f579766c1f35f8c9507cb9f7b33591b95b7406a9cb "$tools/doinstall.tar.gz" | sha256sum --check -
mkdir "$tools/dotask" "$tools/doinstall"
tar -xzf "$tools/dotask.tar.gz" -C "$tools/dotask"
tar -xzf "$tools/doinstall.tar.gz" -C "$tools/doinstall"
cd /path/to/filekeep
"$tools/dotask/dotask" create-filekeep-installer --doinstall-tools "$tools/doinstall"
```

On Windows select the corresponding ZIP assets and Windows hash above, verify
both with `Get-FileHash -Algorithm SHA256` before `Expand-Archive`, then invoke
`& "$tools/dotask/dotask.exe" create-filekeep-installer --doinstall-tools "$tools/doinstall"`.
Keep `sdk/` beside the native CLI. Upload the installer selected by the task's
successful-build record; do not guess the builder's generated inner filename.

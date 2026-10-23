# dotask portable distribution

Keep dotask[.exe], dotask-doinstall-create-installer[.exe], sdk/,
and the notices together.
Run the executable directly; no installation or source checkout is needed.
Rust tasks require Rust/Cargo 1.95+; C# tasks require .NET SDK 10.0.300+.
Go tasks require an installed stable Go 1.22+ compiler and bundled sdk/go.
Go execution never installs/downloads a toolchain or requires Rust/.NET.
YAML tasks, native installer creation, and metadata/help need no language toolchain.
Installer creation consumes separately installed precompiled doinstall tools;
application build steps still require their selected toolchains.

Linux x64 baseline: Ubuntu 22.04 with glibc 2.35. Preserve executable modes
when extracting tar.gz. Windows x64 builds are tested on Windows Server 2022.
macOS is verified separately and has no release bundle in this release flow.

`RELEASE.json` records the exact source commit, toolchains, pinned doinstall
dependency, and payload hashes. Verify the release SHA256SUMS before extraction.
The doinstall tools used to assemble this distribution are separate downloads;
they are not part of dotask's application payload.

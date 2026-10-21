# dotask portable distribution

Keep `dotask` (`dotask.exe` on Windows), `sdk/`, and the notices together.
Run the executable directly; no installation or source checkout is needed.
Rust tasks require Rust/Cargo 1.95+; C# tasks require .NET SDK 10.0.300+.
YAML tasks and metadata/help do not need either language toolchain.

Linux x64 baseline: Ubuntu 22.04 with glibc 2.35. Preserve executable modes
when extracting tar.gz. Windows x64 builds are tested on Windows Server 2022.
macOS is verified separately and has no release bundle in this release flow.

`RELEASE.json` records the exact source commit, toolchains, pinned doinstall
dependency, and payload hashes. Verify the release SHA256SUMS before extraction.
The doinstall tools used to assemble this distribution are separate downloads;
they are not part of dotask's application payload.

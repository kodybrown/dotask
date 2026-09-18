# DoTask Windows shim

`shim.c` is a small standalone Win32 launcher under the repository's MIT license.
It is inspired by Scoop's executable/sidecar pattern, with an intentionally
smaller format. No Scoop source or runtime dependency is included.

The installer copies the bundled binary to `<command>.exe` and writes UTF-8
`<command>.shim` next to it:

```ini
path = "C:\Users\Alice\AppData\Local\Programs\example\1.2.3-fingerprint\example.exe"
```

Only one quoted absolute `path` assignment is supported. No environment expansion,
extra arguments, elevation, shell evaluation, or working-directory change is
performed. The launcher locates the sidecar beside its own executable, not in the
current directory. It preserves the original Windows argument tail, inherits
environment/standard handles/current directory, and waits for the child's exit
code. Its console handler lets the child handle Ctrl+C and Ctrl+Break normally.
Missing/invalid sidecars and launch failures produce a stderr diagnostic and exit 1.

## Bundled binaries and rebuilding

`assets/win-x64.exe` and `assets/win-arm64.exe` are intentional checked-in delivery
assets. The library embeds them so installing another project never compiles or
downloads a shim. They import only Windows' Kernel32 APIs; there is no CRT or .NET
runtime dependency. `kernel32.def` and the small declarations in `shim.c` allow
cross-compilation without Windows SDK headers/libraries.

Normal `./build.sh` / `build.cmd` verification needs only the .NET SDK and Git.
Maintainers changing the shim also need LLVM's `clang`, `llvm-dlltool`, and
`lld-link` on PATH:

```sh
./build.sh shim
./build.sh shim --verify
```

Use `build.cmd` on Windows. Generated object/import libraries go to a temporary
directory. The task deliberately replaces the two delivery assets and
`assets/hashes.json`; review and commit the source, assets, and hashes together.
Initial binaries were built with LLVM 22.1.8. Output is deterministic with the
same toolchain; different LLVM versions may generate different bytes.

`--verify` checks recorded C source, import definition, build-task and binary hashes; it is not proof of
source-to-binary reproducibility. CI separately rebuilds both architectures from
source. The OS test matrix exercises the shipped shim on Windows, including
arguments, Unicode paths, stdin/stdout/stderr, exit codes and console cancellation.
ARM64 binaries need a native ARM64 acceptance run beyond the current x64 Windows
CI job. Linux PE inspection cannot substitute for Windows execution.

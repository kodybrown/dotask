# Windows x64 CLI size experiment (2026-10-05)

This is measured evidence for one committed baseline, not a change to the
[CLI transition design](../DESIGN.md#rust-cli-transition) or a claim of Native
AOT support. No runtime source, dependencies, or packaging defaults changed.

**Historical baseline:** every measurement and original smoke result below is
from `1df4d14d13530db497dc68bc7768ffd29cb993da`. The later common-header and
minimal-helper change, `7b28895aa31ff4f91ddf3f4b283bc2818b037b31`, removed the
managed C# support host and its Roslyn payload. Its separate
[verification record](../VERIFICATION.md) reports the smaller delivery layout.
The 20.42 MB Rust total here is not a measurement of that newer layout. This
report was subsequently rebased and accepted for integration onto `develop`;
rebasing the documentation did not rebuild or remeasure any executable.

## Result

The C# Native AOT executable emitted at **17.40 MB**, but project discovery and
C# execution crashed. It is not a working alternative at this baseline. The
working self-contained C# single-file executable measured **90.73 MB**, versus
**3.72 MB** for Rust. However, with installed language toolchains accepted, the
normal framework-dependent C# payload measured **17.31 MB**, versus **20.42 MB**
for Rust with its existing C# support and Rust SDK sources. Executable size and
complete deployment size answer different questions.

| Windows x64 Release variant | Executable bytes | Decimal MB | Binary MiB | Executable / Rust | Smoke result |
| --- | ---: | ---: | ---: | ---: | --- |
| C# Native AOT, size optimized, experimental | 17,403,392 | 17.403392 | 16.597168 | 4.6813 | Help/version only; discovery/execution crash |
| C# self-contained single file, full extraction | 90,726,248 | 90.726248 | 86.523293 | 24.4043 | All four pass |
| C# framework-dependent, ordinary apphost | 162,816 | 0.162816 | 0.155273 | 0.0438 | All four pass; requires companion files |
| Rust, static CRT | 3,717,632 | 3.717632 | 3.545410 | 1.0000 | All four pass with staged support |

MB means bytes / 1,000,000; MiB means bytes / 1,048,576. These are file lengths,
not disk allocation, compressed downloads, memory usage, or extracted-cache size.

| Variant | Non-debug shipped bytes | Decimal MB | Binary MiB | External companion bytes | Optional PDB bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| Experimental AOT | 17,403,392 | 17.403392 | 16.597168 | 0 emitted; CLI is broken | 52,568,836 |
| Self-contained single file | 90,726,248 | 90.726248 | 86.523293 | 0 | 76,864 |
| Framework-dependent C# | 17,306,045 | 17.306045 | 16.504331 | 17,143,229 | 76,864 |
| Rust with C# support and Rust SDK | 20,417,652 | 20.417652 | 19.471790 | 16,700,020 | 43,280 |

The working self-contained C# executable is 5.2131 times the emitted AOT
executable. This is not a comparison of two functioning AOT/JIT implementations.
The complete Rust payload is 1.1798 times the framework-dependent C# payload at
this baseline. Both latter totals exclude installed toolchains and retain all
published localized resources. No dependencies or functionality were removed
to obtain these numbers.

## Baseline and environment

- Source: committed local `develop`,
  `1df4d14d13530db497dc68bc7768ffd29cb993da` (`updated .gitignore`).
- Isolated branch: `codex/aot-size-experiment`, in the managed worktree
  `C:\Users\kodyb\Development\Codex\5945\dotask`. The app supplied this
  worktree; it was advanced from its initial detached revision to committed
  `develop` before the experiment. No other checkout's uncommitted work was used.
- Windows x64; .NET SDK **10.0.400**, selected by `global.json`'s `latestFeature`
  policy. Published runtime/compiler packs: **10.0.11**.
- Rust/Cargo **1.95.0**, target `x86_64-pc-windows-msvc`.
- Installed Visual Studio Professional 2026 **18.9**, x64 C++ tools present;
  MSVC toolsets **14.44.35207** and **14.51.36231**, Windows SDKs
  **10.0.22621.0** and **10.0.26100.0**. Native linking completed successfully.
- CLI references `Dotask.Library`, `Microsoft.CodeAnalysis.CSharp` **5.0.0**,
  and `YamlDotNet` **16.3.0**. The Rust host references the library and Roslyn.
- Rust uses the committed Release profile (`lto=true`, `strip=true`,
  `codegen-units=1`) and the baseline Windows packaging flag
  `RUSTFLAGS=-C target-feature=+crt-static`.

The installed user-level `Directory.Build.props` and `Directory.Build.targets`
were preserved. For `Dotask.Cli`, Release/win-x64 evaluates to:

```text
BaseOutputPath=C:\tmp\_dotnet\C\Users\kodyb\Development\Codex\5945\dotask\src\Dotask.Cli\bin\
BaseIntermediateOutputPath=C:\tmp\_dotnet\C\Users\kodyb\Development\Codex\5945\dotask\src\Dotask.Cli\obj\
PublishDir=C:\tmp\_dotnet\C\Users\kodyb\Development\Codex\5945\dotask\src\Dotask.Cli\publish\win-x64\Release\
```

Each publish was copied to an external experiment snapshot before the next
publish cleaned the policy-owned `PublishDir`. Cargo's repository configuration
evaluates to `C:\tmp\_rust\dotask\target`; this experiment used a process-local
`CARGO_TARGET_DIR=C:\tmp\_rust\dotask-aot-size-5945\target` to avoid overwriting
another chat's build. No repository or user configuration was rewritten.

## AOT blockers and diagnostics

The unmodified warning policy rejected `PublishAot=true` with `IL2026` and
`IL3050` errors in the library's reflection-based `System.Text.Json` calls.
No executable was emitted by that strict attempt.

For measurement only, a second attempt set `TreatWarningsAsErrors=false` and
`IlcTreatWarningsAsErrors=false`. It did **not** suppress warning output. The
preserved log contains 112 warning records, including repeated build/native
analysis reports: 51 `IL2026`, 55 `IL3050`, four `IL3000`, one `IL2104`, and one
`IL3053`. `YamlDotNet` produced both trimming and AOT-analysis summary warnings.
Roslyn remains compiled into this CLI; it was not removed or replaced.

`OptimizationPreference=Size` is the applicable SDK property. The actual native
compiler response file contained `--Os` and `--dehydrate`, confirming the size
setting reached the compiler. See Microsoft's
[AOT optimization reference](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/optimizing).

The emitted executable passes CLI-only help/version. Both sample discovery and
`hello` terminate with Windows exit `-1073740791` (`0xC0000409`) and:

```text
System.InvalidOperationException: Reflection-based serialization has been disabled for this application.
```

The first failing frame is `TaskGroupReader.Read`, while building the sample
catalog. `JsonSerializerIsReflectionEnabledByDefault` evaluates to `false`.
The smoke tests therefore never reach successful task compilation or execution.

There is a further directly reported incompatibility: `TargetCompiler` uses
`typeof(BuildContext).Assembly.Location` for the task's managed library reference,
and `TargetExecutor` uses that location when snapshotting the task runtime.
`IL3000` reports an empty location for bundled/native assemblies. No separate
`Dotask.Library.dll` is emitted in the AOT publish. Fixing only JSON serialization
would not establish working C# execution. This experiment did not redesign those
paths, add preservation descriptors, disable features, or claim an AOT fix.

The ordinary single-file attempt with only native-library extraction also failed
the repository warning policy on those two `IL3000` sites. Full content extraction
(`IncludeAllContentForSelfExtract=true`) preserves real managed assembly paths;
that publish passed without relaxing warnings and ran the representative task.
It creates an extraction cache, which is not included in the shipped-file total.

## Deployment companions

PDB files are optional debug symbols, excluded from all shipped-payload totals.
Native import inspection found Windows system DLLs; the Rust static-CRT binary
has no imported MSVC runtime DLL. Windows/API/UCRT prerequisites are not copied
into these artifact directories. AOT and the self-contained single-file build
need no installed .NET runtime for the CLI itself; executing C# tasks still
requires the .NET SDK. AOT is unusable for the tested task workflow.

For ordinary framework-dependent C#, keep these files beside `dotask.exe`:

| File | Bytes |
| --- | ---: |
| `dotask.dll` | 212,480 |
| `dotask.deps.json` | 6,135 |
| `dotask.runtimeconfig.json` | 342 |
| `Dotask.Library.dll` | 58,368 |
| `Microsoft.CodeAnalysis.dll` | 3,059,000 |
| `Microsoft.CodeAnalysis.CSharp.dll` | 6,839,608 |
| `YamlDotNet.dll` | 293,376 |

For Rust C# metadata/execution support, keep the following in `csharp/`:

| File | Bytes |
| --- | ---: |
| `Dotask.CSharpHost.dll` | 53,248 |
| `Dotask.CSharpHost.deps.json` | 5,574 |
| `Dotask.CSharpHost.runtimeconfig.json` | 342 |
| `Dotask.Library.dll` | 58,368 |
| `Microsoft.CodeAnalysis.dll` | 3,059,000 |
| `Microsoft.CodeAnalysis.CSharp.dll` | 6,839,608 |

Both managed publish inventories also include 26 Roslyn resource assemblies,
totalling **6,673,920 bytes**: `Microsoft.CodeAnalysis.resources.dll` and
`Microsoft.CodeAnalysis.CSharp.resources.dll` under each of `cs`, `de`, `es`,
`fr`, `it`, `ja`, `ko`, `pl`, `pt-BR`, `ru`, `tr`, `zh-Hans`, and `zh-Hant`.
They support localized diagnostics rather than basic English execution. They
were retained, counted as deployment content, and not classified as debug symbols.

Rust task execution additionally requires the baseline bundled SDK source files:
`sdk/Cargo.toml` (**262 bytes**) and `sdk/src/lib.rs` (**9,698 bytes**), plus an
installed Rust/Cargo toolchain. The framework-dependent C# host requires an
installed .NET runtime even for reading C# metadata at this baseline; the .NET
SDK satisfies that prerequisite. Native help/version and Rust metadata do not
need that host. This is a limitation of this committed baseline, regardless of
later work in other chats.

Optional symbols: AOT `dotask.pdb` **52,547,584** and `Dotask.Library.pdb`
**21,252** bytes; each working C# variant `dotask.pdb` **55,612** and
`Dotask.Library.pdb` **21,252** bytes; Rust support host
`Dotask.CSharpHost.pdb` **22,028** and `Dotask.Library.pdb` **21,252** bytes.
No native Rust PDB was emitted by the stripped Release build.

## Smoke tests and verification

| Check | Experimental AOT | Self-contained single file | Framework-dependent C# | Rust with support |
| --- | --- | --- | --- | --- |
| `--help` | Pass, 0 | Pass, 0 | Pass, 0 | Pass, 0 |
| `--version` | Pass, 0 | Pass, 0 | Pass, 0 | Pass, 0 |
| `--use-dir examples/basic/.tasks` | Crash | Pass, four targets | Pass, four targets | Pass, four targets |
| `--use-dir examples/basic/.tasks hello -n "Size experiment"` | Crash before execution | Pass, 0 | Pass, 0 | Pass, 0 |

The AOT greeting attempt used `-n "AOT experiment"` and failed before binding.
The working builds printed `Hello from dotask, Size experiment!` and
`Host: Windows; configuration: Debug`. No installation was performed.

Documentation verification: `build.cmd verify-docs` and `git diff --check`.
This document and its changelog/index links are the only repository changes;
the source verification gate is not claimed as rerun. The failed strict AOT
publish and crashing experimental smoke tests are findings, not passing checks.
Native Linux/macOS, complete AOT functionality, performance, and delivery on a
second Windows machine were not tested.

## Preserved artifacts

External experiment root: `C:\tmp\_dotnet\dotask-aot-size-5945`.

```text
aot\dotask.exe
single-file\dotask.exe
framework-dependent\dotask.exe
rust\dotask.exe
rust\csharp\...
rust\sdk\...
sizes.json
inventory.json
aot-warning-counts.json
aot-properties.json
aot-strict.log
aot-experimental.log
single-file.log                 (failed native-only extraction attempt)
single-file-extraction.log      (successful full extraction publish)
framework-dependent.log
csharp-host.log
rust-build.log                 (initial dynamic-CRT build, not the table artifact)
rust-static-build.log          (measured table artifact)
*-help.log / *-version.log / *-discovery.log / *-hello.log
*-dependents.log
```

Measured executable SHA-256 hashes:

```text
AOT:        8ED0F016EE7156D6B0D22FC813B51DABA25EA78B9F00BDF48EF8392F102040E5
SingleFile: E3BDE89C31047B572B1C960436F9EDB135C5E9F8F41DC2E04BF8B88E556E9899
Framework:  600D44786F48F4DCFECE218D49410D963E6E4915F33049E3F9C4E40AF8B8DCC9
Rust:       E7A37A94D56975F530248F3300670E7C4A2E62E0663B31E7B56AC354DAB028C8
```

## Recorded reproduction commands for the measured checkout

These historical PowerShell commands were run against `1df4d14` in the original
experiment checkout. They are not reproduction commands for current `develop`:
the newer source has different metadata and no `Dotask.CSharpHost` project.
The recorded worktree paths below refer to the disposable checkout before its
accepted removal. To reproduce the experiment in another checkout, select the
exact original baseline, substitute that checkout's path, and obtain each
project's `PublishDir` with `dotnet msbuild ... -getProperty:PublishDir`.
Publish commands use the evaluated user-policy directory; no `-o`, `PublishDir`,
or output-root override is needed. Publish replaces that directory, so copy each
result before publishing the next variant. These commands do not install or
activate dotask.

```powershell
cd C:\Users\kodyb\Development\Codex\5945\dotask
git rev-parse HEAD
git merge-base HEAD 1df4d14d13530db497dc68bc7768ffd29cb993da
$experimentRoot = 'C:\tmp\_dotnet\dotask-aot-size-5945'
$publishedRoot = 'C:\tmp\_dotnet\C\Users\kodyb\Development\Codex\5945\dotask\src\Dotask.Cli\publish\win-x64\Release'

dotnet msbuild src/Dotask.Cli/Dotask.Cli.csproj -p:Configuration=Release -p:RuntimeIdentifier=win-x64 -p:PublishAot=true -p:OptimizationPreference=Size -getProperty:BaseOutputPath,BaseIntermediateOutputPath,PublishDir,JsonSerializerIsReflectionEnabledByDefault

# Expected to fail the baseline's warning policy; emits no executable.
dotnet publish src/Dotask.Cli/Dotask.Cli.csproj -c Release -r win-x64 --self-contained true -p:PublishAot=true -p:OptimizationPreference=Size

# Measurement-only artifact: warnings remain visible; task workflow crashes.
dotnet publish src/Dotask.Cli/Dotask.Cli.csproj -c Release -r win-x64 --self-contained true -p:PublishAot=true -p:OptimizationPreference=Size -p:TreatWarningsAsErrors=false -p:IlcTreatWarningsAsErrors=false
Copy-Item -Path "$publishedRoot\*" -Destination "$experimentRoot\aot" -Recurse -Force

dotnet publish src/Dotask.Cli/Dotask.Cli.csproj -c Release -r win-x64 --self-contained true -p:PublishAot=false -p:PublishSingleFile=true -p:PublishTrimmed=false -p:IncludeNativeLibrariesForSelfExtract=true -p:IncludeAllContentForSelfExtract=true
Copy-Item -Path "$publishedRoot\*" -Destination "$experimentRoot\single-file" -Recurse -Force

dotnet publish src/Dotask.Cli/Dotask.Cli.csproj -c Release -r win-x64 --self-contained false -p:PublishAot=false -p:PublishSingleFile=false -p:PublishTrimmed=false
Copy-Item -Path "$publishedRoot\*" -Destination "$experimentRoot\framework-dependent" -Recurse -Force

$previousCargoTarget = $env:CARGO_TARGET_DIR
$previousRustFlags = $env:RUSTFLAGS
try {
  $env:CARGO_TARGET_DIR = 'C:\tmp\_rust\dotask-aot-size-5945\target'
  $env:RUSTFLAGS = '-C target-feature=+crt-static'
  cargo build --release --locked --target x86_64-pc-windows-msvc -p dotask-cli
  if ($LASTEXITCODE -ne 0) { throw 'Rust build failed.' }
  Copy-Item -LiteralPath "$env:CARGO_TARGET_DIR\x86_64-pc-windows-msvc\release\dotask.exe" -Destination "$experimentRoot\rust\dotask.exe" -Force
} finally {
  $env:CARGO_TARGET_DIR = $previousCargoTarget
  $env:RUSTFLAGS = $previousRustFlags
}

dotnet publish src/Dotask.CSharpHost/Dotask.CSharpHost.csproj -c Release --self-contained false -p:UseAppHost=false
$hostPublished = 'C:\tmp\_dotnet\C\Users\kodyb\Development\Codex\5945\dotask\src\Dotask.CSharpHost\publish\Release'
Copy-Item -Path "$hostPublished\*" -Destination "$experimentRoot\rust\csharp" -Recurse -Force
Copy-Item -LiteralPath src/dotask-sdk/Cargo.toml -Destination "$experimentRoot\rust\sdk" -Force
Copy-Item -LiteralPath src/dotask-sdk/src/lib.rs -Destination "$experimentRoot\rust\sdk\src" -Force
```

The original smoke and verification invocations were:

```powershell
cd C:\Users\kodyb\Development\Codex\5945\dotask
$experimentRoot = 'C:\tmp\_dotnet\dotask-aot-size-5945'
& "$experimentRoot\aot\dotask.exe" --help
& "$experimentRoot\aot\dotask.exe" --version
& "$experimentRoot\aot\dotask.exe" --use-dir examples/basic/.tasks
# The preceding AOT discovery command is expected to crash.

& "$experimentRoot\single-file\dotask.exe" --use-dir examples/basic/.tasks hello -n 'Size experiment'
& "$experimentRoot\framework-dependent\dotask.exe" --use-dir examples/basic/.tasks hello -n 'Size experiment'
& "$experimentRoot\rust\dotask.exe" --use-dir examples/basic/.tasks hello -n 'Size experiment'

$previousCargoTarget = $env:CARGO_TARGET_DIR
try {
  $env:CARGO_TARGET_DIR = 'C:\tmp\_rust\dotask-aot-size-5945\target'
  .\build.cmd verify-docs
  if ($LASTEXITCODE -ne 0) { throw 'Documentation verification failed.' }
} finally { $env:CARGO_TARGET_DIR = $previousCargoTarget }
git diff --check
```

## After accepted integration

The report is preserved at
`C:\Users\kodyb\Projects\dotask\docs\scratch\2026-10-05-windows-cli-size-experiment.md`.
The external experiment root and measured binaries remain at
`C:\tmp\_dotnet\dotask-aot-size-5945`; removal of the disposable worktree does
not remove them. A Git archive of `1df4d14`'s `examples/basic` is preserved as
`baseline-example.zip`, with extracted files under
`baseline-example/examples/basic`, so the old binaries can still be exercised
with matching task/configuration sources after the worktree is removed.

```powershell
cd C:\Users\kodyb\Projects\dotask
$experimentRoot = 'C:\tmp\_dotnet\dotask-aot-size-5945'
$baselineTasks = "$experimentRoot\baseline-example\examples\basic\.tasks"
& "$experimentRoot\single-file\dotask.exe" --use-dir $baselineTasks hello -n 'Size experiment'
& "$experimentRoot\framework-dependent\dotask.exe" --use-dir $baselineTasks hello -n 'Size experiment'
& "$experimentRoot\rust\dotask.exe" --use-dir $baselineTasks hello -n 'Size experiment'
# The AOT artifact's matching-baseline discovery still crashes as recorded.
& "$experimentRoot\aot\dotask.exe" --use-dir $baselineTasks
```

Local integration and worktree/branch cleanup were explicitly accepted. The app
archive tool refused because it classified this chat's checkout as primary.
Cleanup therefore uses non-force Git removal of the accepted disposable
worktree, after integrated ancestry and clean status are proven, rather than an
app archive. The report and measurements are committed on `develop`, and the
external experiment files are preserved separately. No installation, publication,
or push is part of that acceptance.

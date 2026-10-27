# Application installers

`dotask install` finds and runs an existing installer for console and GUI applications.
The shared adapters live under `_/doinstall/`, with generic launching under
`_/installer/`; they support arbitrary application payloads. Application builds
remain project-specific.
It never builds the application or invokes a creation task. Projects can use
doinstall, Inno Setup, or another installer tool, with any names for their build tasks.

Dotask and doinstall tooling are installed separately. Dotask's application
payload contains its command and SDK helpers; packaging does not download or
install doinstall tools automatically. Its installer package carries the runtime
as `installer`, which is also retained for uninstall, as with other applications.
Consumer projects supply their own doinstall runtime/builder, plus the precompiled
launcher template for self-extracting delivery. The native CLI
has no doinstall-specific behavior; optional shared tasks invoke its ordinary
command-line interface, just like adapters for other external utilities.

## Install doinstall's tools once

Use doinstall's own self-extracting installer. From its source checkout,
`build.cmd package` (Unix: `./build.sh package`) builds it and prints the file
to run. The installer creates its owned tool commands and retains its launcher
template beside the real binaries. See
[doinstall tool installation](https://github.com/kodybrown/doinstall/blob/develop/docs/INSTALLATION.md#install-doinstall-tools).

With `doinstall-builder` available on PATH, the updated shared
`_/doinstall/create-installer` needs no doinstall tool-path settings. It queries
`doinstall-builder --print-tool-paths` for real `Installer` and `LauncherTemplate`
paths, so a command shim is safe for the builder. Explicit `--builder`,
`--installer`, `--launcher-template`, and corresponding `settings.doinstall`
values override that selection. Queries fail directly on missing/invalid tools;
there is no registry, receipt, version-directory, or older-tool fallback search.
Sync the creator task before using the new query contract.

## Find and run an existing installer

Install the optional `_/installer/install` shared task, then configure a literal
project-relative or absolute file path, or a template in `.dotasks.yaml`:

```yaml
settings:
  installer:
    installer_file: './artifacts/installers/${os}-${arch}/MyApp-*/MyApp-setup-v${version}-se${exe-extension}'
    select: highest-version
```

`${os}`, `${arch}`, and `${exe-extension}` resolve to the current host (for
example, `windows`, `x64`, `.exe`). `${version}` matches a numeric dotted version
with optional prerelease/build metadata; `${hash}` matches a 7–64 character
hexadecimal revision; `${dirty}` matches an empty string or `-dirty`. Use only
the captures present in your naming scheme. Each capture may appear once.
Up to four `*` wildcards match within a single path component. Other placeholders
are errors. The file name can be `setup.exe`, an MSI, or another supported kind;
it need not be named `installer.exe`.

The only selection policy is `highest-version` (the default). Numeric version
components are compared numerically, with prereleases before the corresponding
release; build metadata does not affect precedence. A pattern without a version
must match exactly one file. Equal highest versions, including different Git
hashes or clean/dirty builds, are ambiguous and fail with the matching paths.
Neither file timestamps nor revision hashes break ties. A configured path or
template is authoritative: no matching file is an error, even if a build record
exists. Template searches are bounded to 20,000 entries and skip linked paths.

The default kind comes from the file extension (`.msi`, `.sh`, `.dll`, otherwise
executable), or from matching recorded metadata. Optional `kind` overrides it
with `executable`, `msi`, `shell-script`, or `dotnet-assembly`. Optional
`installer_args` is a YAML string array of default argument tokens. The CLI
option `--installer-args '["--quiet"]'` replaces those defaults exactly.

### Successful-build records and first-run setup

After **all** packaging steps succeed, a task can call Rust
`project.record_installer(&artifact)?` or C#
`await project.RecordInstallerAsync(artifact);`. The shared doinstall creator
does this automatically after its build/gather/archive sequence. Other tools can
record the same generic artifact. The SDK writes
`artifacts/.dotask/installers/<os>-<arch>.json`, separately from user settings,
with artifact metadata, a SHA-256 digest and completion time. Keep this local
state out of version control. Returning an invocation result alone does not
record a build; failed creation leaves the previous record intact. Record writes
are atomic and serialized per platform, and refuse unrecognized existing state.
An absent or changed recorded output is rejected when using that record.

When `installer_file` is missing, `install` prints a clear example. It first
offers a valid recorded build; otherwise it searches the project for common
`installer`/`setup` executable names and MSI files. Discovery excludes hidden,
dependency, build-output, payload, SDK and receipt-bearing installation
directories, and skips links. It stops at depth 12, 20,000 entries or 50
candidates. It does not guess which of several candidates you intended.

In a terminal, select/confirm an installer, review the suggested path/template,
then choose whether to save it. Saving adds only the missing property, preserving
comments, UTF-8 BOM, line endings and final-newline state. Unsupported YAML
layouts and concurrent edits fail with manual guidance. Legacy task-directory
configuration must be moved explicitly before creating a root configuration.
Declining to save can still launch the installer you confirmed. With no candidate,
setup offers to add an editable `YOUR-APP` placeholder and explains what to change.
Cancellation or EOF launches nothing. Without an interactive terminal, missing
configuration prints instructions/candidates and fails without edits or launch.

Discovery suggestions are a starting point; review the path before accepting it.
For predictable unattended use, configure `installer_file` explicitly. Installer
creation, last-successful-build recording and installation remain separate actions.

## Where build configuration comes from

The source `.tasks/installer.yaml` separates four responsibilities:

- `application` declares application identity and banner metadata.
- Optional `packaging` sets producer-only installer/launcher delivery filenames.
- `build-steps` runs producer tasks, gathers their expected files, assembles the
  installer directory, and optionally runs archive tasks.
- `runtime` defines destination behavior: `interactive`, `inputs`, `defaults`,
  `profiles`, `values`, `commands`, `shortcuts`, and application-owned `settings`.

The creator removes `build-steps`, supplies the resolved version/build, platform,
architecture, and payload location, and preserves the nested `runtime` mapping
in the distributed matching-name YAML. The builder consumes `packaging`; the
standalone installer reads YAML matching its executable name beside itself,
or the file selected with `--config`. It never discovers
`.dotasks.yaml`, runs build tasks, or accepts source-only `build-steps`.
Old root-level runtime fields are rejected; there is no compatibility loader.

In dotask's source project, `.dotasks.yaml` points `settings.installer.config`
to `.tasks/installer.yaml`. Top-level project name/description are not copied
automatically. Application version policy lives in `settings.app`:

```yaml
settings:
  app:
    version: '0.1.yyDDD.HHmm'
    git-hash: short           # none, short (7 characters), or full
    dirty-suffix: true
  installer:
    config: .tasks/installer.yaml
    output: artifacts/installers
    installer_file: './artifacts/installers/${os}-${arch}/example-*/example-setup-v${version}${exe-extension}'
```

Version tokens use UTC: `yyyy`, `yy`, `MM` (month), `dd` (day), `DDD` (day of
year), `HH` or `hh` (24-hour clock), and `mm` (minute). Other characters are
literal. For example, `0.1.yyMM.ddhh` retains the requested calendar shape,
and `0.1.yyDDD.HHmm` includes ordinal day and minute. An existing destination
version directory stops creation with an error; the creator never advances the
clock or overwrites an earlier build.

A source configuration can compose ordinary tasks:

```yaml
schema: 1
application:
  id: example
  name: Example App
build-steps:
  - run: build
    with:
      stage-dir: '${staging}/app'
    gather:
      - from: '${staging}/app'
        files: ['example${exe-extension}', 'sdk/**']
        to: .
  - run: _/doinstall/assemble
    with:
      builder: '${builder}'
      installer: '${installer}'
      config: '${config}'
      output: '${package}'
      result-file: '${result-file}'
  - run: _/archive/create-zip
    enabled: false
    with:
      source: '${package}'
      output: '${package}.zip'
runtime:
  interactive: true
  commands:
    - name: example
      executable: 'example${exe-extension}'
  shortcuts: []
  inputs: {}
  defaults: {}
```

Build steps run in order. A task failure stops the sequence; missing expected
files and payload collisions are errors. `gather.files` accepts exact portable
relative file paths and directory selections ending in `/**`, preserving their
structure beneath `to` (payload root by default). Producers need no output
registration: their parameters and the expected-file declarations establish the
relationship. Configure SDK file selections in `.tasks/sdks.yaml` for the reusable
`build-sdks` task; installer payload selections belong in `build-steps`.

Available build values are `${staging}`, `${payload}`, `${config}`, `${package}`,
`${version}`, `${build-info}`, `${exe-extension}`, `${result-file}`, and `${settings.NAME}`.
The doinstall creator also supplies `${builder}` (the `--builder` option,
`settings.doinstall.builder`, or `doinstall-builder` on PATH) and `${installer}`
(the `--installer` option or `settings.doinstall.installer`, resolved from the
project root, otherwise the selected builder's tool-path query).
`${launcher-template}` comes from `--launcher-template` or
`settings.doinstall.launcher-template`, also resolved from the project root,
otherwise that query when a recipe needs the build value.
Supply actual runtime/template executables, not command launcher shims.
The creator expands build placeholders inside `packaging` to literal filenames;
the native builder itself does not expand filename templates.
Assembly must pass `${result-file}` to the builder. The creator reads its
absolute `FilePath`, verifies it is inside the package, and records/returns that
artifact only after all build steps succeed. No fixed filename or fallback search
is used; failed packaging leaves the preceding successful-build record intact.
`build-info` is JSON metadata for a producer that embeds the same application
version/stamp. Runtime input placeholders, such as `${shortcut-name}`, remain for
the destination installer. Only command/shortcut executable paths resolve build
placeholders inside `runtime`.

| Installer YAML | Purpose |
| --- | --- |
| `application.name`, `description`, `copyright` | Console banner metadata |
| `runtime.shortcuts[].start-menu`, `desktop`, `local` | Permit shortcut locations |
| `runtime.defaults.*.start-menu-shortcuts`, `desktop-shortcuts`, `local-shortcuts` | Default selection of permitted shortcuts |
| `runtime.defaults.*.additional-command` | Default for a second command/shim, initially false |
| `runtime.defaults.*.add-to-path` | Default for Windows user PATH addition |
| `runtime.interactive`, `inputs`, `defaults`, `values` | Prompt mode, labels, defaults, and supplied values |

An additional-command default of false sets the initial No answer. The engine
has no separate permission flag to forbid that additional shim. Dotask declares
no shortcuts, so it offers no Start Menu or desktop shortcut questions.

## Shared tasks

Install the optional adapters using the tool:

```sh
dotask --add "_/doinstall/*" "_/installer/install"
```

Review conflicts rather than deleting locally modified tasks. Remove obsolete
tracked installer tasks explicitly so short names do not become ambiguous.
An exact project `install` can wrap `_/installer/install`. Updating the CLI
alone does not update committed task copies. Until published, use the
[local catalog override](SHARED-TASKS.md#official-catalog-and-unpublished-preview).

`_/doinstall/assemble.task` invokes a prebuilt builder directly, with no C# or
Rust task compilation. For an already-resolved configuration and payload:

```powershell
dotask --add "_/doinstall/assemble.task"
New-Item -ItemType Directory -Force artifacts | Out-Null
dotask doinstall/assemble --installer C:/Tools/doinstall/doinstall.exe --config .tasks/resolved-installer.yaml --output artifacts/example-installer --result-file artifacts/example-result.json
```

The builder defaults to `doinstall-builder` on PATH; `--builder` accepts an
explicit executable path. `--installer` must point to the matching actual
runtime executable because a PATH command may be a launcher shim. The builder
must run on the host; the runtime must match the YAML's target OS/architecture.
The adapter takes resolved doinstall YAML,
not dotask's source-only `build-steps`, and does not return an `InstallerArtifact`.
Missing tools and builder failures stop execution; no fallback build or download
occurs. Both assembly tasks require `--result-file`, outside the package with
an existing parent. They write the builder's JSON result, not an invocation-local
SDK result. Configure recurring paths in the selected target's `defaults`.
When updating existing task copies, add `result-file: '${result-file}'` to each
creator assembly step and supply `--result-file FILE` on direct assembly calls.
Sync updates tracked tasks; explicitly add the new self-extracting task because
sync does not install newly published sibling tasks.

For a single self-extracting launcher, use the separate declarative task:

```powershell
dotask --add "_/doinstall/assemble-self-extracting.task"
New-Item -ItemType Directory -Force artifacts | Out-Null
dotask doinstall/assemble-self-extracting --builder C:/Tools/doinstall/doinstall-builder.exe --installer C:/Tools/doinstall/doinstall.exe --launcher-template C:/Tools/doinstall/doinstall-launcher.exe --config .tasks/resolved-installer.yaml --output artifacts/example-se --result-file artifacts/example-se-result.json
$launcher = (Get-Content artifacts/example-se-result.json -Raw | ConvertFrom-Json).FilePath
& $launcher --validate
```

Neither assembly task needs Rust/Cargo, .NET, or an archiver. Supply a prebuilt
launcher template matching the runtime's target OS/architecture. The output
directory contains only the finished launcher; its ZIP contains the installer,
matching YAML, and payload. Validation extracts and retains sources without
installing the application. Use the returned `FilePath` rather than guessing it.

Replace old tracked `assemble.rs` copies explicitly with `--remove` followed by
`--add`; shared sync does not silently change their implementation. Review local
edits first. The build/gather creator is a native helper with a declarative
adapter. The source-build helper, retained-uninstaller adapter and generic install
launcher remain Rust tasks and require Rust/Cargo when executed.

The shared creator accepts `installer.config` (`--config`) and
`installer.output` (`--output`), with optional `--app-version` and `--build-stamp`
overrides. Paths are project-relative or absolute. A project task can call the
shared creator and forward its `InstallerArtifact`; its name is unrestricted.
Creation records successful output for guided setup but never launches it.

Packages live beneath
`<output>/<os>-<architecture>/<app-id>-<version>[-<revision>][-dirty]/` and contain:

```text
example-setup-v1.0.0.exe   # omit .exe on Linux/macOS
example-setup-v1.0.0.yaml  # resolved application and nested runtime settings
payload/
```

Distribute the entire directory. It contains one installer executable without a
companion language runtime. The destination does not need dotask, Rust, or an SDK
to install; application runtime requirements depend on the payload. Build-machine
assembly belongs to the separate `doinstall-builder` executable, which is
not distributed. The shipped installer has no `package` command.

ZIP and 7z creation are ordinary optional `build-steps` using
`_/archive/create-zip.task` and `_/archive/create-7z.task`. Each uses a configurable `7z`
executable and includes the installer directory as the archive's top-level
folder. Enable one or both, or substitute a project task for another format.
Archive flags do not belong to the destination installer.
These tasks need no Rust/.NET compiler. Outputs must be new and outside the
source directory; failed commands leave no final archive. Staged publication
refuses concurrent destination creation. Replace old tracked `.rs` copies
explicitly using shared-task remove/add after reviewing local edits.

Supply a host-native builder and target-native runtime/template. The builder
can assemble another target's package without executing it; the build/gather
creator still selects the current host OS/architecture. Cross-platform application
builds remain project-owned. Native acceptance is recorded separately in
[VERIFICATION.md](VERIFICATION.md).

### Self-extracting build-and-gather workflow

An AI creating a project installer should first read the project's tasks and
settings, select separately supplied prebuilt tools, then choose resolved-YAML
assembly above or the precompiled build/gather creator below. Do not put build steps
in native doinstall YAML or add SDK/runtime requirements to the launcher.

Add `_/doinstall/create-installer.task` and
`_/doinstall/assemble-self-extracting.task` with `dotask --add`, then configure:

```yaml
# .dotasks.yaml
version: 1
settings:
  app:
    version: '1.2.0'
    git-hash: none
    dirty-suffix: false
  installer:
    config: .tasks/installer.yaml
    output: artifacts/installers
    installer_file: 'artifacts/installers/${os}-${arch}/example-*/example-setup-v${version}-se${exe-extension}'
```

```yaml
# .tasks/installer.yaml; published/ already contains the application's build.
schema: 1
application: {id: example, name: Example App}
packaging:
  installer-name: 'example-setup-v${version}${exe-extension}'
  launcher-name: 'example-setup-v${version}-se${exe-extension}'
build-steps:
  - gather:
      - from: published
        files: ['example${exe-extension}']
  - run: _/doinstall/assemble-self-extracting
    with:
      builder: '${builder}'
      installer: '${installer}'
      launcher-template: '${launcher-template}'
      config: '${config}'
      output: '${package}'
      result-file: '${result-file}'
runtime:
  interactive: true
  commands:
    - {name: example, executable: 'example${exe-extension}'}
  defaults:
    windows: {add-to-path: false}
```

Run `dotask _/doinstall/create-installer` to create and record the artifact without
installing it. Then use `dotask install` after adding `_/installer/install` when
installation is intended. The creator is a declarative adapter to the native
dotask-doinstall-create-installer executable shipped beside dotask. With a
prebuilt payload and declarative build steps, creation requires neither Rust/Cargo
nor .NET. Application build steps still require their own selected toolchains.
Replace the gather step with application build/publish steps as needed.
The output directory contains `example-setup-v1.2.0-se.exe` on Windows.
Omit `packaging` to use doinstall's defaults. Custom names are literal after
the creator expands them, must be portable filenames, and need `.exe` on Windows.
The inner YAML follows the installer name; installed recovery files retain
their internal `installer[.exe]` and `installer.yaml` names.

The launcher shares stdin/stdout/stderr with the installer, retains extracted
sources, and uses a fresh OS temp directory for every execution. To choose a
directory, run the returned artifact with `--extract-to NEW_OR_EMPTY_PATH`.
Windows waits and returns the installer status; Unix replaces the launcher
process. Sign the completed launcher after packaging; macOS targets assembled
elsewhere require final signing on macOS. See doinstall's
[authoritative self-extracting contract](https://github.com/kodybrown/doinstall/blob/develop/docs/INSTALLATION.md#self-extracting-launchers).

### Native creator distribution and task replacement

Keep dotask-doinstall-create-installer[.exe] beside the actual dotask executable.
Portable archives and installers ship the matching helper on every release
platform (Windows x64 and Linux x64). Source builds also stage the helper on the
current host. Resolution uses the running executable's directory, independently
of CWD, PATH, command shims, and SDK overrides. A missing helper fails with repair
guidance; there is no download or source-compilation fallback.

The adapter uses the same immutable task context as a code task: project root,
invocation directory, target settings/defaults, parameters, call chain and session.
Nested build steps resolve their own target settings and preserve failure codes
and cancellation. The helper returns InstallerArtifact through the existing
invocation-local result channel and records the actual builder FilePath only
after every build/gather/packaging step succeeds. The separately installed
doinstall builder/runtime/launcher remain independent inputs; the helper neither
builds nor downloads them.

For a previously tracked Rust creator, review local edits, then explicitly
replace the managed implementation:

    dotask --remove _/doinstall/create-installer
    dotask --add _/doinstall/create-installer.task

Remove refuses edited managed copies; preserve and reconcile those edits before
replacement. Do not keep both implementations or add a compilation fallback.
Sync preserves implementation ownership and does not silently migrate a tracked
Rust entry to YAML. Extensionless target calls and existing settings remain valid;
explicit .rs calls must change to the extensionless name or .task.

## Installer runtime configuration

Doinstall is an independent project. Its authoritative YAML, prompts, shortcuts,
ownership, receipt, retention, and uninstall contract lives in
[the doinstall repository](https://github.com/kodybrown/doinstall/blob/main/docs/INSTALLATION.md).
Dotask keeps generic artifact launching and shared tool adapters; it has no
installer runtime, builder, shim implementation, or bundled shim assets.

The project can use prebuilt doinstall and doinstall-builder tools with the
`_/doinstall/assemble` wrapper's explicit paths. For local development, set
`settings.doinstall.source` to an external checkout and explicitly run
`_/doinstall/build`. That wrapper delegates to the external project's Cargo
helper and can stage the runtime, builder, launcher template, and optional server
helper. It does not add the external package to dotask's
Cargo workspace. No source download or tool installation happens implicitly.
Consumers do not need `settings.doinstall.source`. Keep the builder/runtime pair
from the same doinstall build; this adapter requires its schema-1 resolved YAML
and `--installer`, `--config`, `--output` interface. Doinstall owns that interface;
dotask owns the optional task files and their add/sync lifecycle.

## Build and install dotask itself

For the complete checkout-local workflow, run:

```powershell
cd C:\Users\kodyb\Projects\dotask
.\build.cmd create-dotask-installer
```

Unix uses `./build.sh create-dotask-installer`. Supply a matching native prebuilt
bundle with `--doinstall-tools PATH` or `DOINSTALL_TOOL_DIR`. Otherwise the task
asks `doinstall-builder` on PATH for its real adjacent tools. Nothing downloads
or builds doinstall implicitly. To develop against source explicitly, use
`--source ../doinstall`; that mode builds/stages the checkout and runs its
independent verification before dotask's build, ignoring ambient prebuilt tools.
Do not combine `--source` and `--doinstall-tools`. This task is local to dotask,
not a shared consumer task.

The task builds/lints/stages dotask, then assembles and tests both a portable
archive and self-extracting installer. Run `test` beforehand for the complete
repository gate; `--reuse-build` packages that exact inspected executable without
rebuilding it. It records/returns the installer only after native acceptance,
using temporary app/bin/cache locations with source SDK overrides cleared and
persistent PATH unchanged. No active installation is changed.

Version comes from `src/dotask-cli/Cargo.toml`; build time is display metadata.
Only explicit `update-version` changes the package version, patch by default.
`--increment-minor`, `--increment-major`, and `--dry-run` are available there.
SDK versions remain independent. See [native releases](RELEASING.md) for asset
names, pinned published doinstall tools, main-push publication, and manual builds.

The release driver packages the inspected app, complete SDKs, license, dependency
notices and provenance. The source-only `.tasks/installer.yaml` supplies runtime
configuration; its generic build/gather recipe remains available to the shared
creator. Dotask's release driver supplies a curated identical payload to both
forms and follows the builder's result `FilePath`. Doinstall tools are separate
inputs and never become dotask application payload. There is no local `pack`
task; `_/dotnet/pack` remains a NuGet task for consumers.

`build-sdks` publishes the maintained C# helper using evaluated MSBuild
`PublishDir`, then copies the Rust SDK selections from `.tasks/sdks.yaml`.
Compiler output stays external. Local installers default to a fresh directory
under Cargo's evaluated `target_directory/installers`; optional `--output PATH`
selects a fresh package parent. Repeating packaging does not increment the
version or overwrite previous output. Destination machines need
Rust/Cargo to execute Rust tasks and the .NET SDK to execute C# tasks; neither is
needed for metadata, help, initialization, completion, or installer execution.
No language runtime, SDK, Roslyn, or managed support host is bundled.

```powershell
.\build.cmd                         # Bootstrap and list available tasks.
$env:DOINSTALL_TOOL_DIR = 'C:/Tools/doinstall' # Prebuilt runtime, builder, and launcher template.
.\build.cmd test                    # Complete gate, including external adapters.
.\build.cmd verify                  # Extra checks; requires a prior inspected build.
.\build.cmd create-dotask-installer --reuse-build # Package the inspected build and test both deliveries.
# Or invoke the optional shared adapter directly with a prebuilt pair:
.\build.cmd _/doinstall/create-installer --builder "$env:DOINSTALL_TOOL_DIR/doinstall-builder.exe" --installer "$env:DOINSTALL_TOOL_DIR/doinstall.exe"
# Use fresh, isolated locations for acceptance:
# Run the recorded FilePath with --non-interactive --set add-to-path=false
# --install-dir C:/Temp/dotask-acceptance/apps/dotask --bin-dir C:/Temp/dotask-acceptance/bin
```

After creation, `build.cmd install` finds the self-extracting launcher through
the SDK's separate successful-build record, including its external FilePath.
It does not need doinstall build tools or source.
Unix uses actual tool paths without `.exe`.
`test`, `verify` and `test-sdks` accept `--doinstall-tools PATH`, which takes precedence
over `DOINSTALL_TOOL_DIR`. They require prebuilt runtime, builder, and launcher
template tools and fail when one is
absent. If developing against source, explicitly run
`build.cmd _/doinstall/build --stage-dir C:/Tools/doinstall` first. Release CI uses
verified published Windows/Linux bundles. macOS has no CI job or release gate.

Unix uses `./build.sh`. Both launchers forward arbitrary arguments to a temporary
native runner; no task whitelist is maintained. The Rust source SDK is available
to bootstrapped Rust tasks. The launcher prepares the C# helper through
`build-sdks` only when a C# task is executed. `build` calls `build-sdks` explicitly
for a distributable native app. Doinstall owns installer Rust tests, formatting,
clippy, and shim verification. Rust 1.95+, rustfmt, and clippy are required for
source verification. See [verification status](VERIFICATION.md) for platform evidence.

## Installer library contract

`InstallerArtifact` contains an absolute `FilePath`, `Kind`, `OS`, `Architecture`,
and optional `DefaultArguments`. `BuildContext.SetInstallerResultAsync` resolves
relative artifact paths against the project root. The standalone
`InstallerRunner` requires an absolute path. The artifact must exist and match
the host OS and OS architecture; unknown kinds and invalid arguments fail.

| Kind | Launch behavior |
| --- | --- |
| `Executable` | Execute the artifact; Windows requires `.exe`, Unix requires execute permissions |
| `Msi` | Windows system `msiexec.exe /i <artifact>`; `.msi` required |
| `ShellScript` | Linux/macOS `/bin/sh <artifact>`; script must be POSIX sh compatible |
| `DotNetAssembly` | `dotnet <artifact>`; suitable runtime must be available |

Package formats without a built-in kind require a project-authored installer
wrapper. There is no universal package generator, privilege escalation, desktop
shortcut generation, or shell command inference in the shared task.

- `CreateInstallerAsync(target = "create-installer", parameters = null, cancellationToken)`
  builds and returns the artifact using normal target resolution and parameter binding.
- `SetInstallerResultAsync(artifact, cancellationToken)` returns one artifact.
- `RecordInstallerAsync(artifact, cancellationToken)` explicitly records a
  successful build for later discovery; it never edits `.dotasks.yaml`.
- `RunInstallerAsync(artifact, arguments = null, cancellationToken)` runs it;
  null uses defaults, while an explicit list replaces them.
- `InstallerRunner.ParseArguments(json)` decodes argument tokens for task options.
- `InstallerRunner.RunAsync` also works outside an ambient task context.

Rust equivalents are `create_installer`, `set_installer_result`,
`record_installer`, and `run_installer`. `find_installer()` is read-only: resolve
the configured path/template, or return a validated successful-build record when
no setting exists. `install(arguments)` adds the shared task's terminal setup
flow and launching. C# tasks can record outputs for that same shared task;
guided setup and template selection are implemented in the Rust task SDK.

Launches use the artifact’s parent as the working directory and wait for the
launched process. Windows executable installers use shell activation, honoring
the installer’s elevation manifest without forcing `runas`; UAC cancellation is a
launch failure. Other kinds inherit the environment and standard streams. Nonzero exit codes
propagate as failures, including installer cancellation. MSI 1641 and 3010 are
successful restart-required outcomes and are reported explicitly. Task cancellation
attempts to stop the process tree; an elevated Windows installer may remain
running if access is denied, which is reported. Installer rollback is the
installer’s responsibility.
The shared task reports the observed exit code, not independently verified
installation success. An installer that detaches must supply a wrapper which
waits for completion if that guarantee is needed.

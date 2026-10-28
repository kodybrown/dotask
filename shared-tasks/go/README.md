# Native Go installer tasks

Refresh with `dotask --save go/create-installer.go`, then install with
`dotask --add go/create-installer.go`. Its dependencies include
`go/stage-installer.go` and the native `doinstall` creator/assembly adapters.
Use Go 1.22+, prebuilt dotask with `sdk/go` and its companion
`dotask-doinstall-create-installer`, and matching prebuilt doinstall tools.
Creation uses no Rust/Cargo or .NET. It never installs the result, increments
versions, or downloads a Go toolchain. The project's own Go dependencies may
use Go's normal external module cache/proxy; module files are readonly.

Configure the native application in `.dotasks.yaml`, for example:

```yaml
version: 1
settings:
  go:
    package: ./cmd/myapp
    executable: myapp.exe
    version-file: cmd/myapp/version.go
    version-symbol: buildVersionStr
    version-output: 'myapp v{version}'
    installer-files: [README.md, LICENSE.md]
    clean-environment-prefixes: [MYAPP_]
  installer:
    config: .tasks/installer.yaml
    output: artifacts/installers
```

`version-symbol` must occur exactly once as a top-level const/var string literal
in `version-file`. A leading `v` is removed for installer metadata. Parsing uses
the Go syntax tree, supports raw/escaped strings and comments, and never executes
or rewrites version source. `version-output` contains `{version}`; its expansion
must equal the executable's trimmed stdout. `version-arguments` defaults to
`[--version]`; an explicit empty array supplies no arguments.

`executable` is a plain native filename (`.exe` on Windows). `installer-files`
contains explicit portable project-relative file paths, copied at the same paths.
No recursive globs, links, directories, traversal or duplicate destinations are
accepted. An omitted/empty inventory distributes only the executable. Configure
`clean-environment-prefixes` to remove site/application settings from the version
smoke check. Tests/vet/build keep the project root as their working directory.

The shared tasks build for the current OS on x64/arm64. They test and vet `./...`,
then build `go.package` with readonly modules, trimpath, no VCS stamp and Cgo off.
Inherited Go controls are cleared, and `GOTOOLCHAIN=local` prevents downloads.
A newer compiler required by the application's module must already be installed.
The fresh stage must be outside source; its parent must exist. Failed staging
remains available through the creator's diagnostics. Executables containing
checkout/home paths or failing the configured version check are refused.

In the source-only installer YAML, use these build steps, plus your normal
application/runtime schema:

```yaml
build-steps:
  - run: _/go/stage-installer.go
    with:
      stage-dir: '${staging}/app'
      app-version: '${version}'
    gather:
      - from: '${staging}/app'
        files: [myapp.exe, README.md, LICENSE.md]
  - run: _/doinstall/assemble-self-extracting.task
    with:
      builder: '${builder}'
      installer: '${installer}'
      launcher-template: '${launcher-template}'
      config: '${config}'
      output: '${package}'
      result-file: '${result-file}'
```

Run `dotask _/go/create-installer.go`, or call it with the Go SDK's
`CreateInstaller` from a thin project task and return `SetInstallerResult`.
It accepts `output`, `builder`, `installer` and `launcher-template` overrides.
The native helper discovers builder companion tools, preserves nested context,
uses the builder's actual artifact path and records only fully successful builds.
Keep `gather.files` aligned with your exact configured payload inventory.
See [installer authoring](../../docs/INSTALLATION.md#self-extracting-build-and-gather-workflow)
and the [Go SDK contract](../../docs/TARGETS.md#go-tasks).

To preview unpublished shared changes locally, set `DOTASK_ONLINE_TASKS` to the
absolute dotask `shared-tasks` directory before `--add`. Replace a previously
tracked Rust creator with `--remove _/doinstall/create-installer` before adding
the Go group; never edit its managed files or tracking hashes manually.

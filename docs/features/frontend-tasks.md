# npm and Angular tasks

Install these shared tasks with `dotask --save npm/check npm/install angular/check`
and `dotask --add npm/check npm/install angular/check`. Commit their managed copies
and `.dotasks-lock.yaml`; extend canonical sources and sync consumers rather than
editing installed copies. These tasks use native YAML actions and need a dotask
CLI supporting `{tool: ...}` executable references. No JavaScript, PowerShell, or
shell support files are required.

Configure an Angular workspace independently of the .NET solution:

```yaml
version: 1
settings:
  solution: Example.slnx
  angular: web
```

`dotask npm/check` checks Node.js and npm and prints their versions. It does not
install either runtime or any project packages. `dotask angular/check` reads the
configured `angular.json` and invokes `node node_modules/@angular/cli/bin/ng.js
version` from that workspace. It accepts JSON-comment workspaces, uses the installed
project-local CLI, and never invokes global ng or npx downloads. Angular supplies
its compatibility diagnostics; this task does not implement a separate version
matrix or promise strict validation of all package peer-dependency ranges.
Missing workspace files/dependencies and nonzero commands fail the check.

The checks are independent: Angular does not require a particular package manager.
A project `check.task` can call `_/dotnet/check.cs`, `_/npm/check.task`, and
`_/angular/check.task` in sequence. Checking runs probes and exits without installing
dependencies or starting application services.

`dotask npm/install --directory web` runs `npm install` in the selected directory,
resolved from the project root even when invoked from a subdirectory. It can modify
the lockfile and run the package's lifecycle scripts. It installs project packages,
not Node.js/npm, global tools, a global Angular CLI, or application services.
Use a project-owned `install-dependencies.task` to compose it with local .NET tool
restore and `_/dotnet/restore` for the configured solution. The SDK already provides
`dotnet format`; restore only additional tools declared in the project's manifest.

Use `.task` actions first. Review general missing declarative capabilities before
introducing support scripts; task-relative helper support is unnecessary for these
checks and has not been added.

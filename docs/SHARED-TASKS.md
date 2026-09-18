# Shared tasks

Shared tasks are **copied into your project and committed**. Running a task,
showing project help, and completing a target name require no online access,
shared-task cache, or private original. Normal .NET SDK, package, and tool
requirements still apply when compiling/executing C# tasks.

## Locations and terminology

| Name          | Purpose and default Linux location                                                                     |
| ------------- | ------------------------------------------------------------------------------------------------------ |
| Online cache  | The official published catalog and source files in `kodybrown/dotask`, under `shared-tasks/` on `main` |
| Local cache   | Disposable downloaded copies and original snapshots under `~/.cache/dotask/tasks/<source>/`            |
| Private tasks | User-owned originals under `~/.config/dotask/private-tasks/<group>/<task>.cs`; never disposable        |
| Project tasks | Committed copies under `.tasks/<source>/<group>/<task>.cs`, alongside handwritten project tasks        |

Linux honors `XDG_CACHE_HOME` and `XDG_CONFIG_HOME`. Windows uses Local Application
Data for the local cache and roaming Application Data for private tasks. macOS
uses `~/Library/Caches/dotask/tasks` for the local cache and
`~/.config/dotask/private-tasks` for private tasks (or `XDG_CONFIG_HOME`).
`DOTASK_CACHE_HOME` overrides the entire shared-task cache directory;
`DOTASK_PRIVATE_TASKS` overrides the private task source directory.
Neither directory is the compiler cache, which remains separate.

The first version supports **dotask-official** and **private-tasks**. Named third-party
GitHub repositories, authenticated private repositories, other task languages,
and CLI self-update are future features. Reserving a source component in IDs and
paths does not mean arbitrary remote sources are already supported.

## Add selected tasks

```sh
dotask --list "dotnet/*"
dotask --save dotnet/build
dotask --add "dotnet/{build,run,format}"
```

`--list` reads the catalog, without downloading every task. `--save` downloads
selected tasks into the local cache without modifying a project. `--add` uses
the cached catalog when available, fetches missing files, verifies SHA-256, and
copies the selected tasks into the project. Required dependencies and declared
support files are included. Optional calls do not cause automatic installation.
Run `--save` to refresh cached selections before adding, or `--sync` afterward to
check for newer versions. A previously downloaded selection can be added offline.

Omitting the source means `dotask-official`; `dotnet/build` and
`dotask-official/dotnet/build` select the same shared task. Multiple arguments,
quoted brace selections, and quoted group wildcards are supported:

```sh
dotask --add dotnet/build dotnet/run dotnet/format
dotask --add "dotnet/{build,run,format}"
dotask --add "dotnet/*"
dotask --save "dotnet/*"
```

Prefer individual selections. A wildcard selects tasks **at add time**. Sync only
updates recorded tasks; new tasks published later require another add operation.
Quote braces/wildcards so DoTask receives them intact in Bash, PowerShell, and
other shells. This is a small selection syntax, not shell expression evaluation.

Adding creates `.tasks` and a minimal `.dotasks.yaml` if needed. From an existing
project or subdirectory it uses normal project discovery. For a new project run
`--add` from its intended root. `--use-dir` selects a custom project task directory;
shared-task management requires that directory to be inside the project root.
The lock file records its location and rejects being reused with another directory.

A typical project becomes:

```text
.dotasks.yaml
.dotasks-lock.yaml
.tasks/
  build.cs                              Handwritten orchestrator, optional
  dotask-official/
    dotnet/build.cs                     Downloaded build task
    dotnet/restore.cs                   Required by the build task
    dotnet/run.cs
    dotnet/format.cs
```

Commit the configuration, lock file, and task source/support files. The generated
`.tasks/.dotask/` directory contains transaction state and ignores itself in Git;
it is excluded from discovery. Do not place handwritten files there.

## Names and calls between tasks

Every source has its own destination directory. Shared-task identities include
the source, group, and task. A basename or `group/task` shortcut works only when
unambiguous; an exact project-relative name takes precedence. There is no
`default-group`, language detection, or `imports` registration section.

**Use fully qualified names, including the source, whenever one shared task calls
another.** Apply this to all three target-call helpers:

```csharp
await project.ExecTargetAsync("dotask-official/dotnet/build");
bool present = await project.TargetExistsAsync("dotask-official/dotnet/test");
var result = await project.ExecTargetIfExistsAsync("dotask-official/dotnet/test");
```

Handle a failed optional result; `NotFound` is the only skippable absence.
Fully qualified calls remain stable if another source provides the same group/task.
Shortcuts remain convenient at the terminal. A top-level `.tasks/build.cs` can
orchestrate multiple explicitly named tasks without a preferred group.

Two installed sources may both have `text/fixeol.cs` without a file collision.
`text/fixeol` and `fixeol` then become ambiguous unless an exact project task owns
that name. Existing handwritten files at **any** destination remain protected;
source directories do not authorize overwriting them.

## Private tasks

Create an ordinary self-describing C# task at:

```text
~/.config/dotask/private-tasks/my-tasks/sortini.cs
```

Then use:

```sh
dotask --list "private-tasks/*"
dotask --add private-tasks/my-tasks/sortini
```

The project receives `.tasks/private-tasks/my-tasks/sortini.cs`. Both sources use
the same source/group/task structure. Edit the private original to maintain your
personal reusable task, then sync each consuming project. DoTask never uploads,
overwrites, removes, or cleans up private originals. Private task snapshots are
retained separately in the disposable local cache for comparison.

Declare required support files and other tasks **within the same source** in the
entry-point class's XML documentation (or on its static `Main`, if the class has
no XML documentation):

```csharp
#:include _support/Helpers.cs
using DoTask;

/// <summary>Run my reusable task.</summary>
/// <requires task="my-tasks/check" />
/// <requires file="my-tasks/_support/Helpers.cs" />
public static class Target
{
  public static async Task Main()
  {
    await BuildContext.Current.ExecTargetAsync("private-tasks/my-tasks/check");
  }
}
```

Use one `<requires>` element per task or file. Task IDs are `group/task` without
the source name or `.cs`; file paths are relative to the shared source root.
For example, `dotask-official/dotnet/build.cs` declares
`<requires task="dotnet/restore" />`, while its C# code calls the fully qualified
`dotask-official/dotnet/restore`. These declarations tell add/sync which companions
to copy. They appear in help but do not execute tasks, check for companions at
runtime, or download anything during help or execution. Calls and their order
remain explicit in `Main`; do not declare optional task calls as required dependencies.

The entry-point `.cs` file is always included. Put support C# inside an
underscore-prefixed directory such as
`_support` so discovery does not turn it into another target. Include it with a
task-file-relative `#:include` directive in the entry point; the XML declaration
controls copying, while `#:include` controls compilation. Ordinary assets must
also be explicitly declared. Absolute paths, traversal, symlinks/reparse points,
case collisions, conflicting file definitions, and filenames outside the supported portable subset are
rejected by shared-task management. Asset path components use ASCII letters, digits,
spaces, dots, underscores, and hyphens, without trailing spaces/dots or reserved
Windows device names. Required dependencies are explicit metadata;
DoTask does not infer them by analyzing arbitrary C# calls.

Separate `.task.json` files are no longer used. For existing private tasks, move
each `requires` entry to `<requires task="..." />` and each `files` entry to
`<requires file="..." />`, preserving their source-relative paths, then remove
the sidecar. Catalog generation and private-task management report an actionable
error if a sidecar remains, rather than silently ignoring its dependencies.
DoTask does not rewrite private originals for you. Existing committed project
copies and lock files remain usable; normal sync protections still apply.

## Update and review changes

```sh
dotask --sync --dry-run
dotask --sync
dotask --sync dotask-official/dotnet/build
```

Sync refreshes the relevant source catalogs and updates the **current project's
installed tasks**. Other projects retain their committed copies until synced.
Consider syncing periodically and reviewing the resulting Git diff before
committing. Task files are executable code; downloading them does not run them.

The lock file records each source, content revision, required tasks, project paths,
and original SHA-256 hashes. Hashes cover exact bytes, including line endings and
encoding. Keep shared source files at consistent Git line endings (normally LF)
to avoid checkout conversion appearing as edits. A private source does not need
Git; its content hashes identify its snapshot.

| Project state                              | Sync behavior                                        |
| ------------------------------------------ | ---------------------------------------------------- |
| Unchanged installed copy, newer source     | Update the files and their tracking together         |
| Local edits, unchanged source revision     | Keep the local edits and original baseline           |
| Local edits and newer source               | Refuse the update and prepare comparison files       |
| Project already equals the incoming source | Record the incoming baseline without rewriting it    |
| Tracked file locally deleted               | Report it; do not silently restore the deletion      |
| Task removed upstream                      | Report it and preserve the project copy and tracking |
| Missing/invalid tracking                   | Never infer ownership or adopt existing files        |

All planned changes are checked before project mutation. A conflict blocks the
whole batch. `--add` also refuses to overwrite/adopt an untracked destination,
even if its bytes match the download. Re-adding an untouched installed task reports
it as already installed; it does not silently upgrade it. Modifying an installed
task never updates its baseline automatically.

On conflict, comparison files are placed in the local cache's `.conflicts/` tree,
outside project task discovery. They contain `.base`, `.project`, and `.incoming`
versions when available. If the local cache was cleared, the original `.base`
may be unavailable; hashes still protect project files, and DoTask never invents
a merge base. Sources/credentials are not required just to run committed tasks.

To keep local edits, leave the project files and tracking unchanged. To accept the
incoming source, copy its reviewed contents into the project and rerun sync.
For a manual merge that retains local modifications:

```sh
dotask --sync private-tasks/my-tasks/sortini --accept-merge
```

Run this **only after reviewing and merging the incoming revision**. It preserves
the project file contents and records the new upstream baseline. Remaining local
edits stay protected. It requires explicit task IDs, without wildcards, and an
unchanged support-file set and a previously prepared comparison for this exact
incoming revision. If upstream changed again, DoTask prepares a new comparison
and refuses to acknowledge it until reviewed. A file-set conflict needs individual
manual review.
There is no automatic merge or blanket force-overwrite option. Conflicts return
exit 1 and do not wait for terminal input, so agents and CI cannot hang.

`--dry-run` performs discovery/download/validation and may populate the local
cache and comparison files, but does not change project files or tracking.

## Remove shared tasks

```sh
dotask --remove dotask-official/dotnet/run --dry-run
dotask --remove dotask-official/dotnet/run
```

Removal only handles recorded shared tasks. Every affected file must match the
**originally installed source version**, even if upstream has since changed.
It works without the online source, private originals, or downloaded snapshots.
Modified, missing, or untracked files are not removed. Required dependencies
cannot be removed while their callers remain; select both if that is intended.
Support files still owned by another installed task are retained. Other files
in the directory are never recursively removed.

## Interrupted updates

Project updates use an exclusive management lease and a durable rollback journal
under the task directory's `.dotask/` folder. An ordinary failure rolls back files
and tracking. On the next management operation, an interrupted update is rolled
back before new work begins. Task execution refuses an incomplete journal.

If files changed again after the interruption, recovery refuses to overwrite
those edits and reports the retained backup directory for manual inspection.
Preserve those backups. `--dry-run` reports pending recovery without performing it.
Deleting state to bypass a recovery error can discard the only original copies.

## Official catalog and unpublished preview

The client uses:

```text
https://raw.githubusercontent.com/kodybrown/dotask/main/shared-tasks/catalog.json
```

This implementation prepares the catalog locally; it does **not** publish the
repository. Until the catalog is published, online commands report that it is
unavailable. Private tasks work immediately. To test the official catalog from a
source checkout, set `DOTASK_ONLINE_TASKS` to its absolute `shared-tasks` directory.
Replace `/path/to/dotask` below with your checkout location:

```sh
# Bash; run from the consuming project directory.
DOTASK_ONLINE_TASKS="/path/to/dotask/shared-tasks" dotask --list "dotnet/*"
DOTASK_ONLINE_TASKS="/path/to/dotask/shared-tasks" dotask --add "dotnet/{build,run,format}"
```

In PowerShell, set `$env:DOTASK_ONLINE_TASKS` to the corresponding absolute local
path first. This override is a local publisher/preview fixture, not configuration
for arbitrary remote sources. It must contain `catalog.json` and its declared files.

Maintainers regenerate the small static catalog without compiling/executing the
tasks being indexed:

```sh
./build.sh catalog
./build.sh catalog --verify
./build.sh
```

On Windows use `.\build.cmd catalog` and `.\build.cmd catalog --verify`.
The launcher bootstraps dotask from this checkout; the generator is a C# task
under `.tasks`. No installed dotask or Python is required. `--verify` fails for
a stale catalog without rewriting it.

Run generation after the final edits/formatting whenever shared tasks or declared
support files are added, changed, renamed, or removed, including metadata and
comment edits: hashes cover the exact source bytes. Review and commit the generated
`shared-tasks/catalog.json` alongside those changes. The full repository gate and
CI check catalog freshness; they do not silently regenerate it. Repeating
generation with unchanged inputs produces identical output.

Descriptions, support files, and required same-source task IDs come from the
task's XML documentation, using the same parser as help and private tasks.
All catalog-generation methods live in `.tasks/catalog.cs`. The task includes
the CLI parser's C# sources and uses the same
Roslyn package as the CLI; it never compiles or executes the tasks being indexed.
The generated `catalog.json` remains the downloadable index, not a file authors
maintain per task. It contains portable IDs, `runtime: csharp`, entry points,
file paths, hashes, and requirements. Unsupported runtimes are rejected. Only
selected task files and required companions are downloaded; DoTask never clones
the entire repository. Downloads are verified before installation; mismatched
files preserve the last installed project version. HTTPS transport tests exercise
catalog/file requests without depending on a live published repository.

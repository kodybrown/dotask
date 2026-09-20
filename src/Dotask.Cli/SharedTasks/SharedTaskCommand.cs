using System.Text.Json;
using DoTask.Cli.Configuration;
using DoTask.Cli.Discovery;
using DoTask.Cli.Parsing;

namespace DoTask.Cli.SharedTasks;

internal sealed class SharedTaskCommand( SharedTaskStore store, TextWriter output )
{
  public static readonly string[] Commands = ["--list", "--save", "--add", "--sync", "--remove"];

  public async Task<int> RunAsync( CommandLine command, string invocation, CancellationToken token )
  {
    var action = command.Command!.ToLowerInvariant();
    var dryRun = command.Arguments.Contains("--dry-run", StringComparer.OrdinalIgnoreCase);
    var acceptMerge = command.Arguments.Contains("--accept-merge", StringComparer.OrdinalIgnoreCase);
    var arguments = command.Arguments.Where(a => !a.Equals("--dry-run", StringComparison.OrdinalIgnoreCase)
      && !a.Equals("--accept-merge", StringComparison.OrdinalIgnoreCase)).ToArray();
    if (arguments.Any(a => a.StartsWith('-')) || (dryRun && action is not ("--add" or "--sync" or "--remove"))
      || (acceptMerge && (action != "--sync" || arguments.Length == 0))) {
      throw new TaskException("Usage: dotask --list [SELECTION...] | --save SELECTION... | --add SELECTION... [--dry-run] | --sync [SELECTION...] [--dry-run] [--accept-merge] | --remove SELECTION... [--dry-run]");
    }
    if (arguments.Length == 0 && action is "--add" or "--save" or "--remove") {
      throw new TaskException($"{action} requires a task selection, for example dotnet/build or \"dotnet/{{build,run}}\".");
    }
    var selections = TaskSelection.Parse(arguments.Length == 0 && action == "--list" ? ["*"] : arguments).ToArray();
    if (acceptMerge && selections.Any(s => s.Pattern.Contains('*'))) {
      throw new TaskException("--accept-merge requires explicitly named tasks, without wildcards.");
    }
    if (action is "--list" or "--save") {
      var tasks = await SelectAvailableAsync(selections, refresh: true, includeDependencies: action == "--save", token);
      if (action == "--list" && tasks.Count > 0) {
        WriteList(tasks, invocation, command.UseDirectory, token);
      }
      foreach (var (id, task) in tasks) {
        if (action == "--save") {
          await store.SaveAsync(id.Split('/')[0], task, token);
          output.WriteLine($"Saved {id}");
        }
      }
      if (tasks.Count == 0) {
        output.WriteLine("No shared tasks found.");
      }
      return 0;
    }

    var project = TaskDirectory.Locate(invocation, command.UseDirectory, allowMissing: action == "--add");
    var prefix = Path.GetRelativePath(project.RootDirectory, project.DirectoryPath).Replace(Path.DirectorySeparatorChar, '/');
    SharedTaskFiles.ValidateRelativePath(prefix);
    if (prefix == "." || prefix.StartsWith("../", StringComparison.Ordinal)) {
      throw new TaskException("Shared project tasks must be in a subdirectory of the project root.");
    }
    _ = SharedTaskFiles.Resolve(project.RootDirectory, prefix);
    await using var lease = await ProjectTaskTransaction.AcquireAsync(project, store.Options, token);
    var transaction = new ProjectTaskTransaction(project);
    if (transaction.HasPending) {
      if (dryRun) {
        throw new TaskException("An interrupted update requires recovery. Run the management command without --dry-run to recover it first.");
      }
      transaction.Recover();
      output.WriteLine("Recovered the interrupted shared-task operation.");
    }
    _ = ProjectConfiguration.Load(project.DirectoryPath, project.RootDirectory);
    var lockPath = SharedTaskFiles.Resolve(project.RootDirectory, ".dotasks-lock.yaml");
    var beforeBytes = File.Exists(lockPath) ? await File.ReadAllBytesAsync(lockPath, token) : null;
    var before = TaskLock.Read(beforeBytes, prefix);
    var next = TaskLock.Read(beforeBytes, prefix);
    var preserve = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
    var conflicts = new List<string>();

    if (action == "--remove") {
      var ids = SelectInstalled(before, selections);
      foreach (var id in ids) {
        VerifyInstalled(project, before.Tasks[id], conflicts);
        next.Tasks.Remove(id);
        output.WriteLine($"{(dryRun ? "Would remove" : "Remove")} {id}");
      }
    } else {
      if (action == "--sync") {
        var ids = selections.Length == 0 ? before.Tasks.Keys.ToArray() : SelectInstalled(before, selections);
        selections = TaskSelection.Parse(ids).ToArray();
      }
      var available = await SelectAvailableAsync(selections, refresh: action == "--sync", includeDependencies: true, token);
      foreach (var (id, task) in available) {
        var source = id.Split('/')[0];
        if (action == "--add" && before.Tasks.TryGetValue(id, out var already)) {
          VerifyInstalled(project, already, conflicts);
          output.WriteLine($"Already installed: {id}. Use --sync to update it.");
          continue;
        }
        await store.SaveAsync(source, task, token);
        var replacement = InstalledTask.From(source, task);
        if (before.Tasks.TryGetValue(id, out var installed)) {
          var changed = installed.Files.Where(f => SharedTaskFiles.HashFile(SharedTaskFiles.Resolve(project.DirectoryPath, f.Key)) != f.Value).ToArray();
          if (installed.Revision == replacement.Revision) {
            foreach (var file in changed) {
              var path = SharedTaskFiles.Resolve(project.DirectoryPath, file.Key);
              if (!File.Exists(path)) {
                conflicts.Add($"Tracked file is missing: {path}. Restore it explicitly; sync will not undo a local deletion.");
              }
            }
            output.WriteLine($"Unchanged: {id}{(changed.Length > 0 ? " (local changes preserved)" : "")}");
            continue;
          }
          var sameFiles = installed.Files.Keys.ToHashSet(StringComparer.Ordinal).SetEquals(replacement.Files.Keys);
          var matchesIncoming = sameFiles && replacement.Files.All(f => SharedTaskFiles.HashFile(SharedTaskFiles.Resolve(project.DirectoryPath, f.Key)) == f.Value);
          var acceptThisMerge = acceptMerge && selections.Any(s => s.Source == source && s.Pattern.Equals(task.Id, StringComparison.OrdinalIgnoreCase));
          if (changed.Length > 0 && !matchesIncoming) {
            if (acceptThisMerge && sameFiles && HasReview(project, id, installed, replacement)
              && installed.Files.Keys.All(f => File.Exists(SharedTaskFiles.Resolve(project.DirectoryPath, f)))) {
              preserve.UnionWith(installed.Files.Keys);
              output.WriteLine($"Record reviewed merge: {id}. Local differences remain protected.");
            } else {
              var comparison = PrepareComparison(project, id, installed, replacement);
              conflicts.Add($"Locally modified: {id}. Files were preserved. Compare originals/project/incoming in {comparison}. Merge manually, then run dotask --sync {id} --accept-merge; or replace the project copy with incoming and rerun --sync.");
              continue;
            }
          }
        }
        next.Tasks[id] = replacement;
        output.WriteLine($"{(dryRun ? "Would install/update" : "Install/update")} {id}");
      }
    }
    foreach (var (id, task) in next.Tasks) {
      foreach (var dependency in task.Requires) {
        if (!next.Tasks.ContainsKey(dependency)) {
          conflicts.Add($"Cannot remove required task '{dependency}'; '{id}' still depends on it.");
        }
      }
    }
    var afterBytes = next.Serialize();
    // Validate cross-task support-file ownership before any mutation.
    _ = TaskLock.Read(afterBytes, prefix);
    var oldFiles = FileMap(before);
    var newFiles = FileMap(next);
    var changes = new List<TaskFileChange>();
    foreach (var path in oldFiles.Keys.Union(newFiles.Keys, StringComparer.Ordinal).Order(StringComparer.Ordinal)) {
      var oldHash = oldFiles.GetValueOrDefault(path);
      var newHash = newFiles.GetValueOrDefault(path);
      if (oldHash == newHash || preserve.Contains(path)) {
        continue;
      }
      var destination = SharedTaskFiles.Resolve(project.DirectoryPath, path);
      var current = SharedTaskFiles.HashFile(destination);
      if (Directory.Exists(destination) || (oldHash is null && current is not null)
        || (oldHash is not null && current != oldHash && (current is null || current != newHash))) {
        conflicts.Add($"Refusing to overwrite/remove '{destination}': it is untracked, missing, or locally modified.");
        continue;
      }
      if (current != newHash) {
        changes.Add(new(prefix + "/" + path, current, newHash is null ? null : store.Original(path.Split('/')[0], newHash)));
      }
    }
    if (conflicts.Count > 0) {
      foreach (var conflict in conflicts.Distinct(StringComparer.Ordinal)) {
        output.WriteLine(conflict);
      }
      output.WriteLine("No project task files or tracking data were changed.");
      return 1;
    }
    if (beforeBytes is null && next.Tasks.Count == 0) {
      output.WriteLine("No installed shared tasks to sync.");
      return 0;
    }
    if (beforeBytes is null || !beforeBytes.AsSpan().SequenceEqual(afterBytes)) {
      changes.Add(new(".dotasks-lock.yaml", beforeBytes is null ? null : SharedTaskFiles.Hash(beforeBytes), afterBytes));
    }
    var configPath = SharedTaskFiles.Resolve(project.RootDirectory, ".dotasks.yaml");
    if (action == "--add" && !File.Exists(configPath) && !File.Exists(Path.Combine(project.DirectoryPath, "config.yaml"))) {
      changes.Add(new(".dotasks.yaml", null, "version: 1\nsettings: {}\n"u8.ToArray()));
    }
    if (dryRun) {
      output.WriteLine($"Dry run: {changes.Count} project file change(s); nothing written to the project.");
    } else {
      transaction.Apply(changes, token);
      output.WriteLine(changes.Count == 0 ? "Project tasks are up to date." : "Project tasks and tracking updated. Review and commit the changes.");
    }
    return 0;
  }

  private void WriteList( SortedDictionary<string, SharedTask> tasks, string invocation, string? useDirectory, CancellationToken token )
  {
    var project = TaskDirectory.Locate(invocation, useDirectory, allowMissing: useDirectory is null);
    var prefix = Path.GetRelativePath(project.RootDirectory, project.DirectoryPath).Replace(Path.DirectorySeparatorChar, '/');
    var lockPath = SharedTaskFiles.Resolve(project.RootDirectory, ".dotasks-lock.yaml");
    var tracking = TaskLock.Read(File.Exists(lockPath) ? File.ReadAllBytes(lockPath) : null, prefix);
    var rows = tasks.Select(pair =>
    {
      token.ThrowIfCancellationRequested();
      var (id, task) = pair;
      var source = id.Split('/')[0];
      tracking.Tasks.TryGetValue(id, out var installed);
      var entry = SharedTaskFiles.Resolve(project.DirectoryPath, id + ".cs");
      var present = File.Exists(entry);
      var status = present ? installed is null ? "Untracked" : "Installed"
        : installed is null ? "Not in project" : "Missing";
      var comparison = "—";
      if (present) {
        var paths = task.Files.Select(f => source + "/" + f.Path)
          .Union(installed?.Files.Keys.AsEnumerable() ?? [], StringComparer.Ordinal).ToArray();
        var pairs = paths.Select(path => (
          Project: SharedTaskFiles.HashFile(SharedTaskFiles.Resolve(project.DirectoryPath, path)),
          Cached: SharedTaskFiles.HashFile(SharedTaskFiles.Resolve(
            source == "private-tasks" ? store.Options.PrivateDirectory : store.Options.CacheDirectory,
            source == "private-tasks" ? path[(source.Length + 1)..] : path)))).ToArray();
        comparison = pairs.Any(p => p.Cached is null) ? "Unavailable"
          : pairs.All(p => p.Project == p.Cached) ? source == "private-tasks" ? "Matches original" : "Matches cache" : "Differs";
      }
      return (Id: id, Status: status, Comparison: comparison, task.Description);
    }).ToArray();
    var width = Math.Max(4, rows.Max(r => r.Id.Length));
    output.WriteLine($"{"Task".PadRight(width)}  {"Project",-14}  {"Comparison",-16}  Description");
    foreach (var row in rows) {
      output.WriteLine($"{row.Id.PadRight(width)}  {row.Status,-14}  {row.Comparison,-16}  {row.Description}");
    }
  }

  private static Dictionary<string, string> FileMap( TaskLock data ) => data.Tasks.Values.SelectMany(t => t.Files)
    .GroupBy(f => f.Key, StringComparer.Ordinal).ToDictionary(g => g.Key, g => g.First().Value, StringComparer.Ordinal);

  private static string[] SelectInstalled( TaskLock data, TaskSelection[] selections )
  {
    var ids = new HashSet<string>(StringComparer.Ordinal);
    foreach (var selection in selections) {
      var matches = data.Tasks.Keys.Where(id => id.StartsWith(selection.Source + "/", StringComparison.OrdinalIgnoreCase)
        && selection.Matches(id[(id.IndexOf('/') + 1)..])).ToArray();
      if (matches.Length == 0) {
        throw new TaskException($"No installed shared tasks match '{selection.Source}/{selection.Pattern}'. Handwritten/untracked tasks cannot be removed by dotask.");
      }
      ids.UnionWith(matches);
    }
    return ids.Order(StringComparer.Ordinal).ToArray();
  }

  private static void VerifyInstalled( TaskDirectory project, InstalledTask task, List<string> conflicts )
  {
    foreach (var (relative, original) in task.Files) {
      var path = SharedTaskFiles.Resolve(project.DirectoryPath, relative);
      if (Directory.Exists(path) || SharedTaskFiles.HashFile(path) != original) {
        conflicts.Add($"Refusing to change '{path}': the installed shared task is missing or locally modified.");
      }
    }
  }

  private async Task<SortedDictionary<string, SharedTask>> SelectAvailableAsync( TaskSelection[] selections, bool refresh,
    bool includeDependencies, CancellationToken token )
  {
    var result = new SortedDictionary<string, SharedTask>(StringComparer.Ordinal);
    foreach (var group in selections.GroupBy(s => s.Source)) {
      var catalog = await store.CatalogAsync(group.Key, refresh, token);
      var lookup = catalog.Tasks.ToDictionary(t => t.Id, StringComparer.OrdinalIgnoreCase);
      var visiting = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
      void Add( SharedTask task )
      {
        var id = group.Key + "/" + task.Id;
        if (!visiting.Add(id)) {
          throw new TaskException($"Shared task dependency cycle at '{id}'.");
        }
        if (!result.ContainsKey(id)) {
          if (includeDependencies) {
            foreach (var dependency in task.Requires) {
              Add(lookup[dependency]);
            }
          }
          result[id] = task;
        }
        visiting.Remove(id);
      }
      foreach (var selection in group) {
        var matches = catalog.Tasks.Where(t => selection.Matches(t.Id)).ToArray();
        if (matches.Length == 0 && selection.Pattern != "*") {
          throw new TaskException($"No shared tasks match '{selection.Source}/{selection.Pattern}'. Existing project files are preserved.");
        }
        foreach (var task in matches) {
          Add(task);
        }
      }
    }
    return result;
  }

  private string PrepareComparison( TaskDirectory project, string id, InstalledTask oldTask, InstalledTask incoming )
  {
    var directory = ComparisonDirectory(project, id, incoming.Revision);
    Directory.CreateDirectory(directory);
    foreach (var path in oldTask.Files.Keys.Union(incoming.Files.Keys, StringComparer.Ordinal)) {
      if (oldTask.Files.TryGetValue(path, out var original)) {
        var cached = store.ObjectPath(oldTask.Source, original);
        if (File.Exists(cached)) {
          SharedTaskFiles.AtomicWrite(SharedTaskFiles.Resolve(directory, path + ".base"), store.Original(oldTask.Source, original));
        }
      }
      var current = SharedTaskFiles.Resolve(project.DirectoryPath, path);
      if (File.Exists(current)) {
        SharedTaskFiles.AtomicWrite(SharedTaskFiles.Resolve(directory, path + ".project"), File.ReadAllBytes(current));
      }
      if (incoming.Files.TryGetValue(path, out var hash)) {
        SharedTaskFiles.AtomicWrite(SharedTaskFiles.Resolve(directory, path + ".incoming"), store.Original(incoming.Source, hash));
      }
    }
    SharedTaskFiles.AtomicWrite(SharedTaskFiles.Resolve(directory, "review.json"),
      JsonSerializer.SerializeToUtf8Bytes(new MergeReview(id, oldTask.Revision, incoming.Revision), SharedTaskJson.Options));
    return directory;
  }

  private string ComparisonDirectory( TaskDirectory project, string id, string revision )
    => SharedTaskFiles.Resolve(store.Options.CacheDirectory,
      ".conflicts/" + SharedTaskFiles.Hash(project.RootDirectory)[..16] + "/" + id + "/" + revision);

  private bool HasReview( TaskDirectory project, string id, InstalledTask installed, InstalledTask incoming )
  {
    var path = SharedTaskFiles.Resolve(ComparisonDirectory(project, id, incoming.Revision), "review.json");
    return File.Exists(path) && JsonSerializer.Deserialize<MergeReview>(File.ReadAllBytes(path), SharedTaskJson.Options)
      == new MergeReview(id, installed.Revision, incoming.Revision);
  }

  private sealed record MergeReview( string Task, string BeforeRevision, string IncomingRevision );
}

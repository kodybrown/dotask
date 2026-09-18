using DoTask.Cli.Discovery;
using DoTask.Cli.Metadata;
using DoTask.Cli.Parsing;
using DoTask.Cli.SharedTasks;

namespace DoTask.Cli.Completion;

public sealed record CompletionCandidate( string Value, string Kind, string Description );

public static class CompletionEngine
{
  private static readonly CompletionCandidate[] Globals = [
    new("--use-dir", "option", "Select a task directory"), new("--help", "option", "Show help"),
    new("--version", "option", "Show version"),
    new("--init", "option", "Initialize a project in the current directory"),
    new("--list", "option", "List shared tasks"), new("--save", "option", "Download shared tasks"),
    new("--add", "option", "Add shared project tasks"), new("--sync", "option", "Sync shared project tasks"),
    new("--remove", "option", "Remove unchanged shared project tasks")];

  public static IReadOnlyList<CompletionCandidate> Complete( string line, string invocationDirectory, string shell = "bash",
    SharedTaskOptions? sharedTaskOptions = null )
  {
    var words = ShellWords.Parse(line, shell).Skip(1).ToList();
    if (words.Count == 0) {
      words.Add("");
    }
    var current = words[^1];
    words.RemoveAt(words.Count - 1);
    string? useDir = null;
    for (var index = 0; index < words.Count; index++) {
      if (words[index].Equals("--use-dir", StringComparison.OrdinalIgnoreCase)) {
        if (index + 1 == words.Count) {
          return Paths(current, invocationDirectory, directoriesOnly: true);
        }
        useDir = words[index + 1];
        words.RemoveRange(index, 2);
        index--;
      } else if (words[index].StartsWith("--use-dir=", StringComparison.OrdinalIgnoreCase)) {
        useDir = words[index][10..];
        words.RemoveAt(index--);
      }
    }
    if (current.StartsWith("--use-dir=", StringComparison.OrdinalIgnoreCase)) {
      return Prefix(Paths(current[10..], invocationDirectory, true), current[..10]);
    }
    if (words.FirstOrDefault()?.Equals("completion", StringComparison.OrdinalIgnoreCase) == true) {
      return Filter(new[] { "bash", "zsh", "fish", "powershell" }.Select(s => new CompletionCandidate(s, "value", "Shell integration")), current);
    }
    if (words.FirstOrDefault()?.Equals("--init", StringComparison.OrdinalIgnoreCase) == true) {
      return Filter(Globals.Where(c => c.Value is "--help" or "--version" || (c.Value == "--use-dir" && useDir is null)), current);
    }
    TargetCatalog? catalog = null;
    TaskDirectory? directory = null;
    try {
      directory = TaskDirectory.Locate(invocationDirectory, useDir);
      catalog = new TargetCatalog(directory.DirectoryPath);
    } catch (Exception ex) when (ex is TaskException or IOException or UnauthorizedAccessException) { }
    var targets = catalog?.Targets ?? [];
    if (SharedTaskCommand.Commands.Contains(words.FirstOrDefault(), StringComparer.OrdinalIgnoreCase)) {
      var candidates = new List<CompletionCandidate>();
      var action = words[0].ToLowerInvariant();
      if (action is "--remove" or "--sync") {
        if (directory is not null) {
          try {
            var path = Path.Combine(directory.RootDirectory, ".dotasks-lock.yaml");
            var taskRoot = Path.GetRelativePath(directory.RootDirectory, directory.DirectoryPath).Replace(Path.DirectorySeparatorChar, '/');
            var tracking = TaskLock.Read(File.Exists(path) ? File.ReadAllBytes(path) : null, taskRoot);
            candidates.AddRange(tracking.Tasks.Keys.Select(id => new CompletionCandidate(id, "target", "Installed shared task")));
          } catch (Exception ex) when (ex is TaskException or IOException or UnauthorizedAccessException) { }
        }
      } else {
        try {
          foreach (var (source, task) in new SharedTaskStore(sharedTaskOptions ?? SharedTaskOptions.FromEnvironment()).CachedTasks()) {
            candidates.Add(new(source + "/" + task.Id, "target", task.Description));
            if (source == "_") {
              candidates.Add(new(task.Id, "target", task.Description));
            }
          }
        } catch (Exception ex) when (ex is TaskException or IOException or UnauthorizedAccessException or System.Text.Json.JsonException) { }
      }
      if (action is "--add" or "--sync" or "--remove") {
        candidates.Add(new("--dry-run", "option", "Preview without changing project files"));
      }
      if (action == "--sync") {
        candidates.Add(new("--accept-merge", "option", "Record an already reviewed manual merge"));
      }
      return Filter(candidates, current);
    }
    if (words.Count == 0 || words[0].Equals("help", StringComparison.OrdinalIgnoreCase)) {
      var candidates = targets.SelectMany(t => catalog!.NamesFor(t)
        .Select(name => new CompletionCandidate(name, "target", t.Error ?? t.Description)));
      if (words.Count == 0) {
        candidates = candidates.Concat(Globals).Concat([
          new("help", "command", "Show target help"), new("completion", "command", "Print shell integration")]);
      }
      return Filter(candidates, current);
    }
    TargetDefinition? target;
    try {
      target = catalog?.Get(words[0]);
    } catch (TaskException) {
      target = null;
    }
    if (target is null) {
      return Filter(Globals, current);
    }
    var equals = current.IndexOf('=');
    if (equals >= 0) {
      var option = OptionBinder.Find(target, current[..equals].TrimStart('-'));
      return option is null ? [] : Prefix(Values(option, current[(equals + 1)..], directory!.RootDirectory), current[..(equals + 1)]);
    }
    var used = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
    OptionDefinition? pending = null;
    foreach (var word in words.Skip(1)) {
      if (pending is not null) {
        if (pending.Type != "bool" || bool.TryParse(word, out _)) {
          pending = null;
          continue;
        }
        pending = null;
      }
      var key = word.Split('=', 2)[0].TrimStart('-');
      var option = OptionBinder.Find(target, key);
      if (option is not null) {
        used.Add(option.Name);
        if (!word.Contains('=') && word.StartsWith('-')) {
          pending = option;
        }
      }
    }
    if (pending is not null && (pending.Type != "bool" || !current.StartsWith('-'))) {
      return Values(pending, current, directory!.RootDirectory);
    }
    var options = target.Options.Where(o => !used.Contains(o.Name)).SelectMany(option =>
    {
      var names = new List<string> { "--" + option.Name, option.Name + "=" };
      if (option.Alias is not null) {
        names.Add("-" + option.Alias);
      }
      return names.Select(n => new CompletionCandidate(n, "option", option.Description));
    });
    return Filter(options.Concat(Globals), current);
  }

  private static IReadOnlyList<CompletionCandidate> Values( OptionDefinition option, string prefix, string root )
  {
    if (option.Choices.Length > 0) {
      return Filter(option.Choices.Select(c => new CompletionCandidate(c, "value", option.Description)), prefix);
    }
    if (option.Type == "bool") {
      return Filter(new[] { "true", "false" }.Select(v => new CompletionCandidate(v, "value", option.Description)), prefix);
    }
    return option.Completion is not null || option.Type == "path"
      ? Paths(prefix, root, option.Completion == "directory") : [];
  }

  private static IReadOnlyList<CompletionCandidate> Paths( string prefix, string root, bool directoriesOnly )
  {
    try {
      var normalized = prefix.Replace('\\', '/');
      var separator = normalized.LastIndexOf('/');
      var parent = separator < 0 ? "" : normalized[..(separator + 1)];
      var name = normalized[(separator + 1)..];
      var folder = parent.StartsWith("~/") ? PortablePath.Resolve(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), parent[2..])
        : PortablePath.Resolve(root, parent);
      if (!Directory.Exists(folder)) {
        return [];
      }
      return Directory.EnumerateFileSystemEntries(folder)
        .Where(path => Path.GetFileName(path).StartsWith(name, OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal))
        .Where(path => !directoriesOnly || Directory.Exists(path))
        .Select(path => new CompletionCandidate(parent + Path.GetFileName(path) + (Directory.Exists(path) ? "/" : ""),
          Directory.Exists(path) ? "directory" : "file", Directory.Exists(path) ? "Directory" : "File"))
        .Where(c => !c.Value.Any(char.IsControl)).OrderBy(c => c.Value, StringComparer.Ordinal).ToArray();
    } catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException or TaskException) {
      return [];
    }
  }

  private static IReadOnlyList<CompletionCandidate> Prefix( IEnumerable<CompletionCandidate> candidates, string prefix )
    => candidates.Select(c => c with { Value = prefix + c.Value }).ToArray();

  private static IReadOnlyList<CompletionCandidate> Filter( IEnumerable<CompletionCandidate> candidates, string prefix )
    => candidates.Where(c => c.Value.StartsWith(prefix, StringComparison.OrdinalIgnoreCase) && !c.Value.Any(char.IsControl))
      .DistinctBy(c => c.Value, StringComparer.OrdinalIgnoreCase).OrderBy(c => c.Value, StringComparer.OrdinalIgnoreCase).ToArray();
}

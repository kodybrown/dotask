using DoTask.Cli.Configuration;
using DoTask.Cli.Metadata;

namespace DoTask.Cli.Execution;

internal static class Requirements
{
  public static void Validate(TargetDefinition target, ProjectConfiguration config, string root)
  {
    var values = new Values(config.Settings, root);
    foreach (var requirement in target.Requirements)
    {
      var valid = requirement.Kind switch
      {
        "setting" => values.Contains(requirement.Value),
        "tool" => FindTool(requirement.Value, root),
        "os" => requirement.Value.Split(',', StringSplitOptions.TrimEntries).Any(name =>
          name.Equals(HostName(), StringComparison.OrdinalIgnoreCase)),
        // Shared-task management resolves these against the source catalog/root.
        // They never trigger downloads or task execution here.
        "task" or "file" => true,
        _ => false
      };
      if (!valid)
      {
        throw new TaskException($"Target '{target.Name}' requires {requirement.Kind} '{requirement.Value}'.");
      }
    }
  }

  public static string HostName() => OperatingSystem.IsWindows() ? "windows" : OperatingSystem.IsLinux() ? "linux" :
    OperatingSystem.IsMacOS() ? "macos" : "unknown";

  private static bool FindTool(string tool, string root)
  {
    var names = OperatingSystem.IsWindows() && !Path.HasExtension(tool)
      ? new[] { tool }.Concat((Environment.GetEnvironmentVariable("PATHEXT") ?? ".EXE;.COM;.BAT;.CMD").Split(';').Select(e => tool + e))
      : [tool];
    var directories = tool.IndexOfAny(['/', '\\']) >= 0 ? new[] { root } :
      (Environment.GetEnvironmentVariable("PATH") ?? "").Split(Path.PathSeparator, StringSplitOptions.RemoveEmptyEntries);
    foreach (var directory in directories)
    {
      foreach (var name in names)
      {
        var candidate = Path.GetFullPath(Path.Combine(directory, name), root);
        if (File.Exists(candidate) && (OperatingSystem.IsWindows()
          || (File.GetUnixFileMode(candidate) & (UnixFileMode.UserExecute | UnixFileMode.GroupExecute | UnixFileMode.OtherExecute)) != 0))
        {
          return true;
        }
      }
    }
    return false;
  }
}

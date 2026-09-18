namespace DoTask.Cli.Discovery;

public sealed record TaskDirectory(string DirectoryPath, string RootDirectory, string InvocationDirectory)
{
  public static TaskDirectory Locate(string invocationDirectory, string? useDirectory = null, bool allowMissing = false)
  {
    var invocation = Path.GetFullPath(invocationDirectory);
    if (useDirectory is not null)
    {
      var selected = PortablePath.Resolve(invocation, useDirectory);
      if (!Directory.Exists(selected) && !allowMissing)
      {
        throw new TaskException($"Task directory does not exist: {selected}");
      }
      var anchor = File.Exists(Path.Combine(selected, "config.yaml")) ? null : FindConfigurationRoot(selected);
      return new(selected, anchor is not null && !Path.GetRelativePath(anchor, selected).StartsWith("..", StringComparison.Ordinal)
        ? anchor : Directory.GetParent(selected)?.FullName ?? selected, invocation);
    }
    for (var current = new DirectoryInfo(invocation); current is not null; current = current.Parent)
    {
      var candidate = Path.Combine(current.FullName, ".tasks");
      if (File.Exists(Path.Combine(current.FullName, ".dotasks.yaml")) || Directory.Exists(candidate))
      {
        return Create(candidate, invocation);
      }
    }
    if (allowMissing)
    {
      return Create(Path.Combine(invocation, ".tasks"), invocation);
    }
    throw new TaskException("No .dotasks.yaml or .tasks directory found. Create a project configuration or use dotask --add TASK.");
  }

  private static string? FindConfigurationRoot(string invocation)
  {
    for (var current = new DirectoryInfo(invocation); current is not null; current = current.Parent)
    {
      if (File.Exists(Path.Combine(current.FullName, ".dotasks.yaml")))
      {
        return current.FullName;
      }
    }
    return null;
  }

  private static TaskDirectory Create(string path, string invocation) => new(
    Path.TrimEndingDirectorySeparator(path), Directory.GetParent(path)?.FullName ?? path, invocation);
}

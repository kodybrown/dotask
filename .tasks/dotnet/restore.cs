using DoTask;

/// <summary>Restore NuGet packages for the configured solution.</summary>
/// <option name="dotnet" default="dotnet">.NET CLI executable name or path.</option>
/// <requires tool="dotnet" />
/// <requires setting="solution" />
/// <example>dotask restore</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var solution = project.Config.GetPath("solution");
    if (!File.Exists(solution)) {
      throw new TaskException($"Solution is missing: {solution}");
    }

    await project.RunAsync(project.Parameters.Get<string>("dotnet"), ["restore", solution]);
  }
}

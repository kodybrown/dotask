using DoTask;

/// <summary>Clean the configured solution's build outputs for one configuration.</summary>
/// <option name="configuration" alias="c" choices="Debug,Release" default="Debug">Build configuration to clean.</option>
/// <option name="dotnet" default="dotnet">.NET CLI executable name or path.</option>
/// <requires tool="dotnet" />
/// <requires setting="solution" />
/// <example>dotask clean</example>
/// <example>dotask clean -c Release</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var solution = project.Config.GetPath("solution");
    if (!File.Exists(solution)) {
      throw new TaskException($"Solution is missing: {solution}");
    }

    await project.RunAsync(project.Parameters.Get<string>("dotnet"), [
      "clean", solution, "--configuration", project.Parameters.Get<string>("configuration")
    ]);
  }
}

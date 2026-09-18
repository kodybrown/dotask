using DoTask;

/// <summary>Build the configured .NET project and create local NuGet packages.</summary>
/// <option name="configuration" alias="c" choices="Debug,Release" default="Release">Build configuration.</option>
/// <option name="output" alias="o" type="path" default="artifacts/packages" completion="directory">Package directory, relative to the project root.</option>
/// <option name="dotnet" default="dotnet">.NET CLI executable name or path.</option>
/// <requires tool="dotnet" />
/// <requires setting="project" />
/// <example>dotask pack</example>
/// <example>dotask pack -c Debug --output ./artifacts/packages</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var projectFile = project.Config.GetPath("project");
    if (!File.Exists(projectFile))
    {
      throw new TaskException($"Project file does not exist: {projectFile}");
    }
    await project.RunAsync(project.Parameters.Get<string>("dotnet"), [
      "pack", projectFile,
      "--configuration", project.Parameters.Get<string>("configuration"),
      "--output", project.Parameters.GetPath("output")
    ]);
  }
}

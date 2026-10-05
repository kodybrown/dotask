// dotask: 1
// description: "Build the configured .NET project and create local NuGet packages."
// options:
//   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"], "default": "Release", "description": "Build configuration."}
//   - {"name": "output", "alias": "o", "type": "path", "default": "artifacts/packages", "completion": "directory", "description": "Package directory, relative to the project root."}
//   - {"name": "dotnet", "default": "dotnet", "description": ".NET CLI executable name or path."}
// requires:
//   - {"kind": "tool", "value": "dotnet"}
//   - {"kind": "setting", "value": "project"}
// examples: ["dotask pack", "dotask pack -c Debug --output ./artifacts/packages"]
// end-dotask
using DoTask;


public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var projectFile = project.Config.GetPath("project");
    if (!File.Exists(projectFile)) {
      throw new TaskException($"Project file does not exist: {projectFile}");
    }
    await project.RunAsync(project.Parameters.Get<string>("dotnet"), [
      "pack", projectFile,
      "--configuration", project.Parameters.Get<string>("configuration"),
      "--output", project.Parameters.GetPath("output")
    ]);
  }
}

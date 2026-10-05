// dotask: 1
// description: "Clean the configured solution's build outputs for one configuration."
// options:
//   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"], "default": "Debug", "description": "Build configuration to clean."}
//   - {"name": "dotnet", "default": "dotnet", "description": ".NET CLI executable name or path."}
// requires:
//   - {"kind": "tool", "value": "dotnet"}
//   - {"kind": "setting", "value": "solution"}
// examples: ["dotask clean", "dotask clean -c Release"]
// end-dotask
using DoTask;


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

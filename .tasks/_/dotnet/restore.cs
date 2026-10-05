// dotask: 1
// description: "Restore NuGet packages for the configured solution."
// options:
//   - {"name": "dotnet", "default": "dotnet", "description": ".NET CLI executable name or path."}
// requires:
//   - {"kind": "tool", "value": "dotnet"}
//   - {"kind": "setting", "value": "solution"}
// examples: ["dotask restore"]
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

    await project.RunAsync(project.Parameters.Get<string>("dotnet"), ["restore", solution]);
  }
}

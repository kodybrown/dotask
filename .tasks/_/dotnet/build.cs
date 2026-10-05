// dotask: 1
// description: "Restore packages and build the configured solution."
// options:
//   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"], "default": "Debug", "description": "Build configuration."}
//   - {"name": "dotnet", "default": "dotnet", "description": ".NET CLI executable name or path."}
// requires:
//   - {"kind": "tool", "value": "dotnet"}
//   - {"kind": "setting", "value": "solution"}
//   - {"kind": "task", "value": "dotnet/restore"}
// examples: ["dotask build", "dotask build -c Release"]
// end-dotask
using DoTask;


public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var dotnet = project.Parameters.Get<string>("dotnet");
    await project.ExecTargetAsync("_/dotnet/restore", new { Dotnet = dotnet });
    await project.RunAsync(dotnet, [
      "build", project.Config.GetPath("solution"),
      "--configuration", project.Parameters.Get<string>("configuration"), "--no-restore"
    ]);
  }
}

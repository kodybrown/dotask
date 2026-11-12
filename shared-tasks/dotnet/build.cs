// dotask: 1
// description: "Restore packages and build the configured solution."
// options:
//   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"], "description": "Override settings.configuration; defaults to Debug when neither is set."}
//   - {"name": "dotnet", "default": "dotnet", "description": ".NET CLI executable name or path."}
// requires:
//   - {"kind": "tool", "value": "dotnet"}
//   - {"kind": "setting", "value": "solution"}
//   - {"kind": "task", "value": "dotnet/restore"}
// examples: ["dotask build", "dotask build -c Release", "dotask build --configuration Debug"]
// end-dotask
using DoTask;

public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var dotnet = project.Parameters.Get<string>("dotnet");
    // Leave the option without a metadata default so an omitted parameter can
    // inherit the project's configuration instead of always selecting Debug.
    var configuration = project.Parameters.Contains("configuration")
      ? project.Parameters.Get<string>("configuration")
      : project.Config.Get("configuration", "Debug");
    await project.ExecTargetAsync("_/dotnet/restore", new { Dotnet = dotnet });
    await project.RunAsync(dotnet, [
      "build", project.Config.GetPath("solution"),
      "--configuration", configuration, "--no-restore"
    ]);
  }
}

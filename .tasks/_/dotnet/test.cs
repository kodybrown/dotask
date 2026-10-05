// dotask: 1
// description: "Build and run the solution's tests."
// options:
//   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"], "default": "Release", "description": "Build configuration."}
// requires:
//   - {"kind": "tool", "value": "dotnet"}
//   - {"kind": "setting", "value": "solution"}
// end-dotask
using DoTask;


public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var config = project.Config;
    await project.RunAsync("dotnet", ["test", config.GetPath("solution"), "-c", project.Parameters.Get<string>("configuration")]);
  }
}

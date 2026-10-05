// dotask: 1
// description: "Publish the project for the selected OS and CPU architecture."
// options:
//   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"], "default": "Release", "description": "Build configuration."}
//   - {"name": "OS", "choices": ["host", "windows", "linux", "macos"], "default": "host", "description": "Output operating system; project.OS still describes the host."}
//   - {"name": "architecture", "alias": "a", "choices": ["host", "x64", "arm64"], "default": "host", "description": "Output CPU architecture."}
//   - {"name": "self-contained", "type": "bool", "default": "false", "description": "Include the .NET runtime in the published output."}
// requires:
//   - {"kind": "tool", "value": "dotnet"}
//   - {"kind": "setting", "value": "project"}
// examples: ["dotask publish OS=linux architecture=x64"]
// end-dotask
using DoTask;


public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var outputOS = project.Parameters.Get<string>("OS");
    if (outputOS == "host") {
      outputOS = project.OS.ToString().ToLowerInvariant();
    }
    var architecture = project.Parameters.Get<string>("architecture");
    if (architecture == "host") {
      architecture = project.Architecture.ToString().ToLowerInvariant();
    }
    var runtimeOS = outputOS switch { "windows" => "win", "macos" => "osx", _ => outputOS };
    await project.RunAsync("dotnet", ["publish", project.Config.GetPath("project"),
      "-c", project.Parameters.Get<string>("configuration"), "-r", $"{runtimeOS}-{architecture}",
      "--self-contained", project.Parameters.Get<bool>("self-contained") ? "true" : "false"]);
  }
}

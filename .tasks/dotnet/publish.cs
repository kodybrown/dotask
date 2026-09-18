using DoTask;

/// <summary>Publish the project for the selected OS and CPU architecture.</summary>
/// <option name="configuration" alias="c" choices="Debug,Release" default="Release">Build configuration.</option>
/// <option name="OS" choices="host,windows,linux,macos" default="host">Output operating system; project.OS still describes the host.</option>
/// <option name="architecture" alias="a" choices="host,x64,arm64" default="host">Output CPU architecture.</option>
/// <option name="self-contained" type="bool" default="false">Include the .NET runtime in the published output.</option>
/// <requires tool="dotnet" />
/// <requires setting="project" />
/// <example>dotask publish OS=linux architecture=x64</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var outputOS = project.Parameters.Get<string>("OS");
    if (outputOS == "host")
    {
      outputOS = project.OS.ToString().ToLowerInvariant();
    }
    var architecture = project.Parameters.Get<string>("architecture");
    if (architecture == "host")
    {
      architecture = project.Architecture.ToString().ToLowerInvariant();
    }
    var runtimeOS = outputOS switch { "windows" => "win", "macos" => "osx", _ => outputOS };
    await project.RunAsync("dotnet", ["publish", project.Config.GetPath("project"),
      "-c", project.Parameters.Get<string>("configuration"), "-r", $"{runtimeOS}-{architecture}",
      "--self-contained", project.Parameters.Get<bool>("self-contained") ? "true" : "false"]);
  }
}

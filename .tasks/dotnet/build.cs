using DoTask;

/// <summary>Restore packages and build the configured solution.</summary>
/// <option name="configuration" alias="c" choices="Debug,Release" default="Debug">Build configuration.</option>
/// <option name="dotnet" default="dotnet">.NET CLI executable name or path.</option>
/// <requires tool="dotnet" />
/// <requires setting="solution" />
/// <requires task="dotnet/restore" />
/// <example>dotask build</example>
/// <example>dotask build -c Release</example>
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

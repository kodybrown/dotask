using DoTask;

/// <summary>Build and run the solution's tests.</summary>
/// <option name="configuration" alias="c" choices="Debug,Release" default="Release">Build configuration.</option>
/// <requires tool="dotnet" />
/// <requires setting="solution" />
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var config = project.Config;
    await project.RunAsync("dotnet", ["test", config.GetPath("solution"), "-c", project.Parameters.Get<string>("configuration")]);
  }
}

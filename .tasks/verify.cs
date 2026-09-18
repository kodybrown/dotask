using DoTask;

/// <summary>Build and test dotask, then check formatting, documentation, and the shared catalog.</summary>
/// <option name="configuration" alias="c" choices="Debug,Release" default="Release">Build configuration.</option>
/// <remarks>Every verification task is required. A missing task or failure stops verification.</remarks>
/// <example>dotask verify</example>
/// <example>dotask verify -c Debug</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    await project.ExecTargetAsync("check");
    // dotnet test builds the solution before running its tests.
    await project.ExecTargetAsync("dotnet/test", new { Configuration = project.Parameters.Get<string>("configuration") });
    await project.ExecTargetAsync("dotnet/format", new { Verify = true });
    await project.ExecTargetAsync("verify-docs");
    await project.ExecTargetAsync("catalog", new { Verify = true });
    await project.ExecTargetAsync("shim", new { Verify = true });
  }
}

// dotask: 1
// description: "Run available dependency, test, and formatting checks."
// remarks: "Runs _/dotnet/check, _/dotnet/test, and _/dotnet/format in order. Missing targets are skipped; failures stop verification."
// options:
//   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"], "default": "Release", "description": "Build configuration."}
// end-dotask
using DoTask;


public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var ran = 0;
    ran += await RunAsync(project, "_/dotnet/check");
    ran += await RunAsync(project, "_/dotnet/test", new { Configuration = project.Parameters.Get<string>("configuration") });
    ran += await RunAsync(project, "_/dotnet/format", new { Verify = true });
    if (ran == 0) {
      throw new TaskException("No verification targets found. Add dotnet/check, dotnet/test, or dotnet/format.");
    }
  }

  private static async Task<int> RunAsync( BuildContext project, string name, object? parameters = null )
  {
    var result = await project.ExecTargetIfExistsAsync(name, parameters);
    if (result.Status == TargetExecutionStatus.NotFound) {
      Console.WriteLine($"Skipping {name}: target not found.");
      return 0;
    }
    if (result.Status == TargetExecutionStatus.Failed) {
      if (result.ExitCode is { } exitCode) {
        throw new ProcessFailedException(name, exitCode);
      }
      throw new TaskException(result.Error ?? $"Target '{name}' failed.");
    }
    return 1;
  }
}

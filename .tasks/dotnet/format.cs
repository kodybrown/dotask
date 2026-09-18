using DoTask;

/// <summary>Format solution and task code, then run optional fixeol, or check formatting.</summary>
/// <option name="verify" alias="v" type="bool" default="false">Check code formatting without rewriting source or running fixeol.</option>
/// <option name="dotnet" default="dotnet">.NET CLI executable name or path.</option>
/// <requires tool="dotnet" />
/// <requires setting="solution" />
/// <example>dotask format</example>
/// <example>dotask format --verify</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var solution = project.Config.GetPath("solution");
    if (!File.Exists(solution)) {
      throw new TaskException($"Solution is missing: {solution}");
    }

    var dotnet = project.Parameters.Get<string>("dotnet");
    var verify = project.Parameters.Get<bool>("verify");
    List<string> solutionArguments = ["format", solution, "--verbosity", "detailed"];
    List<string> taskArguments = [
      "format", "whitespace", project.TaskDirectory, "--folder", "--verbosity", "minimal"
    ];
    if (verify) {
      solutionArguments.Add("--verify-no-changes");
      taskArguments.Add("--verify-no-changes");
    }

    await project.RunAsync(dotnet, solutionArguments);
    await project.RunAsync(dotnet, taskArguments);

    if (!verify) {
      var result = await project.ExecTargetIfExistsAsync("_/text/fixeol");
      if (result.Status == TargetExecutionStatus.Failed) {
        if (result.ExitCode is { } exitCode) {
          throw new ProcessFailedException("_/text/fixeol", exitCode);
        }
        throw new TaskException(result.Error ?? "Line-ending normalization failed.");
      }
    }
  }
}

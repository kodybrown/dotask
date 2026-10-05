// dotask: 1
// description: "Format solution and task code, then run optional fixeol, or check formatting."
// options:
//   - {"name": "verify", "alias": "v", "type": "bool", "default": "false", "description": "Check code formatting without rewriting source or running fixeol."}
//   - {"name": "dotnet", "default": "dotnet", "description": ".NET CLI executable name or path."}
// requires:
//   - {"kind": "tool", "value": "dotnet"}
//   - {"kind": "setting", "value": "solution"}
// examples: ["dotask format", "dotask format --verify"]
// end-dotask
using DoTask;


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

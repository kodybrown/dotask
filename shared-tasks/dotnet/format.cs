// dotask: 1
// description: "Format solution and task code, then run optional fixeol, or check formatting."
// options:
//   - {"name": "verify", "alias": "v", "type": "bool", "default": "false", "description": "Check code formatting without rewriting source or running fixeol."}
//   - {"name": "normalize-text", "type": "bool", "default": "true", "description": "Run optional recursive text normalization after formatting."}
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
    var solutions = project.Config.Contains("format.solutions")
      ? project.Config.Get<string[]>("format.solutions").Select(path => project.Path(path)).ToArray()
      : [project.Config.GetPath("solution")];
    if (solutions.Length == 0 || solutions.Any(path => !File.Exists(path)))
      throw new TaskException("Every configured format solution must exist, and at least one is required.");

    var dotnet = project.Parameters.Get<string>("dotnet");
    var verify = project.Parameters.Get<bool>("verify");
    List<string> solutionArguments = ["format", "", "--verbosity", "detailed"];
    List<string> taskArguments = [
      "format", "whitespace", project.TaskDirectory, "--folder", "--verbosity", "minimal"
    ];
    if (verify) {
      solutionArguments.Add("--verify-no-changes");
      taskArguments.Add("--verify-no-changes");
    }

    foreach (var solution in solutions) {
      solutionArguments[1] = solution;
      await project.RunAsync(dotnet, solutionArguments);
    }
    await project.RunAsync(dotnet, taskArguments);

    if (!verify && project.Parameters.Get<bool>("normalize-text")) {
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

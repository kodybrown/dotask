using DoTask;

/// <summary>Run the available prerequisite checks.</summary>
/// <example>dotask check</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    foreach (var target in new[] { "git/check", "dotnet/check" }) {
      var result = await project.ExecTargetIfExistsAsync(target);
      if (result.Status != TargetExecutionStatus.Failed) {
        continue;
      }

      if (result.ExitCode is int exitCode) {
        throw new ProcessFailedException(target, exitCode);
      }

      throw new TaskException(result.Error ?? $"Target '{target}' failed.");
    }
  }
}

// dotask: 1
// description: Build and test the configured solution with filtering, a timeout and a single-run process-tree guard.
// options:
//   - {name: configuration, alias: c, choices: [Debug, Release], default: Release}
//   - {name: solution, type: path, description: Solution or test project; otherwise settings.solution.}
//   - {name: filter, default: '', description: dotnet test filter expression.}
//   - {name: dotnet, default: dotnet, description: .NET CLI executable.}
//   - {name: no-build, type: bool, default: false, description: Use an existing build.}
//   - {name: timeout-seconds, type: int, default: 600, description: Maximum test command duration (1..86400 seconds).}
//   - {name: memory-limit-mb, type: int, default: 0, description: Aggregate process-tree memory cap; 0 disables it. Windows Job Object or Linux systemd user scope.}
//   - {name: lock-name, default: '', description: Single-run lock identity; otherwise derived from the project root.}
// requires: [{kind: tool, value: dotnet}, {kind: file, value: dotnet/_support/TestProcess.cs}]
// examples: [dotask test -c Release, 'dotask test --filter FullyQualifiedName~Tests --memory-limit-mb 4096']
// end-dotask
#:include _support/TestProcess.cs
using DoTask;
using DoTask.Shared.DotNet;

public static class Target
{
  public static void Main()
  {
    var project = BuildContext.Current;
    var solution = project.Parameters.Contains("solution") ? project.Parameters.GetPath("solution") : project.Config.GetPath("solution");
    if (!File.Exists(solution))
      throw new TaskException($"Solution does not exist: {solution}");
    List<string> arguments = ["test", solution, "-c", project.Parameters.Get<string>("configuration"),
      "--disable-build-servers"];
    if (project.Parameters.Get<bool>("no-build"))
      arguments.Add("--no-build");
    var filter = project.Parameters.Get<string>("filter");
    if (!string.IsNullOrWhiteSpace(filter))
      arguments.AddRange(["--filter", filter]);
    var code = TestProcess.Run(project.Parameters.Get<string>("dotnet"), arguments, project.RootDirectory,
      project.Parameters.Get<int>("timeout-seconds"), project.Parameters.Get<int>("memory-limit-mb"),
      project.Parameters.Get<string>("lock-name"), project.CancellationToken);
    if (code != 0)
      throw new ProcessFailedException("dotnet test", code);
  }
}

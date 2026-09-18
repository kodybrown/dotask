using DoTask;

/// <summary>Check the solution, .NET SDK, MSBuild, and formatter prerequisites.</summary>
/// <requires tool="dotnet" />
/// <requires setting="solution" />
/// <remarks>Reports installed tool versions using the project's SDK selection.</remarks>
/// <example>dotask dotnet-check</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;

    var solution = project.Config.GetPath("solution");
    if (!File.Exists(solution)) {
      throw new TaskException($"Solution is missing: {solution}");
    }

    await CheckAsync(project, ".NET SDK", "--version");
    await CheckAsync(project, "MSBuild", "msbuild", "-version", "-nologo");
    await CheckAsync(project, ".NET formatter", "format", "--version");
  }

  private static async Task CheckAsync( BuildContext project, string name, params string[] arguments )
  {
    var result = await project.RunAsync(new ProcessDefinition {
      Executable = "dotnet",
      Arguments = arguments,
      CaptureOutput = true,
      ThrowOnError = false
    });
    if (result.ExitCode != 0) {
      var details = (result.StandardOutput + "\n" + result.StandardError).Trim();
      throw new TaskException($"{name} check failed (exit {result.ExitCode}). Ensure the required .NET SDK/tools are installed and check global.json.\n{details}");
    }
    var version = result.StandardOutput.Trim();
    if (version.Length == 0) {
      throw new TaskException($"{name} did not report a version. Check the dotnet installation on PATH.");
    }
    Console.WriteLine($"{name}: {version}");
  }
}

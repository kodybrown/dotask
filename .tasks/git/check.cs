using DoTask;

/// <summary>Check that Git is available.</summary>
/// <requires tool="git" />
/// <example>dotask git-check</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var git = await project.RunAsync(new ProcessDefinition
    {
      Executable = "git",
      Arguments = ["--version"],
      CaptureOutput = true
    });
    Console.WriteLine($"OK: {git.StandardOutput.Trim()}");
  }
}

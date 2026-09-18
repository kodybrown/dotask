using DoTask;

/// <summary>Check that Git is available, optionally checking repository whitespace.</summary>
/// <option name="whitespace" type="bool" default="false">Check staged and unstaged diffs for whitespace errors.</option>
/// <requires tool="git" />
/// <example>dotask git-check</example>
/// <example>dotask git/check --whitespace</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var git = await project.RunAsync(new ProcessDefinition {
      Executable = "git",
      Arguments = ["--version"],
      CaptureOutput = true
    });
    Console.WriteLine($"OK: {git.StandardOutput.Trim()}");
    if (project.Parameters.Get<bool>("whitespace")) {
      await project.RunAsync("git", ["diff", "--check"]);
      await project.RunAsync("git", ["diff", "--cached", "--check"]);
      Console.WriteLine("Git diffs have no whitespace errors.");
    }
  }
}

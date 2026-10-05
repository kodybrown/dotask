// dotask: 1
// description: "Check that Git is available, optionally checking repository whitespace."
// options:
//   - {"name": "whitespace", "type": "bool", "default": "false", "description": "Check staged and unstaged diffs for whitespace errors."}
// requires:
//   - {"kind": "tool", "value": "git"}
// examples: ["dotask git-check", "dotask git/check --whitespace"]
// end-dotask
using DoTask;


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

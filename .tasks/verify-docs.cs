using DoTask;

/// <summary>Check required documentation and Git whitespace errors.</summary>
/// <requires task="git/check" />
/// <remarks>Checks required files, then calls _/git/check --whitespace for staged/unstaged diffs; does not validate links or execute documentation examples.</remarks>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    string[] documents = [
      "README.md", "LICENSE.md", "AGENTS.md", "docs/README.md", "docs/USAGE.md",
      "docs/SHARED-TASKS.md", "docs/INSTALLATION.md", "docs/TARGETS.md", "docs/AI-ASSISTANTS.md",
      "docs/DESIGN.md", "docs/VERIFICATION.md", "docs/CHANGELOG.md", "examples/basic/README.md"
    ];
    foreach (var document in documents) {
      if (!File.Exists(project.Path(document))) {
        throw new TaskException($"Required document is missing: {document}");
      }
    }
    await project.ExecTargetAsync("_/git/check", new { Whitespace = true });
    Console.WriteLine("Required documentation exists and Git diffs have no whitespace errors.");
  }
}

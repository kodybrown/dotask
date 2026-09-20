using DoTask.Cli;
using DoTask.Cli.Completion;
using DoTask.Cli.Parsing;

namespace DoTask.Tests;

public sealed class VerboseTests
{
  [Theory]
  [InlineData("")]
  [InlineData("help")]
  [InlineData("--verbose")]
  [InlineData("help --verbose")]
  [InlineData("--verbose help")]
  public async Task SummaryShowsOnlyTargetsUnlessVerbose( string command )
  {
    using var project = new TestProject();
    project.Write(".dotasks.yaml", "name: Project identity\ndescription: Project description\nsettings: {message: hello}");
    project.Target("run", "DoesNotCompile();", "/// <summary>Run things.</summary>\n/// <option name=\"mode\" default=\"Debug\" />");
    using var output = new StringWriter();
    using var error = new StringWriter();
    Assert.Equal(0, await CliApplication.RunAsync(command.Split(' ', StringSplitOptions.RemoveEmptyEntries), project.Root, output, error));
    var text = output.ToString();
    Assert.Contains("Targets:", text);
    Assert.Contains("Run things.", text);
    Assert.EndsWith("See `dotask help <target>` for detailed information on each target.\n", text);
    Assert.Empty(error.ToString());
    foreach (var detail in new[] { "Project identity", "Project description", "Settings:", "Target options:" }) {
      Assert.Equal(command.Contains("--verbose"), text.Contains(detail));
    }
    if (!command.Contains("--verbose")) {
      Assert.StartsWith("Targets:\n", text);
    }
  }

  [Fact]
  public async Task VerboseFlowsThroughGroupsAndCSharpNestedCallsWithoutChangingStdout()
  {
    using var project = new TestProject();
    project.Target("caller", "await BuildContext.Current.ExecTargetAsync(\"group\");", async: true);
    project.Write(".tasks/group.task", "steps: [{run: absent, optional: true}, {run: child}]");
    project.Target("child", "Console.WriteLine(\"task output\");");
    var normal = await project.RunAsync("caller");
    var verbose = await project.RunAsync("caller", "--verbose");
    Assert.Equal(0, normal.ExitCode);
    Assert.Equal(0, verbose.ExitCode);
    Assert.Equal(normal.StandardOutput, verbose.StandardOutput);
    Assert.DoesNotContain("[dotask]", normal.StandardError);
    Assert.Contains("Compiling: caller", verbose.StandardError);
    Assert.Contains("Skipping absent optional target: absent", verbose.StandardError);
    Assert.Contains("Executing: child", verbose.StandardError);
    Assert.Contains("Completed group: group", verbose.StandardError);
  }

  [Fact]
  public void VerboseIsGlobalAndCompletionWorksBeforeAndAfterIt()
  {
    var command = CommandLine.Parse(["--verbose", "run", "-v", "true", "--verbose"]);
    Assert.True(command.Verbose);
    Assert.Equal("run", command.Command);
    Assert.Equal(["-v", "true"], command.Arguments);
    using var project = new TestProject();
    project.Target("run");
    Assert.Contains(CompletionEngine.Complete("dotask --verbose r", project.Root), c => c.Value == "run");
    Assert.Contains(CompletionEngine.Complete("dotask help --ver", project.Root), c => c.Value == "--verbose");
  }
}

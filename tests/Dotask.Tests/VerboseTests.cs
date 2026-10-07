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
    project.Target("run", "DoesNotCompile();", "// dotask: 1\n// description: \"Run things.\"\n// options:\n//   - {\"name\": \"mode\", \"default\": \"Debug\"}\n// end-dotask\n");
    using var output = new StringWriter();
    using var error = new StringWriter();
    Assert.Equal(0, await NativeCli.RunAsync(command.Split(' ', StringSplitOptions.RemoveEmptyEntries), project.Root, output, error));
    var text = output.ToString();
    Assert.Contains("Targets:", text);
    Assert.Contains("Run things.", text);
    Assert.EndsWith("See `dotask help <target>` for detailed information on each target." + "\n", text);
    Assert.Empty(error.ToString());
    foreach (var detail in new[] { "Project identity", "Project description", "Settings:", "Target options:" }) {
      Assert.Equal(command.Contains("--verbose"), text.Contains(detail));
    }
    if (!command.Contains("--verbose")) {
      Assert.StartsWith("Targets:" + "\n", text);
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
  public async Task VerboseIsGlobalAndCompletionWorksBeforeAndAfterIt()
  {
    using var project = new TestProject();
    project.Target("run", "Console.WriteLine(BuildContext.Current.Parameters.Get<bool>(\"flag\"));",
      "// dotask: 1\n// options: [{name: flag, alias: v, type: bool}]\n// end-dotask");
    var result = await project.RunAsync("--verbose", "run", "-v", "true", "--verbose");
    Assert.Equal(0, result.ExitCode);
    Assert.Contains("True", result.StandardOutput);
    Assert.Contains(NativeCompletion.Complete("dotask --verbose r", project.Root), c => c.Value == "run");
    Assert.Contains(NativeCompletion.Complete("dotask help --ver", project.Root), c => c.Value == "--verbose");
  }
}

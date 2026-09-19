using DoTask.Cli;
using DoTask.Cli.Authoring;
using DoTask.Cli.Completion;
using DoTask.Cli.Discovery;
using DoTask.Cli.Metadata;

namespace DoTask.Tests;

public sealed class TaskWizardTests
{
  private static async Task<string> Run( TestProject project, params string[] answers )
  {
    using var input = new StringReader(string.Join('\n', answers) + "\n");
    using var output = new StringWriter();
    await new TaskWizard(input, output, CancellationToken.None).RunAsync(TaskDirectory.Locate(project.Root));
    return output.ToString();
  }

  [Fact]
  public async Task KnownTargetsUseCanonicalNamesAndOnlyExplicitParameters()
  {
    using var project = new TestProject();
    project.Target("_/dotnet/check", "throw new Exception(\"Must not run\");", """
      /// <summary>Inspect project.</summary>
      /// <option name="configuration" choices="Debug,Release" default="Debug" />
      /// <option name="count" type="int" />
      /// <option name="flag" type="bool" />
      """);
    project.Write(".dotasks.yaml", "targets:\n  _/dotnet/check:\n    defaults: {configuration: Release}\n");
    var output = await Run(project, "verify", "Verify things.", "y", "Inspect", "1", "n", "", "invalid", "3", "true", "n", "y", "y");
    var target = new TargetCatalog(project.Tasks).Get("verify");
    var group = Assert.IsType<TaskGroup>(target.Group);
    Assert.True(group.RequireAtLeastOneStep);
    var step = Assert.Single(group.Steps);
    Assert.Equal("_/dotnet/check", step.Run);
    Assert.False(step.Optional);
    Assert.False(step.Parameters.TryGetProperty("configuration", out _));
    Assert.Equal(3, step.Parameters.GetProperty("count").GetInt32());
    Assert.True(step.Parameters.GetProperty("flag").GetBoolean());
    Assert.Contains("Default: Release", output);
    Assert.Contains("Invalid int", output);
    Assert.Contains("YAML preview:", output);
    Assert.False(Directory.Exists(Path.Combine(project.Tasks, ".dotask")));
  }

  [Fact]
  public async Task ManualParametersAndReorderingRoundTripSafely()
  {
    using var project = new TestProject();
    var output = await Run(project, "checks", "Description: #quoted", "y", ":manual", "future/check", "y",
      "y", "message", "", "a: # b", "y", "enabled", "bool", "false", "n",
      "y", ":manual", "later/run", "y", "n", "n", "1,1", "2,1", "n", "y");
    var group = new TargetCatalog(project.Tasks).Get("checks").Group!;
    Assert.False(group.RequireAtLeastOneStep);
    Assert.Equal("later/run", group.Steps[0].Run);
    Assert.Equal("a: # b", group.Steps[1].Parameters.GetProperty("message").GetString());
    Assert.False(group.Steps[1].Parameters.GetProperty("enabled").GetBoolean());
    Assert.All(group.Steps, step => Assert.True(step.Optional));
    Assert.Contains("exactly once", output);
  }

  [Theory]
  [InlineData(":cancel")]
  [InlineData("new\n:cancel")]
  [InlineData("new\n\ny\n:cancel")]
  [InlineData("new\n\nn\nn\n:cancel")]
  public async Task CancellationNeverWritesAFile( string answers )
  {
    using var project = new TestProject();
    await Assert.ThrowsAnyAsync<OperationCanceledException>(() => Run(project, answers));
    Assert.Empty(Directory.GetFileSystemEntries(project.Tasks));
  }

  [Fact]
  public async Task EofAndDecliningSaveLeaveNoFiles()
  {
    using var project = new TestProject();
    using var output = new StringWriter();
    await Assert.ThrowsAnyAsync<OperationCanceledException>(() => new TaskWizard(new StringReader(""), output, CancellationToken.None)
      .RunAsync(TaskDirectory.Locate(project.Root)));
    Assert.Contains("Cancelled", await Run(project, "new", "", "n", "n", "n"));
    Assert.Empty(Directory.GetFileSystemEntries(project.Tasks));
  }

  [Fact]
  public async Task InvalidNamesAndExistingTargetsAreNotOverwritten()
  {
    using var project = new TestProject();
    var file = project.Target("existing");
    var original = File.ReadAllText(file);
    project.Write(".tasks/kept.task", "steps: []");
    var output = await Run(project, "../escape", "help", "existing", "kept", "new", "", "n", "n", "y");
    Assert.Equal(original, File.ReadAllText(file));
    Assert.Equal("steps: []", File.ReadAllText(Path.Combine(project.Tasks, "kept.task")));
    Assert.NotNull(new TargetCatalog(project.Tasks).Get("new").Group);
    Assert.Contains("already exists", output);
    Assert.Contains("reserved", output);
  }

  [Fact]
  public async Task EmptyStringAndPortablePathParametersArePreserved()
  {
    using var project = new TestProject();
    project.Target("run", metadata: """
      /// <option name="message" required="true" />
      /// <option name="output" type="path" />
      """);
    await Run(project, "new", "", "y", "run", "1", "n", ":empty", "./artifacts/output", "n", "n", "y");
    var parameters = new TargetCatalog(project.Tasks).Get("new").Group!.Steps[0].Parameters;
    Assert.Equal("", parameters.GetProperty("message").GetString());
    Assert.Equal("./artifacts/output", parameters.GetProperty("output").GetString());
  }

  [Fact]
  public async Task HelpAndCompletionAreAvailableWithoutAProject()
  {
    using var project = new TestProject();
    Directory.Delete(project.Tasks);
    using var output = new StringWriter();
    using var error = new StringWriter();
    Assert.Equal(0, await CliApplication.RunAsync(["--create-task", "--help"], project.Root, output, error));
    Assert.Contains("preview", output.ToString());
    Assert.Contains(CompletionEngine.Complete("dotask --create", project.Root), item => item.Value == "--create-task");
    Assert.Contains(CompletionEngine.Complete("dotask --create-task --", project.Root), item => item.Value == "--use-dir");
    Assert.False(Directory.Exists(project.Tasks));
  }

  [Fact]
  public async Task RedirectedInputFailsWithGuidance()
  {
    using var project = new TestProject();
    var result = await project.RunAsync("--create-task");
    Assert.Equal(1, result.ExitCode);
    Assert.Contains("interactive terminal", result.StandardError);
    Assert.Empty(Directory.GetFileSystemEntries(project.Tasks));
  }

  [Fact]
  public async Task CancellationInterruptsABlockedTerminalRead()
  {
    using var project = new TestProject();
    using var input = new BlockingReader();
    using var output = new StringWriter();
    using var cancellation = new CancellationTokenSource();
    var wizard = new TaskWizard(input, output, cancellation.Token).RunAsync(TaskDirectory.Locate(project.Root));
    await input.Started.Task.WaitAsync(TimeSpan.FromSeconds(5));
    try {
      cancellation.Cancel();
      await Assert.ThrowsAnyAsync<OperationCanceledException>(() => wizard.WaitAsync(TimeSpan.FromSeconds(5)));
      Assert.Empty(Directory.GetFileSystemEntries(project.Tasks));
    } finally {
      input.Release.Set();
    }
  }

  private sealed class BlockingReader : TextReader
  {
    public TaskCompletionSource Started { get; } = new(TaskCreationOptions.RunContinuationsAsynchronously);
    public ManualResetEventSlim Release { get; } = new();
    public override string? ReadLine()
    {
      Started.SetResult();
      Release.Wait();
      return null;
    }
  }

  [Fact]
  public async Task SymlinkDestinationCannotBeOverwritten()
  {
    if (OperatingSystem.IsWindows()) {
      return;
    }
    using var project = new TestProject();
    var outside = project.Write("outside", "keep");
    File.CreateSymbolicLink(Path.Combine(project.Tasks, "linked.task"), outside);
    var output = await Run(project, "linked", "safe", "", "n", "n", "y");
    Assert.Contains("symbolic", output);
    Assert.Equal("keep", File.ReadAllText(outside));
  }
}

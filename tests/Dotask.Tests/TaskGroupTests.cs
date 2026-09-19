using DoTask.Cli;
using DoTask.Cli.Completion;
using DoTask.Cli.Configuration;
using DoTask.Cli.Discovery;
using DoTask.Cli.Execution;

namespace DoTask.Tests;

public sealed class TaskGroupTests
{
  [Theory]
  [InlineData("steps: []", 0)]
  [InlineData("steps: [{run: missing, optional: true}]", 0)]
  [InlineData("require_at_least_1_step: true\nsteps: []", 1)]
  [InlineData("require_at_least_1_step: true\nsteps: [{run: missing, optional: true}]", 1)]
  [InlineData("steps: [{run: missing}]", 1)]
  public async Task EmptyAndMissingStepsFollowTheDeclaredPolicy( string yaml, int expected )
  {
    using var project = new TestProject();
    project.Write(".tasks/check.task", yaml);
    using var output = new StringWriter();
    using var error = new StringWriter();
    var code = await CliApplication.RunAsync(["check"], project.Root, output, error);
    Assert.Equal(expected, code);
    Assert.Empty(output.ToString());
    if (expected == 0) {
      Assert.Empty(error.ToString());
    } else {
      Assert.NotEmpty(error.ToString());
    }
  }

  [Theory]
  [InlineData("steps: [{run: test, optional: yes}]")]
  [InlineData("steps: [{run: test, optional: 'true'}]")]
  [InlineData("steps: [{run: test, optionl: true}]")]
  [InlineData("steps: [{optional: true}]")]
  [InlineData("steps: [{run: ''}]")]
  [InlineData("steps: [{run: 2}]")]
  [InlineData("steps: [{run: test, with: []}]")]
  [InlineData("steps: [{run: test, with: {value: null}}]")]
  [InlineData("steps: [{run: test, with: {value: [a, b]}}]")]
  [InlineData("steps: [{run: test, with: {value: a, VALUE: b}}]")]
  [InlineData("steps: []\nsteps: []")]
  [InlineData("steps: []\n---\nsteps: []")]
  [InlineData("require_at_least_1_step: 'false'\nsteps: []")]
  [InlineData("description: 12\nsteps: []")]
  [InlineData("steps: {}")]
  [InlineData("{}")]
  [InlineData("steps: [broken")]
  [InlineData("steps: &loop [*loop]")]
  public void MalformedGroupsHaveMetadataErrors( string yaml )
  {
    using var project = new TestProject();
    project.Write(".tasks/check.task", yaml);
    Assert.NotNull(Assert.Single(new TargetCatalog(project.Tasks).Targets).Error);
  }

  [Fact]
  public async Task HelpAndCompletionDescribeGroupsWithoutExecutingChildren()
  {
    using var project = new TestProject();
    project.Write(".tasks/tools/check.task", "description: Check everything.\nsteps: [{run: missing, optional: true, with: {flag: true}}]");
    using var output = new StringWriter();
    using var error = new StringWriter();
    Assert.Equal(0, await CliApplication.RunAsync(["help", "check"], project.Root, output, error));
    Assert.Contains("Check everything.", output.ToString());
    Assert.Contains("./.tasks/tools/check.task", output.ToString());
    Assert.Contains("missing (optional)", output.ToString());
    Assert.Contains("with flag: true", output.ToString());
    Assert.Contains(CompletionEngine.Complete("dotask tools/c", project.Root), c => c.Value == "tools/check");
    project.Target("tools/check");
    Assert.Throws<TaskException>(() => new TargetCatalog(project.Tasks).Get("tools/check"));
  }

  [Fact]
  public async Task OrderedStepsPassParametersOverrideDefaultsAndRepeat()
  {
    using var project = new TestProject();
    project.Write(".dotasks.yaml", "targets:\n  write:\n    defaults: {message: default, count: 9, enabled: false}\n");
    project.Target("write", """
      var p = BuildContext.Current.Parameters;
      File.AppendAllText("order", $"{p.Get<string>("message")}|{p.Get<int>("count")}|{p.Get<bool>("enabled")};");
      """, """
      /// <option name="message" />
      /// <option name="count" type="int" />
      /// <option name="enabled" type="bool" />
      """);
    project.Write(".tasks/check.task", """
      require_at_least_1_step: true
      steps:
        - run: absent
          optional: true
        - run: write
          with: {message: 'hello = world', count: 2, enabled: true}
        - run: write
          with: {message: '007'}
      """);
    var result = await project.RunAsync("check");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Equal("hello = world|2|True;007|9|False;", File.ReadAllText(Path.Combine(project.Root, "order")));
  }

  [Fact]
  public async Task OptionalFailureStopsLaterStepsAndPreservesExitCode()
  {
    using var project = new TestProject();
    project.Target("fail", "Environment.Exit(7);");
    project.Target("later", "File.WriteAllText(\"unexpected\", \"ran\");");
    project.Write(".tasks/check.task", "steps: [{run: fail, optional: true}, {run: later}]");
    var result = await project.RunAsync("check");
    Assert.Equal(7, result.ExitCode);
    Assert.False(File.Exists(Path.Combine(project.Root, "unexpected")));
  }

  [Theory]
  [InlineData("steps: [{run: check, optional: true}]", "cycle")]
  [InlineData("steps: [{run: bad, optional: true}]", "Metadata error")]
  [InlineData("steps: [{run: same, optional: true}]", "Ambiguous")]
  [InlineData("steps: [{run: empty, with: {unknown: true}}]", "Unknown option")]
  public async Task OptionalOnlySuppressesAbsence( string yaml, string diagnostic )
  {
    using var project = new TestProject();
    project.Write(".tasks/check.task", yaml);
    project.Write(".tasks/bad.task", "steps: invalid");
    project.Write(".tasks/empty.task", "steps: []");
    project.Write(".tasks/one/same.task", "steps: []");
    project.Write(".tasks/two/same.task", "steps: []");
    using var output = new StringWriter();
    using var error = new StringWriter();
    Assert.Equal(1, await CliApplication.RunAsync(["check"], project.Root, output, error));
    Assert.Contains(diagnostic, error.ToString());
  }

  [Fact]
  public async Task NestedGroupsCountAsStepsAndCSharpCallsShareCycleDetection()
  {
    using var project = new TestProject();
    project.Write(".tasks/empty.task", "steps: []");
    project.Write(".tasks/check.task", "require_at_least_1_step: true\nsteps: [{run: empty}]");
    project.Target("caller", "await BuildContext.Current.ExecTargetAsync(\"check\");", async: true);
    Assert.Equal(0, (await project.RunAsync("caller")).ExitCode);
    project.Write(".tasks/check.task", "steps: [{run: caller}]");
    var result = await project.RunAsync("caller");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Contains("caller -> check -> caller", result.StandardError);
  }

  [Fact]
  public async Task OptionalCompilationErrorsAreNotSkipped()
  {
    using var project = new TestProject();
    project.Write(".tasks/broken.cs", "this is not valid C#");
    project.Write(".tasks/check.task", "steps: [{run: broken, optional: true}]");
    var result = await project.RunAsync("check");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Contains("Compilation failed", result.StandardError);
  }

  [Fact]
  public void SharedSourceEnumerationStillExcludesProjectGroups()
  {
    using var project = new TestProject();
    project.Target("tools/run");
    project.Write(".tasks/tools/group.task", "steps: []");
    Assert.Single(TargetCatalog.EnumerateSources(project.Tasks, false));
    Assert.Equal(2, new TargetCatalog(project.Tasks).Targets.Count);
  }

  [Fact]
  public async Task CancellationIsHonoredEvenForEmptyGroups()
  {
    using var project = new TestProject();
    project.Write(".tasks/check.task", "steps: []");
    var directory = TaskDirectory.Locate(project.Root);
    var executor = new TargetExecutor(directory, ProjectConfiguration.Empty, new TargetCompiler());
    await Assert.ThrowsAnyAsync<OperationCanceledException>(() => executor.ExecuteAsync(
      new TargetCatalog(project.Tasks).Get("check"), [], new CancellationToken(true)));
  }
}

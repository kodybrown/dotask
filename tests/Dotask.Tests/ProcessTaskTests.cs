using System.Text.Json;

namespace DoTask.Tests;

public sealed class ProcessTaskTests
{
  [Fact]
  public async Task ProcessValuesPreserveArgumentBoundariesAndResolveCwdFromProjectRoot()
  {
    using var project = new TestProject();
    var probe = await project.BuildProcessProbeAsync("Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new { Args = args, Cwd = Environment.CurrentDirectory }));");
    Directory.CreateDirectory(Path.Combine(project.Root, "work dir"));
    project.Write(".dotasks.yaml", "settings: { executable: dotnet }\n");
    project.Write(".tasks/probe.task", $$"""
      options:
        - {name: message, alias: m, required: true}
        - {name: directory, type: path, default: 'work dir'}
      steps:
        - exec:
            executable: {setting: executable}
            arguments: [{{JsonSerializer.Serialize(probe)}}, {parameter: message}, '', 'back\slash\', 'a=b=c', '$(touch BAD)', '日本語']
            cwd: {parameter: directory}
      """);
    Directory.CreateDirectory(Path.Combine(project.Root, "nested"));
    var result = await project.RunFromAsync(Path.Combine(project.Root, "nested"), "probe", "-m", "two words\nwith a \"quote\"");
    Assert.True(result.ExitCode == 0, result.StandardError);
    using var data = JsonDocument.Parse(result.StandardOutput);
    Assert.Equal(["two words\nwith a \"quote\"", "", "back\\slash\\", "a=b=c", "$(touch BAD)", "日本語"],
      data.RootElement.GetProperty("Args").EnumerateArray().Select(v => v.GetString()!).ToArray());
    Assert.Equal(Path.Combine(project.Root, "work dir"), data.RootElement.GetProperty("Cwd").GetString());
    Assert.False(File.Exists(Path.Combine(project.Root, "work dir", "BAD")));
  }

  [Fact]
  public async Task ConditionsDefaultOverridesAndFailureStopTheSequenceWithoutTaskCompilation()
  {
    using var project = new TestProject();
    project.Write(".tasks/check.task", """
      options: [{name: enabled, type: bool, default: false}]
      steps:
        - when: {parameter: enabled}
          exec: {executable: deliberately-missing-process}
        - run: missing
          optional: true
      """);
    Assert.Equal(0, (await project.RunAsync("check")).ExitCode);
    Assert.Equal(1, (await project.RunAsync("check", "--enabled")).ExitCode);
    project.Write(".dotasks.yaml", "targets: {check: {defaults: {enabled: true}}}");
    Assert.Equal(1, (await project.RunAsync("check")).ExitCode);
    Assert.Equal(0, (await project.RunAsync("check", "enabled=false")).ExitCode);
    project.Write(".dotasks.yaml", "settings: {}\n");
    var probe = await project.BuildProcessProbeAsync("Console.WriteLine(\"stdout\"); Console.Error.WriteLine(\"stderr\"); Environment.Exit(19);");
    project.Write(".tasks/check.task", $$"""
      steps:
        - exec: {executable: dotnet, arguments: [{{JsonSerializer.Serialize(probe)}}]}
        - exec: {executable: must-not-start}
      """);
    var failed = await project.RunAsync("check");
    Assert.Equal(19, failed.ExitCode);
    Assert.Contains("stdout", failed.StandardOutput);
    Assert.Contains("stderr", failed.StandardError);
    Assert.DoesNotContain("must-not-start", failed.StandardError);
  }

  [Fact]
  public async Task HelpCompletionAndSharedYamlTasksNeedNoLanguageToolchainAndPreserveTracking()
  {
    using var project = new TestProject();
    var privateFile = project.Write("private/tools/check.task", """
      description: A reusable process task.
      options: [{name: mode, choices: [one, two], default: one}]
      steps: [{exec: {executable: git, arguments: [--version]}}]
      """);
    var options = new SharedTaskOptions(Path.Combine(project.Root, "cache"), Path.Combine(project.Root, "private"));
    var add = await RustArtifacts.Run(project.Root, ["--add", "private-tasks/tools/check.task"], options);
    Assert.True(add.ExitCode == 0, add.StandardError);
    Assert.Equal(File.ReadAllBytes(privateFile), File.ReadAllBytes(Path.Combine(project.Tasks, "private-tasks/tools/check.task")));
    var help = await project.RunAsync("help", "check");
    Assert.Equal(0, help.ExitCode);
    Assert.Contains("exec:", help.StandardOutput);
    Assert.Contains("--mode", help.StandardOutput);
    project.Write(".dotasks.yaml", "invalid: [");
    Assert.Contains(NativeCompletion.Complete("dotask check --mode t", project.Root), c => c.Value == "two");
    project.Write(".dotasks.yaml", "settings: {}\n");
    var run = await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = RustArtifacts.Binary("dotask"),
      Arguments = ["check"],
      WorkingDirectory = project.Root,
      CaptureOutput = true,
      ThrowOnError = false,
      Environment = new Dictionary<string, string?> { ["DOTNET_HOST_PATH"] = "not-a-host", ["DOTASK_SOURCE_RUST_SDK"] = "not-an-sdk" }
    }, CancellationToken.None);
    Assert.True(run.ExitCode == 0, run.StandardError);
    Assert.Contains("git version", run.StandardOutput);
    var tracking = project.Write(".dotasks-lock.yaml", File.ReadAllText(Path.Combine(project.Root, ".dotasks-lock.yaml")) + "# preserved\n");
    var before = File.ReadAllBytes(tracking);
    Assert.Equal(0, (await RustArtifacts.Run(project.Root, ["--sync"], options)).ExitCode);
    Assert.Equal(before, File.ReadAllBytes(tracking));
    File.AppendAllText(Path.Combine(project.Tasks, "private-tasks/tools/check.task"), "# local edit\n");
    Assert.Equal(1, (await RustArtifacts.Run(project.Root, ["--remove", "private-tasks/tools/check.task"], options)).ExitCode);
    Assert.Equal(before, File.ReadAllBytes(tracking));
  }

  [Theory]
  [InlineData("steps: [{run: x, exec: {executable: git}}]")]
  [InlineData("steps: [{exec: {arguments: []}}]")]
  [InlineData("steps: [{exec: {executable: ''}}]")]
  [InlineData("steps: [{exec: {executable: git, arguments: '--version'}}]")]
  [InlineData("steps: [{exec: {executable: git, arguments: [null]}}]")]
  [InlineData("steps: [{exec: {executable: git, arguments: [{parameter: unknown}]}}]")]
  [InlineData("steps: [{exec: {executable: git}, optional: true}]")]
  [InlineData("steps: [{exec: {executable: git}, with: {x: 1}}]")]
  [InlineData("steps: [{exec: {executable: git, cwd: null}}]")]
  [InlineData("steps: [{exec: {executable: git, cwd: ''}}]")]
  [InlineData("steps: [{exec: {executable: git}, when: true}]")]
  [InlineData("options: [{name: x}]\nsteps: [{exec: {executable: git}, when: {parameter: x}}]")]
  [InlineData("options: [{name: help}]\nsteps: []")]
  public async Task InvalidProcessMetadataFailsBeforeExecution( string yaml )
  {
    using var project = new TestProject();
    project.Write(".tasks/check.task", yaml);
    var result = await project.RunAsync("check");
    Assert.Equal(1, result.ExitCode);
    Assert.Contains("Metadata error", result.StandardError);
  }
}

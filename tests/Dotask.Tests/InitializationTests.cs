using DoTask.Cli;
using DoTask.Cli.Completion;
using DoTask.Cli.Configuration;
using DoTask.Cli.Execution;

namespace DoTask.Tests;

public sealed class InitializationTests
{
  [Theory]
  [InlineData("MyProject")]
  [InlineData("true")]
  [InlineData("2026")]
  [InlineData("O'Brien 日本語")]
  public async Task InitCreatesUsableLocalProjectWithQuotedDirectoryNameAndSafeReruns( string name )
  {
    using var parent = new TestProject();
    // Initialization must not discover or load this parent project.
    var parentConfig = parent.Write(".dotasks.yaml", "invalid: [\n");
    var root = Path.Combine(parent.Root, name);
    Directory.CreateDirectory(root);
    var result = await parent.RunFromAsync(root, "--init");
    Assert.Equal(0, result.ExitCode);
    Assert.Contains("Created .dotasks.yaml.", result.StandardOutput);
    Assert.Contains("dotask --add GROUP/TASK", result.StandardOutput);
    var tasks = Path.Combine(root, ".tasks");
    var configPath = Path.Combine(root, ".dotasks.yaml");
    Assert.Empty(Directory.EnumerateFileSystemEntries(tasks));
    Assert.False(File.Exists(Path.Combine(root, ".dotasks-lock.yaml")));
    var config = ProjectConfiguration.Load(tasks, root);
    Assert.Equal(name, config.Name);
    Assert.Null(config.Description);
    Assert.Empty(config.Settings.EnumerateObject());
    Assert.Empty(config.TargetDefaults.EnumerateObject());
    var summary = await parent.RunFromAsync(root);
    Assert.Equal(0, summary.ExitCode);
    Assert.StartsWith(name + Environment.NewLine, summary.StandardOutput);
    Assert.Contains("(no C# targets)", summary.StandardOutput);
    var original = File.ReadAllBytes(configPath);
    File.SetLastWriteTimeUtc(configPath, new DateTime(2001, 1, 1, 0, 0, 0, DateTimeKind.Utc));
    var written = File.GetLastWriteTimeUtc(configPath);
    var again = await parent.RunFromAsync(root, "--INIT");
    Assert.Equal(0, again.ExitCode);
    Assert.Contains("Kept existing .dotasks.yaml unchanged.", again.StandardOutput);
    Assert.Equal(original, File.ReadAllBytes(configPath));
    Assert.Equal(written, File.GetLastWriteTimeUtc(configPath));
    Assert.Equal("invalid: [\n", File.ReadAllText(parentConfig));
    Assert.Equal(2, Directory.EnumerateFileSystemEntries(root).Count());
  }

  [Fact]
  public async Task InitPreservesExistingTasksConfigurationCommentsAndLockFile()
  {
    using var project = new TestProject();
    var task = project.Target("init", "DoesNotCompile();", "/// <summary>A handwritten task.</summary>");
    var source = File.ReadAllBytes(task);
    var first = await project.RunAsync("--init");
    Assert.Equal(0, first.ExitCode);
    Assert.Equal(source, File.ReadAllBytes(task));
    var config = project.Write(".dotasks.yaml", "# Keep this comment\nversion: 1\nname: Custom\nsettings: { answer: 42 }\n");
    var original = File.ReadAllBytes(config);
    var tracking = project.Write(".dotasks-lock.yaml", "user-owned tracking\n");
    File.Delete(task);
    Directory.Delete(project.Tasks);
    var second = await project.RunAsync("--init");
    Assert.Equal(0, second.ExitCode);
    Assert.True(Directory.Exists(project.Tasks));
    Assert.Equal(original, File.ReadAllBytes(config));
    Assert.Equal("user-owned tracking\n", File.ReadAllText(tracking));
  }

  [Fact]
  public async Task InitNeedsNoSdkOrSharedTaskSourcesAndDoesNotExecuteTasks()
  {
    using var project = new TestProject();
    var task = project.Target("init", "File.WriteAllText(\"executed\", \"bad\");");
    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(30));
    var result = await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = DotnetHost.Find(),
      Arguments = [typeof(CliApplication).Assembly.Location, "--init"],
      WorkingDirectory = project.Root,
      Environment = new Dictionary<string, string?> {
        ["DOTNET_HOST_PATH"] = Path.Combine(project.Root, "missing-sdk"),
        ["DOTASK_ONLINE_TASKS"] = Path.Combine(project.Root, "missing-online-source"),
        ["DOTASK_CACHE_HOME"] = Path.Combine(project.Root, "cache"),
        ["DOTASK_PRIVATE_TASKS"] = Path.Combine(project.Root, "private")
      },
      CaptureOutput = true,
      ThrowOnError = false
    }, timeout.Token);
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.False(File.Exists(Path.Combine(project.Root, "executed")));
    Assert.False(Directory.Exists(Path.Combine(project.Root, "cache")));
    Assert.False(Directory.Exists(Path.Combine(project.Root, "private")));
    var help = await project.RunAsync("init", "--help");
    Assert.Equal(0, help.ExitCode);
    Assert.Contains("Target:", help.StandardOutput);
    Assert.True(File.Exists(task));
  }

  [Theory]
  [InlineData("invalid-config")]
  [InlineData("legacy-config")]
  [InlineData("config-directory")]
  [InlineData("tasks-file")]
  public async Task ConflictsFailBeforeCreatingFiles( string conflict )
  {
    using var project = new TestProject();
    Directory.Delete(project.Tasks);
    var config = Path.Combine(project.Root, ".dotasks.yaml");
    var content = "invalid: [\n";
    switch (conflict) {
      case "invalid-config":
        project.Write(".dotasks.yaml", content);
        break;
      case "legacy-config":
        project.Write(".tasks/config.yaml", "settings: {}\n");
        break;
      case "config-directory":
        Directory.CreateDirectory(config);
        break;
      case "tasks-file":
        project.Write(".tasks", content);
        break;
    }
    var before = Directory.GetFileSystemEntries(project.Root, "*", SearchOption.AllDirectories).Order().ToArray();
    var result = await project.RunAsync("--init");
    Assert.Equal(1, result.ExitCode);
    Assert.NotEmpty(result.StandardError);
    Assert.Equal("", result.StandardOutput);
    Assert.Equal(before, Directory.GetFileSystemEntries(project.Root, "*", SearchOption.AllDirectories).Order().ToArray());
    if (conflict == "invalid-config") {
      Assert.Equal(content, File.ReadAllText(config));
    }

    if (conflict == "tasks-file") {
      Assert.Equal(content, File.ReadAllText(project.Tasks));
    }

    if (conflict == "legacy-config") {
      Assert.Contains("Legacy configuration", result.StandardError);
      Assert.Equal("settings: {}\n", File.ReadAllText(Path.Combine(project.Tasks, "config.yaml")));
      Assert.False(File.Exists(config));
    }
  }

  [Theory]
  [InlineData("--force")]
  [InlineData("--dry-run")]
  [InlineData("--init")]
  [InlineData("another-project")]
  public async Task UnexpectedArgumentsFailWithoutInitializing( string argument )
  {
    using var project = new TestProject();
    Directory.Delete(project.Tasks);
    var result = await project.RunAsync("--init", argument);
    Assert.Equal(1, result.ExitCode);
    Assert.Contains("Usage: dotask --init", result.StandardError);
    Assert.Empty(Directory.EnumerateFileSystemEntries(project.Root));
  }

  [Fact]
  public async Task ExplicitTaskDirectoryStaysWithinCurrentRootAndWorksInProjectHelp()
  {
    using var project = new TestProject();
    Directory.Delete(project.Tasks);
    var result = await project.RunAsync("--init", "--use-dir", "build support/tasks");
    Assert.Equal(0, result.ExitCode);
    Assert.Contains("override is not saved", result.StandardOutput);
    Assert.True(Directory.Exists(Path.Combine(project.Root, "build support/tasks")));
    Assert.False(Directory.Exists(project.Tasks));
    Assert.True(File.Exists(Path.Combine(project.Root, ".dotasks.yaml")));
    var summary = await project.RunAsync("--use-dir=build support/tasks");
    Assert.Equal(0, summary.ExitCode);
    Assert.Contains("tasks: './build support/tasks'", summary.StandardOutput);
  }

  [Theory]
  [InlineData("..")]
  [InlineData(".")]
  [InlineData("../outside")]
  [InlineData(".dotasks.yaml")]
  [InlineData(".dotasks.yaml/tasks")]
  [InlineData(".dotasks-lock.yaml")]
  public async Task InvalidTaskLocationsFailWithoutWriting( string directory )
  {
    using var project = new TestProject();
    Directory.Delete(project.Tasks);
    var result = await project.RunAsync("--init", "--use-dir", directory);
    Assert.Equal(1, result.ExitCode);
    Assert.Empty(Directory.EnumerateFileSystemEntries(project.Root));
  }

  [Fact]
  public async Task LinksAreNotFollowedIncludingDanglingConfigurationLinks()
  {
    if (OperatingSystem.IsWindows()) {
      return; // Creating links may require privileges on Windows.
    }

    using var project = new TestProject();
    using var outside = new TestProject();
    Directory.Delete(project.Tasks);
    Directory.CreateSymbolicLink(project.Tasks, outside.Root);
    Assert.Equal(1, (await project.RunAsync("--init")).ExitCode);
    Assert.False(File.Exists(Path.Combine(project.Root, ".dotasks.yaml")));
    Directory.Delete(project.Tasks);
    var config = Path.Combine(project.Root, ".dotasks.yaml");
    var destination = Path.Combine(outside.Root, "untouched.yaml");
    File.CreateSymbolicLink(config, destination);
    Assert.Equal(1, (await project.RunAsync("--init")).ExitCode);
    Assert.False(File.Exists(destination));
    Assert.Equal(destination, new FileInfo(config).LinkTarget);
    Assert.False(Directory.Exists(project.Tasks));
    File.Delete(config);
    Directory.CreateSymbolicLink(Path.Combine(project.Root, "linked"), outside.Root);
    Assert.Equal(1, (await project.RunAsync("--init", "--use-dir", "linked/tasks")).ExitCode);
    Assert.False(Directory.Exists(Path.Combine(outside.Root, "tasks")));
    Assert.False(File.Exists(config));
  }

  [Fact]
  public async Task HelpCompletionAndCancellationDoNotInitializeOrReadConfiguration()
  {
    using var project = new TestProject();
    Directory.Delete(project.Tasks);
    var missing = Path.Combine(project.Root, "does-not-exist");
    var output = new StringWriter();
    Assert.Equal(0, await CliApplication.RunAsync(["--init", "--help"], missing, output, new StringWriter()));
    Assert.Contains("Usage: dotask --init", output.ToString());
    Assert.False(Directory.Exists(missing));
    var config = project.Write(".dotasks.yaml", "invalid: [");
    Assert.Equal(0, (await project.RunAsync("--help", "--init")).ExitCode);
    Assert.Contains(CompletionEngine.Complete("dotask --i", project.Root), c => c.Value == "--init");
    var options = CompletionEngine.Complete("dotask --init --", project.Root);
    Assert.Contains(options, c => c.Value == "--help");
    Assert.Contains(options, c => c.Value == "--use-dir");
    Assert.DoesNotContain(options, c => c.Value is "--init" or "--add");
    var error = new StringWriter();
    Assert.Equal(130, await CliApplication.RunAsync(["--init"], project.Root, new StringWriter(), error,
      new CancellationToken(canceled: true)));
    Assert.Contains("Cancelled", error.ToString());
    Assert.False(Directory.Exists(project.Tasks));
    Assert.Equal("invalid: [", File.ReadAllText(config));
  }

  [Fact]
  public async Task ConcurrentInitializersNeverOverwriteOrLeaveTemporaryFiles()
  {
    using var project = new TestProject();
    Directory.Delete(project.Tasks);
    var codes = await Task.WhenAll(Enumerable.Range(0, 8).Select(_ => Task.Run(async () =>
      await CliApplication.RunAsync(["--init"], project.Root, new StringWriter(), new StringWriter()))));
    Assert.All(codes, code => Assert.Equal(0, code));
    Assert.Equal(Path.GetFileName(project.Root), ProjectConfiguration.Load(project.Tasks, project.Root).Name);
    Assert.Empty(Directory.EnumerateFileSystemEntries(project.Tasks));
    Assert.Equal(2, Directory.EnumerateFileSystemEntries(project.Root).Count());
  }
}

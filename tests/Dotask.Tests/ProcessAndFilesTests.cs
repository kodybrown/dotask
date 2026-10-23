using System.Diagnostics;
using System.Text.Json;

namespace DoTask.Tests;

public sealed class ProcessAndFilesTests
{
  [Fact]
  public async Task ProcessArgumentsRoundTripWithoutShellExpansion()
  {
    using var project = new TestProject();
    var assembly = await project.BuildProcessProbeAsync("Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(args));");
    string[] arguments = ["", "two words", "a\"b", "'single'", "a=b=c", @"back\slash\", "$(touch BAD)", "& | ; < > %PATH%", "日本語", "line1\nline2"];
    var result = await project.Context().RunAsync(new ProcessDefinition {
      Executable = DotnetHost.Find(),
      Arguments = [assembly, .. arguments],
      CaptureOutput = true
    });
    Assert.Equal(arguments, JsonSerializer.Deserialize<string[]>(result.StandardOutput.Trim()));
    Assert.False(File.Exists(Path.Combine(project.Root, "BAD")));
  }

  [Fact]
  public async Task ProcessFailurePreservesExitCodeAndCanBeOptedOut()
  {
    using var project = new TestProject();
    var assembly = await project.BuildProcessProbeAsync("Environment.Exit(19);");
    var definition = new ProcessDefinition { Executable = DotnetHost.Find(), Arguments = [assembly], CaptureOutput = true };
    var error = await Assert.ThrowsAsync<ProcessFailedException>(() => project.Context().RunAsync(definition));
    Assert.Equal(19, error.ExitCode);
    Assert.Equal(19, (await project.Context().RunAsync(definition with { ThrowOnError = false })).ExitCode);
  }

  [Theory]
  [InlineData(false)]
  [InlineData(true)]
  public async Task CancellationKillsTheStartedProcess( bool failBeforeReady )
  {
    using var project = new TestProject();
    // Publish complete markers by rename: File.WriteAllText creates its
    // destination before writing, so existence alone is not a readiness signal.
    var assembly = await project.BuildProcessProbeAsync("""
      var root = Environment.CurrentDirectory;
      var pending = Path.Combine(root, "pid.pending");
      if (args.Length != 0) {
        // Deliberately reproduce the old write-in-place marker's incomplete state.
        File.WriteAllText(Path.Combine(root, "pid.txt"), "");
      }
      File.WriteAllText(pending, Environment.ProcessId.ToString());
      File.Move(pending, Path.Combine(root, "started.txt"));
      while (!File.Exists(Path.Combine(root, "continue.txt"))) {
        await Task.Delay(25);
      }
      File.WriteAllText(pending, Environment.ProcessId.ToString());
      File.Move(pending, Path.Combine(root, "pid.txt"));
      await Task.Delay(Timeout.Infinite);
      """);
    using var cancellation = new CancellationTokenSource(TimeSpan.FromSeconds(15));
    var running = project.Context().RunAsync(new ProcessDefinition {
      Executable = DotnetHost.Find(),
      Arguments = failBeforeReady ? [assembly, "--incomplete-pid"] : [assembly],
      CaptureOutput = true
    }, cancellation.Token);
    var pid = 0;
    async Task ObserveThenCancelAsync()
    {
      try {
        var started = Path.Combine(project.Root, "started.txt");
        while (!File.Exists(started)) {
          await Task.Delay(25, cancellation.Token);
        }
        pid = int.Parse(await File.ReadAllTextAsync(started));
        var ready = Path.Combine(project.Root, "pid.txt");
        if (failBeforeReady) {
          Assert.Empty(await File.ReadAllTextAsync(ready));
          // Reading a newly created marker could previously fail here before
          // cancellation, leaving the child alive until fixture deletion.
          _ = int.Parse(await File.ReadAllTextAsync(ready));
          Assert.Fail("An incomplete PID must fail parsing.");
        }
        project.Write("continue.txt", "");
        while (!File.Exists(ready)) {
          await Task.Delay(25, cancellation.Token);
        }
        Assert.Equal(pid, int.Parse(await File.ReadAllTextAsync(ready)));
      } finally {
        // A readiness timeout/read/assertion failure must not leave the child
        // holding the fixture's working directory or mask the original failure.
        cancellation.Cancel();
        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => running);
      }
    }
    if (failBeforeReady) {
      await Assert.ThrowsAsync<FormatException>(ObserveThenCancelAsync);
    } else {
      await ObserveThenCancelAsync();
    }
    Assert.NotEqual(0, pid);
    Assert.Throws<ArgumentException>(() => Process.GetProcessById(pid));
  }

  [Fact]
  public void FileHelpersResolvePortablePathsAndProtectProjectRoot()
  {
    using var project = new TestProject();
    var context = project.Context();
    context.Files.WriteText(@"artifacts\a file.txt", "contents");
    Assert.Equal("contents", context.Files.ReadText("artifacts/a file.txt"));
    context.Files.CopyFile("artifacts/a file.txt", "copy/out.txt");
    Assert.True(context.Files.FileExists("copy/out.txt"));
    Assert.Throws<IOException>(() => context.Files.CopyFile("artifacts/a file.txt", "copy/out.txt"));
    context.Files.CreateZip("copy", "archives/result.zip");
    Assert.True(context.Files.FileExists("archives/result.zip"));
    Assert.Single(context.Files.FindFiles("artifacts", "*.txt"));
    Assert.Throws<TaskException>(() => context.Files.DeleteDirectory("."));
    context.Files.DeleteDirectory("copy");
    Assert.False(context.Files.DirectoryExists("copy"));
  }

  [Fact]
  public async Task InvalidProcessInputsFailBeforeLaunch()
  {
    using var project = new TestProject();
    await Assert.ThrowsAsync<TaskException>(() => project.Context().RunAsync("dotnet", ["bad\0value"]));
    if (!OperatingSystem.IsWindows()) {
      Assert.Throws<TaskException>(() => project.Context().Path(@"C:\Windows\file.txt"));
    }
  }
}

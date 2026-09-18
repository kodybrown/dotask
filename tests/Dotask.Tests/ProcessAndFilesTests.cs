using System.Diagnostics;
using System.Text.Json;
using DoTask.Cli.Execution;
using DoTask.Cli.Metadata;

namespace DoTask.Tests;

public sealed class ProcessAndFilesTests
{
  [Fact]
  public async Task ProcessArgumentsRoundTripWithoutShellExpansion()
  {
    using var project = new TestProject();
    var source = project.Write(".tasks/probe.cs", "Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(args));");
    var compiler = new TargetCompiler();
    var compiled = await compiler.CompileAsync(MetadataReader.Read(source), CancellationToken.None);
    Assert.True(compiled.Success, compiled.Diagnostics);
    string[] arguments = ["", "two words", "a\"b", "'single'", "a=b=c", @"back\slash\", "$(touch BAD)", "& | ; < > %PATH%", "日本語", "line1\nline2"];
    var result = await project.Context().RunAsync(new ProcessDefinition
    {
      Executable = DotnetHost.Find(),
      Arguments = [compiled.AssemblyPath!, .. arguments],
      CaptureOutput = true
    });
    Assert.Equal(arguments, JsonSerializer.Deserialize<string[]>(result.StandardOutput.Trim()));
    Assert.False(File.Exists(Path.Combine(project.Root, "BAD")));
  }

  [Fact]
  public async Task ProcessFailurePreservesExitCodeAndCanBeOptedOut()
  {
    using var project = new TestProject();
    var target = MetadataReader.Read(project.Target("exit", "Environment.Exit(19);"));
    var compiled = await new TargetCompiler().CompileAsync(target, CancellationToken.None);
    Assert.True(compiled.Success, compiled.Diagnostics);
    var definition = new ProcessDefinition { Executable = DotnetHost.Find(), Arguments = [compiled.AssemblyPath!], CaptureOutput = true };
    var error = await Assert.ThrowsAsync<ProcessFailedException>(() => project.Context().RunAsync(definition));
    Assert.Equal(19, error.ExitCode);
    Assert.Equal(19, (await project.Context().RunAsync(definition with { ThrowOnError = false })).ExitCode);
  }

  [Fact]
  public async Task CancellationKillsTheStartedProcess()
  {
    using var project = new TestProject();
    var file = project.Target("wait", """
      File.WriteAllText(argsFile(), Environment.ProcessId.ToString());
      await Task.Delay(Timeout.Infinite);
      static string argsFile() => Path.Combine(Environment.CurrentDirectory, "pid.txt");
      """, async: true);
    var compiled = await new TargetCompiler().CompileAsync(MetadataReader.Read(file), CancellationToken.None);
    Assert.True(compiled.Success, compiled.Diagnostics);
    using var cancellation = new CancellationTokenSource(TimeSpan.FromSeconds(15));
    var running = project.Context().RunAsync(new ProcessDefinition
    {
      Executable = DotnetHost.Find(),
      Arguments = [compiled.AssemblyPath!],
      CaptureOutput = true
    }, cancellation.Token);
    var pidFile = Path.Combine(project.Root, "pid.txt");
    while (!File.Exists(pidFile))
    {
      await Task.Delay(25, cancellation.Token);
    }
    var pid = int.Parse(await File.ReadAllTextAsync(pidFile));
    cancellation.Cancel();
    await Assert.ThrowsAnyAsync<OperationCanceledException>(() => running);
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
    if (!OperatingSystem.IsWindows())
    {
      Assert.Throws<TaskException>(() => project.Context().Path(@"C:\Windows\file.txt"));
    }
  }
}

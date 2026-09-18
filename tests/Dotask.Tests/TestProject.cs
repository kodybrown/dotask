using System.Text.Json;
using DoTask.Cli;
using DoTask.Cli.Execution;
using DoTask.Runtime;

namespace DoTask.Tests;

public sealed class TestProject : IDisposable
{
  public string Root { get; } = Path.Combine(Path.GetTempPath(), "dotask-tests", "project with spaces " + Guid.NewGuid().ToString("N"));
  public string Tasks => Path.Combine(Root, ".tasks");

  public TestProject() => Directory.CreateDirectory(Tasks);

  public string Write( string relative, string text )
  {
    var file = Path.Combine(Root, relative);
    Directory.CreateDirectory(Path.GetDirectoryName(file)!);
    File.WriteAllText(file, text);
    return file;
  }

  public string Target( string name, string body = "", string metadata = "", bool async = false )
    => Write($".tasks/{name}.cs", $$"""
      using DoTask;
      {{metadata}}
      public static class Target
      {
        public static {{(async ? "async Task" : "void")}} Main()
        {
          {{body}}
        }
      }
      """);

  public BuildContext Context() => new(new ExecutionContextData {
    RootDirectory = Root,
    InvocationDirectory = Root,
    TaskDirectory = Tasks,
    TargetFile = Path.Combine(Tasks, "test.cs"),
    TargetName = "test",
    CliAssembly = typeof(CliApplication).Assembly.Location,
    DotnetExecutable = DotnetHost.Find(),
    SessionDirectory = Root,
    Settings = JsonSerializer.SerializeToElement(new { }),
    Parameters = JsonSerializer.SerializeToElement(new { }),
    TargetDefaults = JsonSerializer.SerializeToElement(new { })
  });

  public async Task<ProcessResult> RunAsync( params string[] arguments ) => await RunFromAsync(Root, arguments);

  public async Task<ProcessResult> RunFromAsync( string directory, params string[] arguments )
  {
    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(90));
    return await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = DotnetHost.Find(),
      Arguments = [typeof(CliApplication).Assembly.Location, .. arguments],
      WorkingDirectory = directory,
      CaptureOutput = true,
      ThrowOnError = false
    }, timeout.Token);
  }

  public void Dispose()
  {
    if (Directory.Exists(Root)) {
      Directory.Delete(Root, recursive: true);
    }
  }
}

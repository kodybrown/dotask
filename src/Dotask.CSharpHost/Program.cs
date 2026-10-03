using System.Text.Json;
using DoTask;
using DoTask.Cli.Configuration;
using DoTask.Cli.Discovery;
using DoTask.Cli.Execution;
using DoTask.Cli.Metadata;
using DoTask.Cli.Parsing;
using DoTask.Runtime;

// Private file protocol: stdout/stderr remain available for tool diagnostics.
// This process prepares C# tasks but never executes one or dispatches CLI commands.
if (Console.IsOutputRedirected || Console.IsErrorRedirected) {
  Console.OutputEncoding = new System.Text.UTF8Encoding(false);
}
using var cancellation = new CancellationTokenSource();
Console.CancelKeyPress += ( _, args ) => { args.Cancel = true; cancellation.Cancel(); };
try {
  if (args.Length != 2) {
    throw new TaskException("The C# support host requires a request file and a response file.");
  }
  var request = JsonSerializer.Deserialize<HostRequest>(await File.ReadAllTextAsync(args[0], cancellation.Token))
    ?? throw new TaskException("Invalid C# support request.");
  if (request.Version != 1) {
    throw new TaskException("Unsupported C# support protocol version.");
  }
  object result;
  switch (request.Operation) {
    case "catalog":
      result = new {
        Configuration = request.Configuration ?? ProjectConfiguration.Load(request.TaskDirectory, request.RootDirectory),
        Targets = new TargetCatalog(request.TaskDirectory).Targets
      };
      break;
    case "bind":
      var target = request.Target ?? throw new TaskException("A target is required.");
      if (target.Error is not null) {
        throw new TaskException(target.Error);
      }
      var config = request.Configuration ?? throw new TaskException("Configuration is required.");
      var parameters = OptionBinder.Bind(target, config, request.Arguments, request.RootDirectory, !request.Help);
      if (!request.Help) {
        Requirements.Validate(target, config, request.RootDirectory);
      }
      result = parameters;
      break;
    case "compile":
      var compilation = await new TargetCompiler().CompileAsync(
        request.Target ?? throw new TaskException("A target is required."), cancellation.Token,
        request.SnapshotDirectory ?? throw new TaskException("A snapshot directory is required."));
      if (!compilation.Success) {
        throw new TaskException($"Compilation failed for '{request.Target.Name}':\n{compilation.Diagnostics}");
      }
      result = new { compilation.AssemblyPath };
      break;
    default:
      throw new TaskException("Unknown C# support operation.");
  }
  await ContextFile.WriteToAsync(args[1], new { Version = 1, Result = result }, cancellation.Token);
  return 0;
} catch (OperationCanceledException) {
  return 130;
} catch (Exception ex) when (ex is TaskException or IOException or UnauthorizedAccessException or ArgumentException or JsonException) {
  Console.Error.WriteLine($"dotask: {ex.Message}");
  return 1;
}

internal sealed record HostRequest( int Version, string Operation, string TaskDirectory, string RootDirectory,
  TargetDefinition? Target = null, ProjectConfiguration? Configuration = null, string? SnapshotDirectory = null,
  bool Help = false )
{
  public string[] Arguments { get; init; } = [];
}

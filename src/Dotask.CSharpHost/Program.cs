using System.Text.Json;
using DoTask;
using DoTask.Cli.Execution;
using DoTask.Cli.Metadata;
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
  if (request.Version != 2) {
    throw new TaskException("Unsupported C# support protocol version.");
  }
  object result;
  switch (request.Operation) {
    case "metadata":
      result = request.Files.Select(file => MetadataReader.Read(file, request.TaskDirectory)).ToArray();
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
  await ContextFile.WriteToAsync(args[1], new { Version = 2, Result = result }, cancellation.Token);
  return 0;
} catch (OperationCanceledException) {
  return 130;
} catch (Exception ex) when (ex is TaskException or IOException or UnauthorizedAccessException or ArgumentException or JsonException) {
  Console.Error.WriteLine($"dotask: {ex.Message}");
  return 1;
}

internal sealed record HostRequest( int Version, string Operation, string TaskDirectory,
  TargetDefinition? Target = null, string? SnapshotDirectory = null )
{
  public string[] Files { get; init; } = [];
}

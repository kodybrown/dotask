using System.Text.Json;

namespace DoTask.Runtime;

internal sealed record ExecutionContextData
{
  public const string EnvironmentVariable = "DOTASK_EXECUTION_CONTEXT";
  public required string RootDirectory { get; init; }
  public required string InvocationDirectory { get; init; }
  public required string TaskDirectory { get; init; }
  public required string TargetFile { get; init; }
  public required string TargetName { get; init; }
  public required string DotnetExecutable { get; init; }
  public required string CliAssembly { get; init; }
  public required string SessionDirectory { get; init; }
  public required JsonElement Settings { get; init; }
  public required JsonElement Parameters { get; init; }
  public required JsonElement TargetDefaults { get; init; }
  public bool Verbose { get; init; }
  public string[] CallChain { get; init; } = [];
}

internal enum TargetCallOperation { Execute, Exists, ExecuteIfExists }

internal sealed record TargetCall( ExecutionContextData Context, string Target, JsonElement Parameters,
  TargetCallOperation Operation = TargetCallOperation.Execute );

internal sealed record TargetCallReply( bool Exists, int? ExitCode = null, string? Error = null );

internal static class ContextFile
{
  public static async Task<string> WriteAsync<T>( string directory, T value, CancellationToken cancellationToken )
  {
    Directory.CreateDirectory(directory);
    var path = System.IO.Path.Combine(directory, Guid.NewGuid().ToString("N") + ".json");
    try {
      await WriteToAsync(path, value, cancellationToken);
      return path;
    } catch {
      File.Delete(path);
      throw;
    }
  }

  public static async Task WriteToAsync<T>( string path, T value, CancellationToken cancellationToken )
  {
    var options = new FileStreamOptions { Mode = FileMode.CreateNew, Access = FileAccess.Write, Share = FileShare.None };
    if (!OperatingSystem.IsWindows()) {
      options.UnixCreateMode = UnixFileMode.UserRead | UnixFileMode.UserWrite;
    }
    await using var stream = new FileStream(path, options);
    await JsonSerializer.SerializeAsync(stream, value, cancellationToken: cancellationToken);
  }
}

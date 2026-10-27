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
  // A native CLI has no argument prefix; the managed CLI uses dotnet + its DLL.
  // Keep the transport independent of the language implementing the caller.
  public required string CliExecutable { get; init; }
  public required string[] CliArguments { get; init; }
  public required string SessionDirectory { get; init; }
  // The CLI retains shared data separately from Settings, the current target's
  // resolved view, so a nested call can resolve its own overrides.
  public required JsonElement ProjectSettings { get; init; }
  public required JsonElement Settings { get; init; }
  public required JsonElement Parameters { get; init; }
  public required JsonElement TargetDefaults { get; init; }
  public bool Verbose { get; init; }
  public string[] CallChain { get; init; } = [];
}

internal enum TargetCallOperation { Execute, Exists, ExecuteIfExists, CreateInstaller }

internal sealed record TargetCall( ExecutionContextData Context, string Target, JsonElement Parameters,
  TargetCallOperation Operation = TargetCallOperation.Execute );

internal sealed record TargetCallReply( bool Exists, int? ExitCode = null, string? Error = null, InstallerArtifact? Installer = null );

internal static class ContextFile
{
  internal static async Task DeleteDirectoryAsync( string directory )
  {
    // Killing a process tree and awaiting its root does not await every child.
    // Windows can briefly retain loaded SDK DLLs after cancellation. Retry only
    // this owned request directory, without the canceled token, and still fail
    // visibly if the lock/permissions do not clear within the bounded grace.
    var elapsed = System.Diagnostics.Stopwatch.StartNew();
    while (Directory.Exists(directory)) {
      try {
        Directory.Delete(directory, recursive: true);
        return;
      } catch (Exception error) when (OperatingSystem.IsWindows()
        && error is IOException or UnauthorizedAccessException
        && elapsed.Elapsed < TimeSpan.FromSeconds(5)) {
        await Task.Delay(50);
      }
    }
  }

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

using System.Diagnostics;
using System.Reflection;
using System.Text.Json;
using DoTask.Cli.SharedTasks;

namespace DoTask.Tests;

internal static class RustArtifacts
{
  private static readonly Lazy<string> TargetDirectory = new(Locate);

  public static string Binary( string name ) => Path.Combine(TargetDirectory.Value, "release",
    name + (OperatingSystem.IsWindows() ? ".exe" : ""));

  public static async Task<ProcessResult> Run( string directory, string[] args, SharedTaskOptions? options = null )
  {
    using var process = new Process {
      StartInfo = new ProcessStartInfo(Binary("dotask")) {
        WorkingDirectory = directory,
        UseShellExecute = false,
        RedirectStandardOutput = true,
        RedirectStandardError = true,
        StandardOutputEncoding = System.Text.Encoding.UTF8,
        StandardErrorEncoding = System.Text.Encoding.UTF8
      }
    };
    foreach (var arg in args) {
      process.StartInfo.ArgumentList.Add(arg);
    }
    if (options is not null) {
      process.StartInfo.Environment["DOTASK_CACHE_HOME"] = options.CacheDirectory;
      process.StartInfo.Environment["DOTASK_PRIVATE_TASKS"] = options.PrivateDirectory;
      process.StartInfo.Environment["DOTASK_ONLINE_TASKS"] = options.OnlineDirectory;
    }
    process.Start();
    var output = process.StandardOutput.ReadToEndAsync();
    var error = process.StandardError.ReadToEndAsync();
    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(90));
    try {
      await process.WaitForExitAsync(timeout.Token);
      return new(process.ExitCode, await output, await error);
    } finally {
      if (!process.HasExited) {
        process.Kill(entireProcessTree: true);
        await process.WaitForExitAsync();
      }
    }
  }

  private static string Locate()
  {
    var root = typeof(RustArtifacts).Assembly.GetCustomAttributes<AssemblyMetadataAttribute>()
      .Single(a => a.Key == "RepositoryRoot").Value!;
    using var process = new Process {
      StartInfo = new ProcessStartInfo("cargo") {
        WorkingDirectory = Path.GetFullPath(root),
        UseShellExecute = false,
        RedirectStandardOutput = true,
        RedirectStandardError = true,
        ArgumentList = { "metadata", "--format-version", "1", "--no-deps", "--locked" }
      }
    };
    process.Start();
    var output = process.StandardOutput.ReadToEndAsync();
    var error = process.StandardError.ReadToEndAsync();
    if (!process.WaitForExit(30000)) {
      process.Kill(entireProcessTree: true);
      throw new InvalidOperationException("Cargo metadata timed out locating the native test artifacts.");
    }
    if (process.ExitCode != 0) {
      throw new InvalidOperationException(error.GetAwaiter().GetResult());
    }
    using var metadata = JsonDocument.Parse(output.GetAwaiter().GetResult());
    return metadata.RootElement.GetProperty("target_directory").GetString()!;
  }
}

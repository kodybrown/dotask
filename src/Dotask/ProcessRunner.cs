using System.ComponentModel;
using System.Diagnostics;

namespace DoTask;

internal static class ProcessRunner
{
  public static async Task<ProcessResult> RunAsync( ProcessDefinition definition, CancellationToken cancellationToken )
  {
    ArgumentException.ThrowIfNullOrWhiteSpace(definition.Executable);
    if (definition.Executable.Contains('\0') || definition.Arguments.Any(x => x is null || x.Contains('\0'))) {
      throw new TaskException("Process executable and arguments must not contain NUL characters or null values.");
    }
    cancellationToken.ThrowIfCancellationRequested();
    var start = new ProcessStartInfo(definition.Executable) {
      UseShellExecute = false,
      WorkingDirectory = definition.WorkingDirectory ?? System.Environment.CurrentDirectory,
      RedirectStandardOutput = definition.CaptureOutput,
      RedirectStandardError = definition.CaptureOutput
    };
    foreach (var argument in definition.Arguments) {
      start.ArgumentList.Add(argument);
    }
    foreach (var variable in definition.Environment) {
      start.Environment[variable.Key] = variable.Value;
    }
    using var process = new Process { StartInfo = start };
    try {
      process.Start();
    } catch (Win32Exception ex) {
      throw new TaskException($"Cannot start '{definition.Executable}': {ex.Message}");
    }
    var stdout = definition.CaptureOutput ? process.StandardOutput.ReadToEndAsync() : Task.FromResult("");
    var stderr = definition.CaptureOutput ? process.StandardError.ReadToEndAsync() : Task.FromResult("");
    try {
      await process.WaitForExitAsync(cancellationToken);
    } catch (OperationCanceledException) {
      try {
        process.Kill(entireProcessTree: true);
      } catch (InvalidOperationException) { }
      await process.WaitForExitAsync(CancellationToken.None);
      await Task.WhenAll(stdout, stderr);
      throw;
    }
    var result = new ProcessResult(process.ExitCode, await stdout, await stderr);
    if (definition.ThrowOnError && result.ExitCode != 0) {
      throw new ProcessFailedException(definition.Executable, result.ExitCode);
    }
    return result;
  }
}

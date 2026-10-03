using System.ComponentModel;

namespace DoTask.Runtime;

/// <summary>Startup support injected by dotask when compiling a target.</summary>
[EditorBrowsable(EditorBrowsableState.Never)]
public static class TargetRuntime
{
  private static int _initialized;

  public static void Initialize()
  {
    if (Interlocked.Exchange(ref _initialized, 1) != 0) {
      return;
    }
    // A native caller does not set .NET's console encoding. Keep redirected
    // task output lossless regardless of the Windows console's legacy page.
    if (Console.IsOutputRedirected || Console.IsErrorRedirected) {
      Console.OutputEncoding = new System.Text.UTF8Encoding(false);
    }
    AppDomain.CurrentDomain.UnhandledException += ( _, args ) =>
    {
      var exception = args.ExceptionObject as Exception;
      Console.Error.WriteLine(exception is TaskException ? exception.Message : args.ExceptionObject);
      // An unhandled target exception is an ordinary task failure. Avoid the
      // runtime's abort/core-dump path, while preserving useful source diagnostics.
      Environment.Exit(exception switch {
        OperationCanceledException => 130,
        ProcessFailedException failed => failed.ExitCode,
        _ => 1
      });
    };
  }
}

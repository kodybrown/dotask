namespace DoTask;

public class TaskException( string message ) : Exception(message);

public sealed class ProcessFailedException( string executable, int exitCode )
  : TaskException($"'{executable}' exited with code {exitCode}.")
{
  public int ExitCode { get; } = exitCode;
}

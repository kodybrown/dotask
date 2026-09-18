namespace DoTask;

public sealed record ProcessDefinition
{
  public required string Executable { get; init; }
  public IReadOnlyList<string> Arguments { get; init; } = [];
  public string? WorkingDirectory { get; init; }
  public IReadOnlyDictionary<string, string?> Environment { get; init; } = new Dictionary<string, string?>();
  public bool CaptureOutput { get; init; }
  public bool ThrowOnError { get; init; } = true;
}

public sealed record ProcessResult( int ExitCode, string StandardOutput, string StandardError );

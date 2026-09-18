namespace DoTask;

public enum TargetExecutionStatus { Succeeded, NotFound, Failed }

/// <summary>The outcome of an optional target invocation.</summary>
/// <param name="Status">Whether the target succeeded, was absent, or failed.</param>
/// <param name="ExitCode">The observed target process exit code, or null if no target process completed.</param>
/// <param name="Error">A failure diagnostic, including failures before execution.</param>
public sealed record TargetExecutionResult(TargetExecutionStatus Status, int? ExitCode = null, string? Error = null);

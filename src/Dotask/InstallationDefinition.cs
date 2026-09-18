namespace DoTask;

/// <summary>A language-independent description of an already published application.</summary>
public sealed record InstallationDefinition
{
  public required string AppId { get; init; }
  public required string Version { get; init; }
  public required string SourceDirectory { get; init; }
  public required IReadOnlyList<InstalledCommand> Commands { get; init; }
  public string? BinDirectory { get; init; }
  /// <summary>Parent directory for application installations; defaults to the current user's platform location.</summary>
  public string? InstallRoot { get; init; }
}

/// <param name="Name">Command name without an extension.</param>
/// <param name="Executable">Portable relative path inside the published directory.</param>
public sealed record InstalledCommand( string Name, string Executable );

public sealed record InstallationResult( string InstallDirectory, string BinDirectory,
  string Fingerprint, bool Reused, IReadOnlyList<string> Warnings );

namespace DoTask.Cli.Metadata;

public sealed record OptionDefinition( string Name, string? Alias, string Type, string Description,
  string? Default, bool Required, string[] Choices, string? Completion );

public sealed record Requirement( string Kind, string Value );

public sealed record TargetDefinition( string Name, string FilePath, string Description,
  IReadOnlyList<OptionDefinition> Options, IReadOnlyList<Requirement> Requirements,
  IReadOnlyList<string> Capabilities, string? Remarks, IReadOnlyList<string> Examples, string? Error = null,
  string? ShortName = null, TaskGroup? Group = null );

public sealed record TaskGroup( bool RequireAtLeastOneStep, IReadOnlyList<TaskStep> Steps );

public sealed record TaskStep( string Run, bool Optional, System.Text.Json.JsonElement Parameters );

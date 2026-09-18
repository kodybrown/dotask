namespace DoTask.Cli.Parsing;

public sealed record CommandLine( string? UseDirectory, string? Command, string[] Arguments, bool Help, bool Version )
{
  public static CommandLine Parse( IReadOnlyList<string> arguments )
  {
    string? directory = null;
    var remaining = new List<string>();
    var help = false;
    var version = false;
    for (var index = 0; index < arguments.Count; index++) {
      var argument = arguments[index];
      if (argument.Equals("--use-dir", StringComparison.OrdinalIgnoreCase)
        || argument.StartsWith("--use-dir=", StringComparison.OrdinalIgnoreCase)) {
        if (directory is not null) {
          throw new TaskException("--use-dir may only be specified once.");
        }
        if (argument.Contains('=')) {
          directory = argument[(argument.IndexOf('=') + 1)..];
        } else {
          if (++index >= arguments.Count) {
            throw new TaskException("--use-dir requires a directory.");
          }
          directory = arguments[index];
        }
        if (string.IsNullOrWhiteSpace(directory)) {
          throw new TaskException("--use-dir requires a nonempty directory.");
        }
      } else if (argument.Equals("--help", StringComparison.OrdinalIgnoreCase) || argument.Equals("-h", StringComparison.OrdinalIgnoreCase)) {
        help = true;
      } else if (argument.Equals("--version", StringComparison.OrdinalIgnoreCase)) {
        version = true;
      } else {
        remaining.Add(argument);
      }
    }
    return new(directory, remaining.FirstOrDefault(), remaining.Skip(1).ToArray(), help, version);
  }
}

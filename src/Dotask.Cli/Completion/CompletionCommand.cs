using System.Text;

namespace DoTask.Cli.Completion;

internal static class CompletionCommand
{
  public static void PrintScript( string shell, TextWriter output )
  {
    shell = shell.ToLowerInvariant();
    if (shell is not ("bash" or "zsh" or "fish" or "powershell")) {
      throw new TaskException("Choose a completion shell: bash, zsh, fish, or powershell.");
    }
    var name = $"DoTask.Cli.Completion.Scripts.{shell}";
    using var stream = typeof(CompletionCommand).Assembly.GetManifestResourceStream(name)
      ?? throw new TaskException($"Completion script '{shell}' is unavailable.");
    using var reader = new StreamReader(stream);
    output.Write(reader.ReadToEnd());
  }

  public static void Query( IReadOnlyList<string> arguments, string invocationDirectory, TextWriter output )
  {
    string line = "", shell = "bash";
    int? position = null;
    for (var i = 0; i < arguments.Count; i++) {
      if (i + 1 >= arguments.Count) {
        return;
      }
      switch (arguments[i]) {
        case "--line":
          line = arguments[++i];
          break;
        case "--shell":
          shell = arguments[++i];
          break;
        case "--position" when int.TryParse(arguments[++i], out var value):
          position = value;
          break;
        default:
          return;
      }
    }
    if (position is { } cursor) {
      if (shell == "bash") {
        var bytes = Encoding.UTF8.GetBytes(line);
        line = Encoding.UTF8.GetString(bytes.AsSpan(0, Math.Clamp(cursor, 0, bytes.Length)));
      } else if (shell == "zsh") {
        line = string.Concat(line.EnumerateRunes().Take(Math.Max(cursor, 0)).Select(r => r.ToString()));
      } else {
        line = line[..Math.Clamp(cursor, 0, line.Length)];
      }
    }
    foreach (var candidate in CompletionEngine.Complete(line, invocationDirectory, shell)) {
      var description = candidate.Description.Replace('\t', ' ').Replace('\r', ' ').Replace('\n', ' ');
      output.WriteLine($"{candidate.Value}\t{candidate.Kind}\t{description}");
    }
  }
}

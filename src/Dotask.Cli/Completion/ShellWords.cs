using System.Text;

namespace DoTask.Cli.Completion;

public static class ShellWords
{
  /// <summary>Tokenizes the text before the cursor; never expands or evaluates shell expressions.</summary>
  public static IReadOnlyList<string> Parse( string line, string shell )
  {
    var words = new List<string>();
    var word = new StringBuilder();
    char quote = '\0';
    var started = false;
    var powershell = shell.Equals("powershell", StringComparison.OrdinalIgnoreCase);
    for (var index = 0; index < line.Length; index++) {
      var c = line[index];
      var escape = powershell ? '`' : '\\';
      if (c == escape && quote != '\'' && index + 1 < line.Length) {
        var next = line[index + 1];
        if (quote != '"' || powershell || next is '"' or '\\' or '$' or '`' or '\n') {
          word.Append(next);
          index++;
          started = true;
          continue;
        }
      }
      if (quote != '\0') {
        if (c == quote) {
          if (powershell && quote == '\'' && index + 1 < line.Length && line[index + 1] == '\'') {
            word.Append('\'');
            index++;
          } else {
            quote = '\0';
          }
        } else {
          word.Append(c);
        }
      } else if (c is '\'' or '"') {
        quote = c;
        started = true;
      } else if (c is ';' or '|' or '&') {
        words.Clear();
        word.Clear();
        started = false;
      } else if (char.IsWhiteSpace(c)) {
        if (started || word.Length > 0) {
          words.Add(word.ToString());
          word.Clear();
          started = false;
        }
      } else {
        word.Append(c);
        started = true;
      }
    }
    words.Add(word.ToString());
    return words;
  }
}

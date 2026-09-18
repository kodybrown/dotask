namespace DoTask.Cli;

internal sealed class HelpText(TextWriter output, int? consoleWidth = null)
{
  private readonly int? _width = consoleWidth is >= 60 ? consoleWidth : null;

  public static int? GetConsoleWidth(TextWriter output)
  {
    if (!ReferenceEquals(output, Console.Out) || Console.IsOutputRedirected)
    {
      return null;
    }
    try
    {
      return Console.WindowWidth;
    }
    catch (Exception ex) when (ex is IOException or PlatformNotSupportedException)
    {
      return null;
    }
  }

  public void WriteLine(string value = "", int continuationIndent = 0)
  {
    if (_width is not { } width)
    {
      output.WriteLine(value);
      return;
    }
    var indent = new string(' ', continuationIndent < width - 1 ? continuationIndent : 2);
    var lines = value.Replace("\r\n", "\n").Replace('\r', '\n').Split('\n');
    for (var i = 0; i < lines.Length; i++)
    {
      var remaining = i == 0 || lines[i].Length == 0 ? lines[i] : indent + lines[i];
      if (remaining.Length - remaining.TrimStart().Length >= width - 1)
      {
        remaining = "  " + remaining.TrimStart();
      }
      while (remaining.Length > width)
      {
        var split = width;
        while (split > 0 && !char.IsWhiteSpace(remaining[split]))
        {
          split--;
        }
        if (split <= remaining.Length - remaining.TrimStart().Length)
        {
          split = width;
          if (char.IsHighSurrogate(remaining[split - 1]) && char.IsLowSurrogate(remaining[split]))
          {
            split--;
          }
        }
        output.WriteLine(remaining[..split].TrimEnd());
        remaining = indent + remaining[split..].TrimStart();
      }
      output.WriteLine(remaining);
    }
  }

  public int WriteRow(string prefix, string value)
  {
    if (_width is { } width && prefix.Length >= width - 1)
    {
      if (!string.IsNullOrWhiteSpace(prefix))
      {
        WriteLine(prefix.TrimEnd(), 2);
      }
      prefix = "  ";
    }
    WriteLine(value.Length == 0 ? prefix.TrimEnd() : prefix + value, prefix.Length);
    return prefix.Length;
  }
}

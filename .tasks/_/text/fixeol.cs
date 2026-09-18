using DoTask;

/// <summary>Normalize text-file encoding and line endings using fixeol.</summary>
/// <option name="eol" choices="lf,crlf" default="lf">Line endings for text files; batch files always use CRLF.</option>
/// <example>dotask fixeol</example>
/// <example>dotask fixeol --eol crlf</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var fixeol = FindFixEol();
    if (fixeol is null) {
      Console.Error.WriteLine("Warning: fixeol was not found on PATH; skipping UTF-8 and line-ending normalization.");
      return;
    }

    string[] exclusions = [
      ".git", ".tasks", "node_modules", "bin", "obj", "dist", "artifacts", "target", "coverage",
      "playwright-report", "test-results", "TestResults"
    ];
    List<string> excludeArguments = ["--exclude", ".mcp_file_state.json"];
    foreach (var directory in exclusions) {
      excludeArguments.AddRange(["--exclude", $"*/{directory}/*"]);
    }

    // Keep generated output out of text normalization.
    string[] textPatterns = [
      "*.code-workspace", "*.cs", "*.csproj", "*.css", "*.editorconfig",
      "*.gitattributes", "*.gitignore", "*.htm", "*.html", "*.ini", "*.js",
      "*.json", "*.md", "*.prettierrc", "*.ps1", "*.psm1", "*.pubxml",
      "*.py", "*.razor", "*.rest", "*.rs", "*.scss", "*.sh", "*.sln",
      "*.slnx", "*.sql", "*.text", "*.toml", "*.ts", "*.txt", "*.vue",
      "*.xml", "*.xsl", "*.yaml", "*.yml", "Makefile"
    ];
    await project.RunAsync(fixeol, [
      "--encoding", "utf8", "--eol", project.Parameters.Get<string>("eol"),
      "--recursive", .. textPatterns, .. excludeArguments
    ]);
    foreach (var pattern in new[] { "*.cmd", "*.bat" }) {
      await project.RunAsync(fixeol, [
        "--encoding", "utf8", "--eol", "crlf", "--recursive", pattern, .. excludeArguments
      ]);
    }
  }

  private static string? FindFixEol()
  {
    var name = OperatingSystem.IsWindows() ? "fixeol.exe" : "fixeol";
    var directories = (Environment.GetEnvironmentVariable("PATH") ?? string.Empty)
      .Split(Path.PathSeparator, StringSplitOptions.RemoveEmptyEntries);
    foreach (var directory in directories) {
      try {
        var candidate = Path.GetFullPath(Path.Combine(directory.Trim('"'), name));
        if (!File.Exists(candidate)) {
          continue;
        }

        if (!OperatingSystem.IsWindows()
            && (File.GetUnixFileMode(candidate)
              & (UnixFileMode.UserExecute | UnixFileMode.GroupExecute | UnixFileMode.OtherExecute)) == 0) {
          continue;
        }

        return candidate;
      } catch (Exception exception) when (
          exception is ArgumentException or IOException or UnauthorizedAccessException) {
        // An unusable PATH entry must not hide a later executable.
      }
    }

    return null;
  }
}

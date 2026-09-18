using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;

namespace DoTask.Cli.SharedTasks;

internal static partial class SharedTaskFiles
{
  public static string Hash( byte[] bytes ) => Convert.ToHexStringLower(SHA256.HashData(bytes));
  public static string Hash( string text ) => Hash(Encoding.UTF8.GetBytes(text));
  public static string? HashFile( string path ) => File.Exists(path) ? Hash(File.ReadAllBytes(path)) : null;
  public static bool IsHash( string? value ) => value is { Length: 64 } && value.All(char.IsAsciiHexDigit);

  public static void ValidateId( string id, int? parts = null )
  {
    if (string.IsNullOrEmpty(id)) {
      throw new TaskException("A shared task identifier cannot be empty.");
    }
    var segments = id.Split('/');
    if ((parts is not null && segments.Length != parts) || segments.Where(( s, index ) => !(index == 0 && s == "_" && parts is 1 or 3)).Any(s => !Identifier().IsMatch(s) || IsDevice(s))) {
      throw new TaskException($"Invalid shared task identifier '{id}'. Use group/task names made of letters, digits, underscores, and hyphens.");
    }
  }

  public static void ValidateRelativePath( string path )
  {
    if (string.IsNullOrEmpty(path) || path.Split('/').Any(p => p.Length == 0 || p is "." or ".."
      || p.EndsWith('.') || p.EndsWith(' ') || p.Any(c => !char.IsAsciiLetterOrDigit(c) && c is not ('_' or '-' or '.' or ' ')) || IsDevice(p))) {
      throw new TaskException($"Unsafe or non-portable shared task path '{path}'.");
    }
  }

  private static bool IsDevice( string name )
  {
    var stem = name.Split('.')[0].ToUpperInvariant();
    return stem is "CON" or "PRN" or "AUX" or "NUL" || Device().IsMatch(stem);
  }

  public static string Resolve( string root, string relative )
  {
    ValidateRelativePath(relative);
    var current = Path.GetFullPath(root);
    CheckLink(current);
    foreach (var segment in relative.Split('/')) {
      if (Directory.Exists(current)) {
        var matches = Directory.EnumerateFileSystemEntries(current).Where(p => Path.GetFileName(p).Equals(segment, StringComparison.OrdinalIgnoreCase)).ToArray();
        if (matches.Length > 1 || matches.Any(p => !Path.GetFileName(p).Equals(segment, StringComparison.Ordinal))) {
          throw new TaskException($"Path casing conflicts with an existing entry: {Path.Combine(current, segment)}");
        }
      }
      current = Path.Combine(current, segment);
      CheckLink(current);
    }
    return current;
  }

  public static void CheckLink( string path )
  {
    if (File.Exists(path) || Directory.Exists(path) || new FileInfo(path).LinkTarget is not null) {
      if ((File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0) {
        throw new TaskException($"Shared task operations refuse symbolic links/reparse points: {path}");
      }
    }
  }

  public static void AtomicWrite( string path, byte[] bytes )
  {
    Directory.CreateDirectory(Path.GetDirectoryName(path)!);
    var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
    try {
      using (var stream = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None)) {
        stream.Write(bytes);
        stream.Flush(flushToDisk: true);
      }
      File.Move(temporary, path, overwrite: true);
    } finally {
      File.Delete(temporary);
    }
  }

  [GeneratedRegex("^[a-zA-Z][a-zA-Z0-9_-]*$")]
  private static partial Regex Identifier();
  [GeneratedRegex("^(COM|LPT)[1-9]$")]
  private static partial Regex Device();
}

public sealed record SharedTaskOptions( string CacheDirectory, string PrivateDirectory, string? OnlineDirectory = null )
{
  public static SharedTaskOptions FromEnvironment()
  {
    var home = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
    var cache = OperatingSystem.IsWindows()
      ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "dotask", "tasks")
      : OperatingSystem.IsMacOS() ? Path.Combine(home, "Library", "Caches", "dotask", "tasks")
      : Path.Combine(Environment.GetEnvironmentVariable("XDG_CACHE_HOME") ?? Path.Combine(home, ".cache"), "dotask", "tasks");
    var config = OperatingSystem.IsWindows()
      ? Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData)
      : Environment.GetEnvironmentVariable("XDG_CONFIG_HOME") ?? Path.Combine(home, ".config");
    return new(Path.GetFullPath(Environment.GetEnvironmentVariable("DOTASK_CACHE_HOME") ?? cache),
      Path.GetFullPath(Environment.GetEnvironmentVariable("DOTASK_PRIVATE_TASKS") ?? Path.Combine(config, "dotask", "private-tasks")),
      Environment.GetEnvironmentVariable("DOTASK_ONLINE_TASKS"));
  }
}

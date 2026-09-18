using System.Security.Cryptography;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace DoTask;

internal static class InstallationFiles
{
  internal const string BuildRecord = ".dotask-build.json";
  internal static readonly JsonSerializerOptions Json = new() { WriteIndented = true };
  internal static readonly StringComparison PathComparison = OperatingSystem.IsWindows()
    ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal;

  internal static void Name(string value, string label, bool version = false)
  {
    if (string.IsNullOrEmpty(value) || value.Length > 100 ||
        !Regex.IsMatch(value, version ? @"\A[A-Za-z0-9][A-Za-z0-9.+_-]*\z" : @"\A[A-Za-z0-9][A-Za-z0-9._-]*\z") ||
        value.EndsWith('.') || Regex.IsMatch(value.Split('.')[0], @"\A(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])\z", RegexOptions.IgnoreCase))
    {
      throw new TaskException($"Invalid {label}: '{value}'. Use a portable name without separators or reserved device names.");
    }
  }

  internal static string Relative(string value)
  {
    if (string.IsNullOrEmpty(value) || value.Contains('\\') || value.Contains(':') ||
        value.Split('/').Any(p => p is "" or "." or ".." || p.Any(c => c < 32 || "\"<>|?*".Contains(c)) || p.EndsWith('.') || p.EndsWith(' ')))
    {
      throw new TaskException($"Expected a portable relative executable path: '{value}'.");
    }

    return value;
  }

  internal static string FullDirectory(string value)
  {
    if (string.IsNullOrWhiteSpace(value) || value.Any(c => c < 32 || c == '"'))
    {
      throw new TaskException("Installation directory must be a nonempty path without control characters or quotes.");
    }

    return Path.TrimEndingDirectorySeparator(Path.GetFullPath(value));
  }

  internal static bool Exists(string path) => File.Exists(path) || Directory.Exists(path) || new FileInfo(path).LinkTarget is not null;

  internal static void NotLink(string path)
  {
    if (Exists(path) && (File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0)
    {
      throw new TaskException($"Refusing to follow a link in managed installation files: {path}");
    }
  }

  // Resolve intentional user-level directory links (e.g. ~/bin), then work in
  // that physical directory. Links within an owned application are rejected.
  internal static string PhysicalDirectory(string path)
  {
    var full = FullDirectory(path);
    var current = Path.GetPathRoot(full)!;
    foreach (var part in full[current.Length..].Split(Path.DirectorySeparatorChar, StringSplitOptions.RemoveEmptyEntries))
    {
      current = Path.Combine(current, part);
      var directory = new DirectoryInfo(current);
      if (directory.LinkTarget is not null)
      {
        current = directory.ResolveLinkTarget(true)?.FullName ?? throw new TaskException($"Broken directory link: {current}");
      }

      if (File.Exists(current))
      {
        throw new TaskException($"Expected an installation directory: {current}");
      }
    }
    return FullDirectory(current);
  }

  internal static T Read<T>(string path)
  {
    NotLink(path);
    try { return JsonSerializer.Deserialize<T>(File.ReadAllBytes(path), Json) ?? throw new JsonException("Empty record."); }
    catch (JsonException ex) { throw new TaskException($"Invalid installation record '{path}': {ex.Message}"); }
  }

  internal static void Write<T>(string path, T value)
  {
    NotLink(path);
    var temp = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
    try
    {
      File.WriteAllText(temp, JsonSerializer.Serialize(value, Json) + "\n");
      File.Move(temp, path, overwrite: true);
    }
    finally { File.Delete(temp); }
  }

  internal static FileStream Lock(string directory)
  {
    Directory.CreateDirectory(directory);
    var path = Path.Combine(directory, ".dotask-install.lock");
    NotLink(path);
    // Let the runtime coordinate deletion with the exclusive lock. Deleting
    // separately after disposing would race another installer opening the file.
    try { return new FileStream(path, FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None, 1, FileOptions.DeleteOnClose); }
    catch (IOException) { throw new TaskException($"Another installer may be using '{directory}'. Retry after it finishes."); }
  }

  internal static async Task<PublishedFile[]> InventoryAsync(string root, bool installed, CancellationToken token)
  {
    var files = new List<PublishedFile>();
    var names = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
    async Task Walk(string directory)
    {
      NotLink(directory);
      foreach (var path in Directory.EnumerateFileSystemEntries(directory).Order(StringComparer.Ordinal))
      {
        token.ThrowIfCancellationRequested();
        NotLink(path);
        if (Directory.Exists(path)) { await Walk(path); continue; }
        var relative = Path.GetRelativePath(root, path).Replace(Path.DirectorySeparatorChar, '/');
        Relative(relative);
        if (relative == BuildRecord)
        {
          if (installed)
          {
            continue;
          }

          throw new TaskException($"Published output uses the reserved filename {BuildRecord}.");
        }
        if (!names.Add(relative))
        {
          throw new TaskException($"Published paths differ only in case: {relative}");
        }

        var mode = OperatingSystem.IsWindows() ? 0 : (int)File.GetUnixFileMode(path);
        await using var input = File.OpenRead(path);
        files.Add(new(relative, Convert.ToHexStringLower(await SHA256.HashDataAsync(input, token)), mode));
      }
    }
    await Walk(root);
    return files.OrderBy(f => f.Path, StringComparer.Ordinal).ToArray();
  }
}

internal sealed record PublishedFile(string Path, string Hash, int Mode);
internal sealed record InstalledBuild(string AppId, string Version, string Fingerprint, PublishedFile[] Files);

namespace DoTask;

public static class PortablePath
{
  public static string Resolve( string root, params string[] parts )
  {
    ArgumentException.ThrowIfNullOrWhiteSpace(root);
    var path = root;
    foreach (var part in parts) {
      ArgumentNullException.ThrowIfNull(part);
      if (part.Contains('\0')) {
        throw new TaskException("A path cannot contain a NUL character.");
      }
      if (!OperatingSystem.IsWindows() &&
          ((part.Length >= 2 && char.IsAsciiLetter(part[0]) && part[1] == ':') || part.StartsWith(@"\\"))) {
        throw new TaskException($"Windows rooted path '{part}' cannot be resolved on this host.");
      }
      var normalized = part.Replace('\\', System.IO.Path.DirectorySeparatorChar)
        .Replace('/', System.IO.Path.DirectorySeparatorChar);
      path = System.IO.Path.Combine(path, normalized);
    }
    return System.IO.Path.GetFullPath(path);
  }
}

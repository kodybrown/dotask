using System.IO.Compression;

namespace DoTask;

public sealed class FileSystemHelpers
{
  private readonly string _root;
  internal FileSystemHelpers(string root) => _root = root;

  public string CreateDirectory(string path) => Directory.CreateDirectory(Resolve(path)).FullName;
  public bool FileExists(string path) => File.Exists(Resolve(path));
  public bool DirectoryExists(string path) => Directory.Exists(Resolve(path));
  public string ReadText(string path) => File.ReadAllText(Resolve(path));

  public void WriteText(string path, string text)
  {
    var destination = Resolve(path);
    Directory.CreateDirectory(System.IO.Path.GetDirectoryName(destination)!);
    File.WriteAllText(destination, text);
  }

  public void CopyFile(string source, string destination, bool overwrite = false)
  {
    var output = Resolve(destination);
    Directory.CreateDirectory(System.IO.Path.GetDirectoryName(output)!);
    File.Copy(Resolve(source), output, overwrite);
  }

  public void DeleteFile(string path) => File.Delete(Resolve(path));

  public void DeleteDirectory(string path)
  {
    var resolved = Resolve(path);
    var comparison = OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal;
    if (System.IO.Path.TrimEndingDirectorySeparator(resolved).Equals(System.IO.Path.TrimEndingDirectorySeparator(_root), comparison)
        || System.IO.Path.GetPathRoot(resolved)!.Equals(resolved, comparison))
    {
      throw new TaskException("Refusing to delete the project root or a filesystem root.");
    }
    if (Directory.Exists(resolved))
    {
      Directory.Delete(resolved, recursive: true);
    }
  }

  public IEnumerable<string> FindFiles(string directory, string pattern = "*", bool recursive = false) =>
    Directory.EnumerateFiles(Resolve(directory), pattern,
      recursive ? SearchOption.AllDirectories : SearchOption.TopDirectoryOnly);

  public void CreateZip(string sourceDirectory, string destinationFile)
  {
    var destination = Resolve(destinationFile);
    Directory.CreateDirectory(System.IO.Path.GetDirectoryName(destination)!);
    ZipFile.CreateFromDirectory(Resolve(sourceDirectory), destination);
  }

  private string Resolve(string path) => PortablePath.Resolve(_root, path);
}

#:include ../src/Dotask.Cli/Metadata/MetadataReader.cs
#:include ../src/Dotask.Cli/Metadata/TargetDefinition.cs
#:package Microsoft.CodeAnalysis.CSharp@5.0.0
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.RegularExpressions;
using DoTask;
using DoTask.Cli.Metadata;

/// <summary>Generate the shared-task catalog, or verify that it matches the sources.</summary>
/// <option name="verify" alias="v" type="bool" default="false">Check the existing catalog without writing files.</option>
/// <option name="root" type="path" default="shared-tasks" completion="directory">Directory containing the shared task groups.</option>
/// <example>dotask catalog</example>
/// <example>dotask catalog --verify</example>
public static class Target
{
  public static void Main()
  {
    var project = BuildContext.Current;
    var root = project.Parameters.GetPath("root");
    var content = Generate(root);
    var destination = Path.Combine(root, "catalog.json");
    if (project.Parameters.Get<bool>("verify")) {
      if (!File.Exists(destination) || !File.ReadAllBytes(destination).AsSpan().SequenceEqual(content)) {
        throw new TaskException("catalog.json is stale. Run ./build.sh catalog (Windows: build.cmd catalog).");
      }
      Console.WriteLine("Shared task catalog is current.");
      return;
    }
    File.WriteAllBytes(destination, content);
    Console.WriteLine($"Wrote {destination}");
  }

  private sealed record CatalogFile( string Path, string Sha256 );
  private sealed record CatalogTask( string Id, string EntryPoint, string Description, string Runtime,
    CatalogFile[] Files, string[] Requires );

  private static byte[] Generate( string directory )
  {
    var root = Path.GetFullPath(directory);
    if (!Directory.Exists(root)) {
      throw new TaskException($"Shared task directory is missing: {root}");
    }
    RejectLink(root);
    List<CatalogTask> tasks = [];
    foreach (var group in Directory.EnumerateDirectories(root).Order(StringComparer.Ordinal)) {
      if (Path.GetFileName(group).StartsWith('.') || Path.GetFileName(group).StartsWith('_')) {
        continue;
      }
      RejectLink(group);
      foreach (var source in Directory.EnumerateFiles(group, "*.cs").Order(StringComparer.Ordinal)) {
        if (Path.GetFileName(source).StartsWith('.') || Path.GetFileName(source).StartsWith('_')) {
          continue;
        }
        RejectLink(source);
        var relative = Path.GetRelativePath(root, source).Replace('\\', '/');
        var id = relative[..^3];
        if (!Regex.IsMatch(id, @"\A[a-zA-Z][a-zA-Z0-9_-]*/[a-zA-Z][a-zA-Z0-9_-]*\z")) {
          throw new TaskException($"Invalid task ID: {id}");
        }
        var manifest = Path.ChangeExtension(source, ".task.json");
        if (File.Exists(manifest)) {
          throw new TaskException($"Move '{manifest}' into XML <requires task=\"...\" /> / <requires file=\"...\" /> comments in '{source}', then remove the .task.json file.");
        }
        var metadata = MetadataReader.Read(source, root);
        if (metadata.Error is not null) {
          throw new TaskException($"{relative}: {metadata.Error}");
        }
        var support = metadata.Requirements.Where(r => r.Kind == "file").Select(r => r.Value);
        var requires = metadata.Requirements.Where(r => r.Kind == "task").Select(r => r.Value).ToArray();
        var files = support.Append(relative).Distinct(StringComparer.Ordinal).Order(StringComparer.Ordinal)
          .Select(name => new CatalogFile(name, Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(ResolveFile(root, name))))))
          .ToArray();
        tasks.Add(new(id, relative, metadata.Description, "csharp", files, requires));
      }
    }
    var ids = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
    foreach (var task in tasks) {
      if (!ids.Add(task.Id)) {
        throw new TaskException($"Duplicate task ID: {task.Id}");
      }
    }
    foreach (var task in tasks) {
      foreach (var required in task.Requires) {
        if (!ids.Contains(required)) {
          throw new TaskException($"{task.Id} requires missing task: {required}");
        }
      }
    }
    var options = new JsonSerializerOptions {
      PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
      WriteIndented = true,
      Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping
    };
    var json = JsonSerializer.Serialize(new { version = 1, tasks }, options).ReplaceLineEndings("\n") + "\n";
    return Encoding.UTF8.GetBytes(json);
  }

  private static string ResolveFile( string root, string name )
  {
    var parts = name.Split('/');
    if (parts.Any(part => part is "" or "." or "..") || name.Contains('\\') || name.Contains(':')) {
      throw new TaskException($"File must be a portable relative path inside the source: {name}");
    }
    var current = root;
    foreach (var part in parts) {
      current = Path.Combine(current, part);
      RejectLink(current);
    }
    return current;
  }

  private static void RejectLink( string path )
  {
    if ((File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0) {
      throw new TaskException($"Shared catalog sources cannot be symbolic links: {path}");
    }
  }
}

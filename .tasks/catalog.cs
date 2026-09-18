#:include _support/TaskCatalog.cs
#:include ../src/Dotask.Cli/Metadata/MetadataReader.cs
#:include ../src/Dotask.Cli/Metadata/TargetDefinition.cs
#:package Microsoft.CodeAnalysis.CSharp@5.0.0
using DoTask;
using DoTask.RepositoryTasks;

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
    var content = TaskCatalog.Generate(root);
    var destination = Path.Combine(root, "catalog.json");
    if (project.Parameters.Get<bool>("verify"))
    {
      if (!File.Exists(destination) || !File.ReadAllBytes(destination).AsSpan().SequenceEqual(content))
      {
        throw new TaskException("catalog.json is stale. Run ./build.sh catalog (Windows: build.cmd catalog).");
      }
      Console.WriteLine("Shared task catalog is current.");
      return;
    }
    File.WriteAllBytes(destination, content);
    Console.WriteLine($"Wrote {destination}");
  }
}

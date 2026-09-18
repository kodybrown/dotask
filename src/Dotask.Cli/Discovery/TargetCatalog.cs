using DoTask.Cli.Metadata;

namespace DoTask.Cli.Discovery;

public sealed class TargetCatalog
{
  public IReadOnlyList<TargetDefinition> Targets { get; }
  private readonly Dictionary<string, TargetDefinition[]> _fullNames;
  private readonly Dictionary<string, TargetDefinition[]> _aliases;

  public TargetCatalog( string directory )
  {
    var targets = (Directory.Exists(directory) ? EnumerateSources(directory) : [])
      .Select(file => MetadataReader.Read(file, directory)).OrderBy(t => t.Name, StringComparer.OrdinalIgnoreCase)
      .ThenBy(t => t.FilePath, StringComparer.Ordinal).ToArray();
    var duplicates = targets.GroupBy(t => t.Name, StringComparer.OrdinalIgnoreCase)
      .Where(g => g.Count() > 1).ToDictionary(g => g.Key,
        g => $"Ambiguous full target name '{g.Key}': {string.Join(", ", g.Select(t => Path.GetRelativePath(directory, t.FilePath)))}.",
        StringComparer.OrdinalIgnoreCase);
    Targets = targets.Select(t => duplicates.TryGetValue(t.Name, out var error) ? t with { Error = error } : t).ToArray();
    _fullNames = Targets.GroupBy(t => t.Name, StringComparer.OrdinalIgnoreCase)
      .ToDictionary(g => g.Key, g => g.ToArray(), StringComparer.OrdinalIgnoreCase);
    _aliases = Targets.SelectMany(t => Aliases(t).Distinct(StringComparer.OrdinalIgnoreCase).Select(alias => (alias, target: t)))
      .GroupBy(x => x.alias, StringComparer.OrdinalIgnoreCase)
      .ToDictionary(g => g.Key, g => g.Select(x => x.target).ToArray(), StringComparer.OrdinalIgnoreCase);
  }

  internal static IEnumerable<string> EnumerateSources( string directory )
  {
    foreach (var entry in new DirectoryInfo(directory).EnumerateFileSystemInfos().OrderBy(e => e.Name, StringComparer.Ordinal)) {
      if (entry.Name.StartsWith('.') || entry.Name.StartsWith('_') || (entry.Attributes & FileAttributes.ReparsePoint) != 0) {
        continue;
      }
      if (entry is DirectoryInfo child) {
        if (child.Name is "bin" or "obj" or "node_modules") {
          continue;
        }
        foreach (var file in EnumerateSources(child.FullName)) {
          yield return file;
        }
      } else if (entry.Extension.Equals(".cs", StringComparison.OrdinalIgnoreCase)) {
        yield return entry.FullName;
      }
    }
  }

  private static IEnumerable<string> Aliases( TargetDefinition target )
  {
    if (target.ShortName is { } shortName && !MetadataReader.ReservedCommands.Contains(shortName, StringComparer.OrdinalIgnoreCase)) {
      yield return shortName;
    }
    var parts = target.Name.Split('/');
    if (parts.Length > 2) {
      yield return string.Join('/', parts[^2..]);
    }
    if (parts.Length > 1) {
      // Compatibility with the original '<group> <target>.cs' command spelling.
      yield return string.Join('-', parts[^2..]);
    }
  }

  public TargetDefinition Get( string name )
  {
    var target = Find(name) ?? throw new TaskException($"Unknown target '{name}'. Run dotask help to list targets.");
    if (target.Error is not null) {
      throw new TaskException(target.Error);
    }
    return target;
  }

  public TargetDefinition? Find( string name )
  {
    if (_fullNames.TryGetValue(name, out var exact)) {
      if (exact.Length > 1) {
        throw new TaskException(exact[0].Error!);
      }
      return exact[0];
    }
    if (_aliases.TryGetValue(name, out var matches)) {
      if (matches.Length > 1) {
        throw new TaskException($"Ambiguous target '{name}'. Use {string.Join(" or ", matches.Select(t => $"'dotask {t.Name}'").Distinct(StringComparer.OrdinalIgnoreCase))}.");
      }
      return matches[0];
    }
    return null;
  }

  public string? AliasFor( TargetDefinition target ) => Aliases(target).FirstOrDefault(alias =>
    !_fullNames.ContainsKey(alias) && _fullNames[target.Name].Length == 1 && _aliases[alias].Length == 1);

  public IEnumerable<string> NamesFor( TargetDefinition target ) => new[] { target.Name }.Concat(Aliases(target)
    .Where(alias => !_fullNames.ContainsKey(alias) && _fullNames[target.Name].Length == 1 && _aliases[alias].Length == 1))
    .Distinct(StringComparer.OrdinalIgnoreCase);

  public string DisplayName( TargetDefinition target ) => AliasFor(target) is { } alias ? $"{alias} ({target.Name})" : target.Name;
}

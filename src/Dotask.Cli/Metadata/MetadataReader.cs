using System.Text.Json;
using System.Text.RegularExpressions;
using DoTask.Cli.Configuration;
using YamlDotNet.Core;
using YamlDotNet.RepresentationModel;

namespace DoTask.Cli.Metadata;

// The reference CLI shares the native header contract. Ordinary comments keep
// metadata independent of compiler syntax, entry-point shape and task execution.
public static partial class MetadataReader
{
  public static TargetDefinition Read( string file, string? taskDirectory = null )
  {
    var name = Path.GetFileNameWithoutExtension(file);
    string? shortName = null;
    try {
      (name, shortName) = ReadName(file, taskDirectory);
      var lines = File.ReadAllText(file).TrimStart('\uFEFF').Replace("\r\n", "\n", StringComparison.Ordinal).Split('\n');
      var first = Array.FindIndex(lines, line => !string.IsNullOrWhiteSpace(line) && !line.StartsWith("#!", StringComparison.Ordinal));
      if (first < 0 || !lines[first].Trim().StartsWith("// dotask:", StringComparison.Ordinal)) {
        return new(name, file, "(no description)", [], [], [], null, [], ShortName: shortName);
      }

      if (lines[first].Trim() != "// dotask: 1") {
        throw new TaskException("Unsupported dotask metadata version.");
      }

      var header = new System.Text.StringBuilder();
      var ended = false;
      foreach (var line in lines.Skip(first + 1)) {
        var comment = line.TrimStart();
        if (!comment.StartsWith("//", StringComparison.Ordinal)) {
          throw new TaskException("Metadata must remain inside the leading comment header.");
        }

        comment = comment[2..];
        if (comment.Trim() == "end-dotask") { ended = true; break; }
        header.AppendLine(comment.StartsWith(' ') ? comment[1..] : comment);
      }
      if (!ended) {
        throw new TaskException("Unterminated dotask metadata header.");
      }

      var yaml = new YamlStream();
      yaml.Load(new StringReader(header.Length == 0 ? "{}" : header.ToString()));
      if (yaml.Documents.Count != 1 || yaml.Documents[0].RootNode is not YamlMappingNode mapping) {
        throw new TaskException("Task metadata must contain one YAML mapping.");
      }

      var root = JsonSerializer.SerializeToElement(ProjectConfiguration.ConvertNode(mapping, 0, file));
      Keys(root, "description", "remarks", "examples", "capabilities", "options", "requires");
      var options = Sequence(root, "options").Select(ReadOption).ToArray();
      var tokens = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
      foreach (var option in options) {
        foreach (var token in new[] { option.Name, option.Alias }.OfType<string>()) {
          if (!tokens.Add(token) || ReservedOptions.Contains(token, StringComparer.OrdinalIgnoreCase)) {
            throw new TaskException($"Duplicate or reserved option/alias '{token}'.");
          }
        }
      }

      var requirements = Sequence(root, "requires").Select(item =>
      {
        Keys(item, "kind", "value");
        var kind = Text(item, "kind", "");
        var value = Text(item, "value", "");
        if (kind is not ("tool" or "setting" or "task" or "file" or "os") || string.IsNullOrWhiteSpace(value)) {
          throw new TaskException("A requirement needs one tool, setting, task, file or os kind and a nonempty value.");
        }

        if (kind == "file" && (value.Contains('\\') || value.Contains(':') || value.Split('/').Any(p => p is "" or "." or ".."))) {
          throw new TaskException("Support files must use a portable relative path inside the task directory.");
        }

        return new Requirement(kind, value);
      }).ToArray();
      return new(name, file, Text(root, "description", "(no description)"), options, requirements,
        Strings(root, "capabilities"), root.TryGetProperty("remarks", out var remarks) && remarks.ValueKind != JsonValueKind.Null ? Text(root, "remarks", "") : null,
        Strings(root, "examples"), ShortName: shortName);
    } catch (Exception ex) when (ex is YamlException or TaskException or IOException or UnauthorizedAccessException) {
      return new(name, file, "", [], [], [], null, [], $"Metadata error: {ex.Message}", shortName);
    }
  }

  private static OptionDefinition ReadOption( JsonElement item )
  {
    Keys(item, "name", "alias", "type", "description", "default", "required", "choices", "completion");
    var name = Text(item, "name", "");
    var alias = item.TryGetProperty("alias", out var aliasValue) && aliasValue.ValueKind != JsonValueKind.Null ? Text(item, "alias", "") : null;
    var type = Text(item, "type", "string");
    if (!Identifier().IsMatch(name)) {
      throw new TaskException($"Invalid option name '{name}'.");
    }

    if (alias is not null && (alias.Length != 1 || !char.IsAsciiLetter(alias[0]))) {
      throw new TaskException($"Option '{name}' must have a single-letter alias.");
    }

    if (type is not ("string" or "bool" or "int" or "number" or "path")) {
      throw new TaskException($"Option '{name}' has unsupported type '{type}'.");
    }

    var completion = item.TryGetProperty("completion", out var completionValue) && completionValue.ValueKind != JsonValueKind.Null ? Text(item, "completion", "") : null;
    if (completion is not null and not ("file" or "directory" or "none")) {
      throw new TaskException($"Option '{name}' has unsupported completion '{completion}'.");
    }

    string? fallback = null;
    if (item.TryGetProperty("default", out var value)) {
      if (value.ValueKind is JsonValueKind.Null or JsonValueKind.Array or JsonValueKind.Object) {
        throw new TaskException("Option defaults must be scalars.");
      }

      fallback = value.ValueKind == JsonValueKind.String ? value.GetString() : value.GetRawText();
    }
    var required = false;
    if (item.TryGetProperty("required", out var flag)) {
      if (flag.ValueKind is not (JsonValueKind.True or JsonValueKind.False)) {
        throw new TaskException("required must be a boolean.");
      }

      required = flag.GetBoolean();
    }
    return new(name, alias, type, Text(item, "description", ""), fallback, required, Strings(item, "choices"), completion);
  }
  private static void Keys( JsonElement root, params string[] keys )
  {
    if (root.ValueKind != JsonValueKind.Object) {
      throw new TaskException("Expected a YAML mapping.");
    }

    foreach (var item in root.EnumerateObject()) {
      if (!keys.Contains(item.Name, StringComparer.Ordinal)) {
        throw new TaskException($"Unknown metadata key '{item.Name}'.");
      }
    }
  }
  private static string Text( JsonElement root, string key, string fallback )
    => !root.TryGetProperty(key, out var value) ? fallback : value.ValueKind == JsonValueKind.String ? value.GetString()! : throw new TaskException($"{key} must be a string.");
  private static IEnumerable<JsonElement> Sequence( JsonElement root, string key )
    => !root.TryGetProperty(key, out var value) ? [] : value.ValueKind == JsonValueKind.Array ? value.EnumerateArray() : throw new TaskException($"{key} must be a sequence.");
  private static string[] Strings( JsonElement root, string key ) => Sequence(root, key).Select(value =>
    value.ValueKind == JsonValueKind.String ? value.GetString()! : throw new TaskException($"{key} entries must be strings.")).ToArray();
  internal static (string Name, string? ShortName) ReadName( string file, string? taskDirectory )
  {
    var name = Path.GetFileNameWithoutExtension(file);
    string? shortName = null;
    var parts = name.Split(' ');
    if (parts.Length > 2 || parts.Any(part => !Identifier().IsMatch(part))) {
      throw new TaskException($"Invalid target filename '{Path.GetFileName(file)}'. Expected a target name or '<group> <target>' with one space.");
    }
    if (parts.Length == 2) {
      shortName = parts[1];
      name = parts[0] + "-" + shortName;
    }
    if (taskDirectory is not null) {
      var relative = Path.GetRelativePath(taskDirectory, file).Replace(Path.DirectorySeparatorChar, '/');
      var parents = relative.Split('/')[..^1];
      if (parents.Where(( parent, index ) => !(index == 0 && parent == "_")).Any(parent => !Identifier().IsMatch(parent))) {
        throw new TaskException($"Invalid target directory in '{relative}'. Use letters, digits, underscores, or hyphens, starting with a letter.");
      }
      if (parents.Length > 0) {
        shortName ??= name;
        name = string.Join('/', parents.Append(name));
      }
    }
    if (ReservedCommands.Contains(name, StringComparer.OrdinalIgnoreCase)) {
      throw new TaskException($"Invalid or reserved target name '{name}'.");
    }
    return (name, shortName);
  }

  public static readonly string[] ReservedCommands = ["help", "completion", "__complete", "__exec"];
  public static readonly string[] ReservedOptions = ["help", "h", "use-dir", "version", "verbose"];

  [GeneratedRegex("^[a-zA-Z][a-zA-Z0-9_-]*$")]
  private static partial Regex Identifier();
}

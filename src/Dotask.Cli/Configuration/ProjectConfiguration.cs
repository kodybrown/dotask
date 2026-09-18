using System.Globalization;
using System.Text.Json;
using YamlDotNet.Core;
using YamlDotNet.RepresentationModel;

namespace DoTask.Cli.Configuration;

public sealed record ProjectConfiguration(JsonElement Settings, JsonElement TargetDefaults, string? Name = null, string? Description = null)
{
  public static ProjectConfiguration Empty { get; } = new(JsonSerializer.SerializeToElement(new { }), JsonSerializer.SerializeToElement(new { }));

  public static ProjectConfiguration Load(string directory, string? rootDirectory = null)
  {
    var legacy = Path.Combine(directory, "config.yaml");
    var current = Path.Combine(rootDirectory ?? Directory.GetParent(directory)!.FullName, ".dotasks.yaml");
    if (File.Exists(current) && File.Exists(legacy))
    {
      throw new TaskException("Both .dotasks.yaml and the legacy task-directory config.yaml exist. Keep the project configuration in .dotasks.yaml and remove the duplicate after reviewing it.");
    }
    var file = File.Exists(current) ? current : legacy;
    if (!File.Exists(file))
    {
      return Empty;
    }
    try
    {
      using var reader = File.OpenText(file);
      var yaml = new YamlStream();
      yaml.Load(reader);
      if (yaml.Documents.Count != 1 || yaml.Documents[0].RootNode is not YamlMappingNode root)
      {
        throw new TaskException("config.yaml must contain one mapping document.");
      }
      var document = JsonSerializer.SerializeToElement(ConvertNode(root, 0));
      foreach (var property in document.EnumerateObject())
      {
        if (!new[] { "version", "name", "description", "settings", "targets" }.Contains(property.Name, StringComparer.OrdinalIgnoreCase))
        {
          throw new TaskException($"Unknown config.yaml key '{property.Name}'. Expected version, name, description, settings, or targets.");
        }
      }
      var values = new Values(document, directory);
      if (values.Get("version", 1) != 1)
      {
        throw new TaskException("Unsupported config.yaml version. Expected version: 1.");
      }
      var settings = values.TryFind("settings", out var s) ? s : Empty.Settings;
      var targets = values.TryFind("targets", out var t) ? t : Empty.TargetDefaults;
      if (settings.ValueKind != JsonValueKind.Object || targets.ValueKind != JsonValueKind.Object)
      {
        throw new TaskException("config.yaml settings and targets must be mappings.");
      }
      foreach (var target in targets.EnumerateObject())
      {
        if (target.Value.ValueKind != JsonValueKind.Object
          || target.Value.EnumerateObject().Any(p => !p.Name.Equals("defaults", StringComparison.OrdinalIgnoreCase)
            || p.Value.ValueKind != JsonValueKind.Object))
        {
          throw new TaskException($"config.yaml targets.{target.Name} only supports a defaults mapping.");
        }
      }
      return new(settings.Clone(), targets.Clone(), ReadText(values, "name"), ReadText(values, "description"));
    }
    catch (YamlException ex)
    {
      throw new TaskException($"Invalid {Path.GetFileName(file)}: {ex.Message}");
    }
    catch (TaskException ex) when (file == current)
    {
      throw new TaskException(ex.Message.Replace("config.yaml", ".dotasks.yaml", StringComparison.Ordinal));
    }
  }

  public Dictionary<string, string> DefaultsFor(string target)
  {
    var values = new Values(TargetDefaults, "");
    if (!values.TryFind(target + ".defaults", out var defaults))
    {
      return new(StringComparer.OrdinalIgnoreCase);
    }
    return defaults.EnumerateObject().ToDictionary(p => p.Name, p => Values.ToArgument(p.Value), StringComparer.OrdinalIgnoreCase);
  }

  private static string? ReadText(Values values, string key)
  {
    if (!values.TryFind(key, out var value))
    {
      return null;
    }
    if (value.ValueKind != JsonValueKind.String)
    {
      throw new TaskException($"config.yaml {key} must be a string.");
    }
    var text = value.GetString()!.Trim();
    return text.Length == 0 ? null : text;
  }

  private static object? ConvertNode(YamlNode node, int depth)
  {
    if (depth > 32)
    {
      throw new TaskException("config.yaml nesting exceeds 32 levels (or contains a recursive alias).");
    }
    switch (node)
    {
      case YamlMappingNode mapping:
        var result = new Dictionary<string, object?>(StringComparer.OrdinalIgnoreCase);
        foreach (var pair in mapping.Children)
        {
          if (pair.Key is not YamlScalarNode { Value: { Length: > 0 } key } || !result.TryAdd(key, ConvertNode(pair.Value, depth + 1)))
          {
            throw new TaskException("config.yaml requires nonempty string keys unique regardless of case.");
          }
        }
        return result;
      case YamlSequenceNode sequence:
        return sequence.Children.Select(n => ConvertNode(n, depth + 1)).ToArray();
      case YamlScalarNode scalar:
        var value = scalar.Value ?? "";
        if (scalar.Style is ScalarStyle.SingleQuoted or ScalarStyle.DoubleQuoted or ScalarStyle.Literal or ScalarStyle.Folded
          || scalar.Tag.ToString() == "tag:yaml.org,2002:str")
        {
          return value;
        }
        if (value is "" or "~" || value.Equals("null", StringComparison.OrdinalIgnoreCase))
        {
          return null;
        }
        if (bool.TryParse(value, out var boolean))
        {
          return boolean;
        }
        if (long.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out var integer))
        {
          return integer;
        }
        if (double.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out var number) && double.IsFinite(number))
        {
          return number;
        }
        return value;
      default:
        throw new TaskException("Unsupported YAML value in config.yaml.");
    }
  }
}

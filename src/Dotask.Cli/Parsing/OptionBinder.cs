using System.Globalization;
using System.Text.Json;
using DoTask.Cli.Configuration;
using DoTask.Cli.Metadata;

namespace DoTask.Cli.Parsing;

public static class OptionBinder
{
  public static JsonElement Bind(TargetDefinition target, ProjectConfiguration config, IReadOnlyList<string> arguments,
    string rootDirectory, bool requireValues = true)
  {
    var supplied = Parse(target, arguments);
    var defaults = config.DefaultsFor(target.Name);
    foreach (var key in defaults.Keys)
    {
      if (!target.Options.Any(o => o.Name.Equals(key, StringComparison.OrdinalIgnoreCase)))
      {
        throw new TaskException($"Target '{target.Name}' has no option '{key}' configured in config.yaml.");
      }
    }
    var values = new Dictionary<string, object?>(StringComparer.OrdinalIgnoreCase);
    foreach (var option in target.Options)
    {
      var value = supplied.GetValueOrDefault(option.Name) ?? defaults.GetValueOrDefault(option.Name) ?? option.Default;
      if (value is null)
      {
        if (requireValues && option.Required)
        {
          throw new TaskException($"Target '{target.Name}' requires --{option.Name}.");
        }
        if (option.Type == "bool")
        {
          values[option.Name] = false;
        }
        continue;
      }
      values[option.Name] = ConvertValue(option, value, rootDirectory);
    }
    return JsonSerializer.SerializeToElement(values);
  }

  public static OptionDefinition? Find(TargetDefinition target, string name) => target.Options.FirstOrDefault(o =>
    o.Name.Equals(name, StringComparison.OrdinalIgnoreCase) || (o.Alias?.Equals(name, StringComparison.OrdinalIgnoreCase) ?? false));

  public static Dictionary<string, string> Parse(TargetDefinition target, IReadOnlyList<string> arguments)
  {
    var supplied = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
    for (var index = 0; index < arguments.Count; index++)
    {
      var argument = arguments[index];
      var token = argument.StartsWith("--") ? argument[2..] : argument.StartsWith('-') ? argument[1..] : argument;
      var separator = token.IndexOf('=');
      var name = separator < 0 ? token : token[..separator];
      var option = Find(target, name) ?? throw new TaskException($"Unknown option '{name}' for target '{target.Name}'.");
      string value;
      if (separator >= 0)
      {
        value = token[(separator + 1)..];
      }
      else if (!argument.StartsWith('-'))
      {
        throw new TaskException($"Expected --{name} VALUE or {name}=VALUE.");
      }
      else if (option.Type == "bool" && (index + 1 >= arguments.Count || !bool.TryParse(arguments[index + 1], out _)))
      {
        value = "true";
      }
      else
      {
        if (index + 1 >= arguments.Count || arguments[index + 1].StartsWith("--"))
        {
          throw new TaskException($"Missing value for --{option.Name}.");
        }
        value = arguments[++index];
      }
      if (!supplied.TryAdd(option.Name, value))
      {
        throw new TaskException($"Option '{option.Name}' was supplied more than once.");
      }
    }
    return supplied;
  }

  private static object ConvertValue(OptionDefinition option, string value, string rootDirectory)
  {
    if (option.Choices.Length > 0)
    {
      value = option.Choices.FirstOrDefault(c => c.Equals(value, StringComparison.OrdinalIgnoreCase))
        ?? throw new TaskException($"Invalid value for --{option.Name}: '{value}'. Choose {string.Join(", ", option.Choices)}.");
    }
    return option.Type switch
    {
      "string" => value,
      "path" => PortablePath.Resolve(rootDirectory, value),
      "bool" when bool.TryParse(value, out var result) => result,
      "int" when int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out var result) => result,
      "number" when double.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out var result) && double.IsFinite(result) => result,
      _ => throw new TaskException($"Invalid {option.Type} value for --{option.Name}: '{value}'.")
    };
  }
}

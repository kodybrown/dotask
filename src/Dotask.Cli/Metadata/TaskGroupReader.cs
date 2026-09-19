using System.Text.Json;
using DoTask.Cli.Configuration;
using YamlDotNet.Core;
using YamlDotNet.RepresentationModel;

namespace DoTask.Cli.Metadata;

internal static class TaskGroupReader
{
  public static TargetDefinition Read( string file, string directory )
  {
    var name = Path.GetFileNameWithoutExtension(file);
    string? shortName = null;
    try {
      (name, shortName) = MetadataReader.ReadName(file, directory);
      using var reader = File.OpenText(file);
      var yaml = new YamlStream();
      yaml.Load(reader);
      if (yaml.Documents.Count != 1 || yaml.Documents[0].RootNode is not YamlMappingNode root) {
        throw new TaskException("A .task file must contain one mapping document.");
      }
      var document = JsonSerializer.SerializeToElement(ProjectConfiguration.ConvertNode(root, 0, Path.GetFileName(file)));
      CheckKeys(document, "description", "require_at_least_1_step", "steps");
      var description = document.TryGetProperty("description", out var text)
        ? ReadText(text, "description") : "(no description)";
      var required = ReadFlag(document, "require_at_least_1_step");
      if (!document.TryGetProperty("steps", out var steps) || steps.ValueKind != JsonValueKind.Array) {
        throw new TaskException("steps must be a sequence (use steps: [] for an empty group).");
      }
      var parsed = steps.EnumerateArray().Select(step =>
      {
        CheckKeys(step, "run", "optional", "with");
        if (!step.TryGetProperty("run", out var run)) {
          throw new TaskException("Every step requires run.");
        }
        var target = ReadText(run, "run");
        if (string.IsNullOrWhiteSpace(target)) {
          throw new TaskException("run must name a target.");
        }
        var parameters = step.TryGetProperty("with", out var value) ? value : ProjectConfiguration.Empty.Settings;
        if (parameters.ValueKind != JsonValueKind.Object) {
          throw new TaskException("with must be a mapping of parameter names to values.");
        }
        foreach (var parameter in parameters.EnumerateObject()) {
          if (parameter.Value.ValueKind is JsonValueKind.Array or JsonValueKind.Object or JsonValueKind.Null) {
            throw new TaskException($"with.{parameter.Name} must be a string, boolean, or number.");
          }
        }
        return new TaskStep(target, ReadFlag(step, "optional"), parameters.Clone());
      }).ToArray();
      return new(name, file, description, [], [], [], null, [], ShortName: shortName, Group: new(required, parsed));
    } catch (Exception ex) when (ex is YamlException or TaskException or IOException or UnauthorizedAccessException) {
      return new(name, file, "", [], [], [], null, [], $"Metadata error in {Path.GetFileName(file)}: {ex.Message}", shortName);
    }
  }

  private static void CheckKeys( JsonElement mapping, params string[] keys )
  {
    if (mapping.ValueKind != JsonValueKind.Object) {
      throw new TaskException("Expected a YAML mapping.");
    }
    foreach (var property in mapping.EnumerateObject()) {
      if (!keys.Contains(property.Name, StringComparer.Ordinal)) {
        throw new TaskException($"Unknown .task key '{property.Name}'. Expected {string.Join(", ", keys)}.");
      }
    }
  }

  private static string ReadText( JsonElement value, string key ) => value.ValueKind == JsonValueKind.String
    ? value.GetString()! : throw new TaskException($"{key} must be a string.");

  private static bool ReadFlag( JsonElement mapping, string key )
  {
    if (!mapping.TryGetProperty(key, out var value)) {
      return false;
    }
    return value.ValueKind is JsonValueKind.True or JsonValueKind.False ? value.GetBoolean()
      : throw new TaskException($"{key} must be a boolean.");
  }
}

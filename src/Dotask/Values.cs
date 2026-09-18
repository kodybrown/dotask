using System.Text.Json;

namespace DoTask;

/// <summary>Case-insensitive, read-only values with typed access.</summary>
public sealed class Values
{
  private readonly JsonElement _values;
  private readonly string _rootDirectory;
  private static readonly JsonSerializerOptions JsonOptions = new() { PropertyNameCaseInsensitive = true };

  internal Values( JsonElement values, string rootDirectory )
  {
    _values = values.Clone();
    _rootDirectory = rootDirectory;
  }

  public bool Contains( string key ) => TryFind(key, out _);

  public T Get<T>( string key )
  {
    if (!TryFind(key, out var value)) {
      throw new TaskException($"Required value '{key}' is missing.");
    }
    try {
      if (typeof(T) == typeof(string) && value.ValueKind is not JsonValueKind.Object and not JsonValueKind.Array) {
        return (T)(object)(value.ValueKind == JsonValueKind.String ? value.GetString()! : value.ToString());
      }
      if (typeof(T).IsEnum && value.ValueKind == JsonValueKind.String) {
        return (T)Enum.Parse(typeof(T), value.GetString()!, ignoreCase: true);
      }
      return value.Deserialize<T>(JsonOptions) ?? throw new TaskException($"Value '{key}' is null.");
    } catch (Exception ex) when (ex is JsonException or ArgumentException or InvalidOperationException) {
      throw new TaskException($"Value '{key}' cannot be read as {typeof(T).Name}: {ex.Message}");
    }
  }

  public T Get<T>( string key, T fallback ) => Contains(key) ? Get<T>(key) : fallback;

  public string GetPath( string key ) => PortablePath.Resolve(_rootDirectory, Get<string>(key));

  internal bool TryFind( string key, out JsonElement value )
  {
    value = _values;
    foreach (var part in key.Split('.')) {
      if (value.ValueKind != JsonValueKind.Object) {
        return false;
      }
      var found = false;
      foreach (var property in value.EnumerateObject()) {
        if (property.Name.Equals(part, StringComparison.OrdinalIgnoreCase)) {
          value = property.Value;
          found = true;
          break;
        }
      }
      if (!found) {
        return false;
      }
    }
    return value.ValueKind != JsonValueKind.Null;
  }

  internal static string ToArgument( JsonElement value ) => value.ValueKind switch {
    JsonValueKind.String => value.GetString()!,
    JsonValueKind.True => "true",
    JsonValueKind.False => "false",
    JsonValueKind.Number => value.GetRawText(),
    _ => throw new TaskException("Target arguments must be strings, numbers, or booleans.")
  };
}

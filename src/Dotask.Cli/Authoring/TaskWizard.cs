using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using DoTask.Cli.Configuration;
using DoTask.Cli.Discovery;
using DoTask.Cli.Metadata;
using DoTask.Cli.Parsing;
using DoTask.Cli.SharedTasks;

namespace DoTask.Cli.Authoring;

internal sealed class TaskWizard( TextReader input, TextWriter output, CancellationToken token )
{
  public async Task RunAsync( TaskDirectory directory )
  {
    var catalog = new TargetCatalog(directory.DirectoryPath);
    var config = ProjectConfiguration.Load(directory.DirectoryPath, directory.RootDirectory);
    output.WriteLine("Create a YAML task group. Enter :cancel or press Ctrl+C to cancel. Nothing is written until save.");
    string name;
    string destination;
    while (true) {
      name = (await Ask("Task name (without extension): ")).Trim();
      try {
        if (!Regex.IsMatch(name, "^[a-zA-Z][a-zA-Z0-9_-]*$")) {
          throw new TaskException("Use a name starting with a letter, followed by letters, digits, underscores, or hyphens.");
        }
        destination = CheckDestination(directory, name);
        break;
      } catch (TaskException ex) {
        output.WriteLine(ex.Message);
      }
    }
    output.WriteLine($"Destination: {destination}");
    var description = await Ask("Description (Enter to omit): ");
    var steps = new List<TaskStep>();
    while (await YesNo("Add a step? [Y/n]: ", true)) {
      TargetDefinition? target = null;
      string run;
      while (true) {
        var search = await Ask("Search task names/descriptions (Enter lists all; :manual enters an absent task): ");
        if (search == ":manual") {
          run = (await Ask("Task name: ")).Trim();
          if (run.Length == 0 || run.Equals(name, StringComparison.OrdinalIgnoreCase)) {
            output.WriteLine("Enter a nonempty task name other than this group.");
            continue;
          }
          try {
            target = catalog.Find(run);
            if (target is not null) {
              if (target.Error is not null) {
                throw new TaskException(target.Error);
              }
              run = target.Name;
            }
            break;
          } catch (TaskException ex) {
            output.WriteLine(ex.Message);
            continue;
          }
        }
        var matches = catalog.Targets.Where(t => t.Error is null &&
          (t.Name.Contains(search, StringComparison.OrdinalIgnoreCase) || t.Description.Contains(search, StringComparison.OrdinalIgnoreCase))).ToArray();
        for (var i = 0; i < matches.Length; i++) {
          output.WriteLine($"  {i + 1}. {matches[i].Name} — {matches[i].Description}");
        }
        if (matches.Length == 0) {
          output.WriteLine("No matching tasks. Search again or use :manual.");
          continue;
        }
        var selection = await Ask("Task number (Enter to search again): ");
        if (!int.TryParse(selection, out var index) || index < 1 || index > matches.Length) {
          continue;
        }
        target = matches[index - 1];
        run = target.Name;
        break;
      }
      var optional = await YesNo("Optional (skip only when absent)? [y/N]: ", false);
      if (target is null) {
        output.WriteLine("Task is not installed; its parameter names and types cannot be checked.");
      }
      var parameters = new Dictionary<string, JsonElement>(StringComparer.OrdinalIgnoreCase);
      if (target is not null) {
        var defaults = config.DefaultsFor(target.Name);
        foreach (var option in target.Options) {
          output.WriteLine($"  {option.Name} ({option.Type}) — {option.Description}");
          if (option.Choices.Length > 0) {
            output.WriteLine("  Choices: " + string.Join(", ", option.Choices));
          }
          var effective = defaults.GetValueOrDefault(option.Name) ?? option.Default;
          output.WriteLine("  Default: " + (effective ?? (option.Type == "bool" ? "false" : "none")));
          var value = await Parameter(target, option, directory.RootDirectory, option.Required && effective is null);
          if (value is { } supplied) {
            parameters.Add(option.Name, supplied);
          }
        }
        OptionBinder.Bind(target, config, Arguments(parameters), directory.RootDirectory);
      } else {
        while (await YesNo("Add a parameter? [y/N]: ", false)) {
          var key = (await Ask("Parameter name: ")).Trim();
          if (!Regex.IsMatch(key, "^[a-zA-Z][a-zA-Z0-9_-]*$") || parameters.ContainsKey(key)) {
            output.WriteLine("Use a unique parameter name starting with a letter.");
            continue;
          }
          var type = await Ask("Type [string/bool/int/number] (Enter for string): ");
          type = type.Length == 0 ? "string" : type;
          if (type is not ("string" or "bool" or "int" or "number")) {
            output.WriteLine("Unknown type.");
            continue;
          }
          var option = new OptionDefinition(key, null, type, "", null, true, [], null);
          var placeholder = new TargetDefinition(run, "", "", [option], [], [], null, []);
          parameters.Add(key, (await Parameter(placeholder, option, directory.RootDirectory, true))!.Value);
        }
      }
      steps.Add(new(run, optional, JsonSerializer.SerializeToElement(parameters)));
    }
    for (var i = 0; i < steps.Count; i++) {
      output.WriteLine($"  {i + 1}. {steps[i].Run}" + (steps[i].Optional ? " (optional)" : ""));
    }
    if (steps.Count > 1) {
      while (true) {
        var order = await Ask("Step order (comma-separated numbers; Enter keeps this order): ");
        if (order.Length == 0) {
          break;
        }
        var indices = order.Split(',').Select(s => int.TryParse(s.Trim(), out var n) ? n - 1 : -1).ToArray();
        if (indices.Length != steps.Count || indices.Distinct().Count() != steps.Count || indices.Any(i => i < 0 || i >= steps.Count)) {
          output.WriteLine("List every step number exactly once.");
          continue;
        }
        steps = indices.Select(i => steps[i]).ToList();
        break;
      }
    }
    var required = await YesNo("Require at least one step to execute? [y/N]: ", false);
    var yaml = Serialize(description, required, steps);
    var parsed = TaskGroupReader.Read(destination, directory.DirectoryPath, yaml);
    if (parsed.Error is not null) {
      throw new TaskException(parsed.Error);
    }
    output.WriteLine("\nYAML preview:\n" + yaml);
    if (!await YesNo("Save this file? [y/N]: ", false)) {
      output.WriteLine("Cancelled; no file created.");
      return;
    }
    token.ThrowIfCancellationRequested();
    destination = CheckDestination(directory, name);
    Directory.CreateDirectory(directory.DirectoryPath);
    var temporary = Path.Combine(directory.DirectoryPath, ".task-wizard-" + Guid.NewGuid().ToString("N"));
    try {
      await File.WriteAllTextAsync(temporary, yaml, new UTF8Encoding(false), token);
      token.ThrowIfCancellationRequested();
      CheckDestination(directory, name);
      File.Move(temporary, destination, overwrite: false);
    } finally {
      if (File.Exists(temporary)) {
        File.Delete(temporary);
      }
    }
    output.WriteLine($"Created {destination}. Run dotask {name} (include --use-dir when using a custom task directory).");
  }

  private async Task<JsonElement?> Parameter( TargetDefinition target, OptionDefinition option, string root, bool required )
  {
    while (true) {
      var value = await Ask($"  {option.Name} value ({(required ? "required" : "Enter keeps default")}; :empty for empty string): ");
      if (value.Length == 0 && !required) {
        return null;
      }
      if (value.Length == 0) {
        continue;
      }
      value = value == ":empty" ? "" : value;
      try {
        var bound = OptionBinder.Bind(target with { Options = [option] }, ProjectConfiguration.Empty,
          [option.Name + "=" + value], root);
        return option.Type == "path" ? JsonSerializer.SerializeToElement(value) : bound.GetProperty(option.Name).Clone();
      } catch (TaskException ex) {
        output.WriteLine(ex.Message);
      }
    }
  }

  private async Task<string> Ask( string prompt )
  {
    token.ThrowIfCancellationRequested();
    output.Write(prompt);
    output.Flush();
    // Console.In blocks even through ReadLineAsync; keep Ctrl+C responsive.
    var line = await Task.Run(input.ReadLine, CancellationToken.None).WaitAsync(token);
    if (line is null or ":cancel") {
      throw new OperationCanceledException();
    }
    return line;
  }

  private async Task<bool> YesNo( string prompt, bool defaultValue )
  {
    while (true) {
      var answer = (await Ask(prompt)).Trim().ToLowerInvariant();
      if (answer.Length == 0) {
        return defaultValue;
      }
      if (answer is "y" or "yes") {
        return true;
      }
      if (answer is "n" or "no") {
        return false;
      }
      output.WriteLine("Enter yes or no.");
    }
  }

  private static string[] Arguments( Dictionary<string, JsonElement> parameters ) => parameters
    .Select(p => p.Key + "=" + Values.ToArgument(p.Value)).ToArray();

  private static string CheckDestination( TaskDirectory directory, string name )
  {
    var destination = SharedTaskFiles.Resolve(directory.DirectoryPath, name + ".task");
    MetadataReader.ReadName(destination, directory.DirectoryPath);
    if (File.Exists(destination) || Directory.Exists(destination)
      || new TargetCatalog(directory.DirectoryPath).Targets.Any(t => t.Name.Equals(name, StringComparison.OrdinalIgnoreCase))) {
      throw new TaskException($"A file or target named '{name}' already exists. Choose another name.");
    }
    return destination;
  }

  private static string Serialize( string description, bool required, List<TaskStep> steps )
  {
    var text = new StringBuilder();
    if (description.Length > 0) {
      text.AppendLine("description: " + JsonSerializer.Serialize(description));
    }
    if (required) {
      text.AppendLine("require_at_least_1_step: true");
    }
    text.AppendLine(steps.Count == 0 ? "steps: []" : "steps:");
    foreach (var step in steps) {
      text.AppendLine("  - run: " + JsonSerializer.Serialize(step.Run));
      if (step.Optional) {
        text.AppendLine("    optional: true");
      }
      if (step.Parameters.EnumerateObject().Any()) {
        text.AppendLine("    with:");
        foreach (var parameter in step.Parameters.EnumerateObject()) {
          text.AppendLine("      " + JsonSerializer.Serialize(parameter.Name) + ": " + parameter.Value.GetRawText());
        }
      }
    }
    return text.ToString().Replace("\r\n", "\n");
  }
}

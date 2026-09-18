using System.Text.Json;
using DoTask.Cli.Configuration;
using DoTask.Cli.Metadata;

namespace DoTask.Cli;

internal static class HelpWriter
{
  public static void Usage( HelpText output )
  {
    output.WriteLine("dotask - portable C# tasks");
    output.WriteLine("Usage: dotask [--use-dir PATH] [TARGET [OPTIONS]]");
    output.WriteLine("       dotask help [TARGET]");
    output.WriteLine("       dotask completion <bash|zsh|fish|powershell>");
    output.WriteLine("       dotask --init [--use-dir PATH]");
    output.WriteLine("       dotask --list [SELECTION...] | --save SELECTION... | --add SELECTION...");
    output.WriteLine("       dotask --sync [SELECTION...] | --remove SELECTION...");
    output.WriteLine();
    output.WriteLine("Commands:");
    output.WriteRow("  dotask, dotask help   ", "Show the project's name, description, settings, targets, and options.");
    output.WriteRow("  dotask TARGET        ", "Run a target with its declared options.");
    output.WriteRow("  dotask help TARGET   ", "Show one target's details (also: dotask TARGET --help).");
    output.WriteRow("  dotask completion    ", "Print a completion script for the specified shell.");
    output.WriteRow("  dotask --init        ", "Create project configuration and a task directory here; preserve existing files.");
    output.WriteLine();
    output.WriteLine("Global options:");
    output.WriteRow("  --use-dir PATH  ", "Use this task directory (relative to the invocation directory).");
    output.WriteRow("  --help, -h      ", "Show CLI help, or target help when a target is given.");
    output.WriteRow("  --version       ", "Show version.");
    output.WriteLine();
    output.WriteLine("Shared tasks:");
    output.WriteRow("  --list          ", "List the online cache, or private-tasks/* for private tasks.");
    output.WriteRow("  --save          ", "Download selected shared tasks into the local cache.");
    output.WriteRow("  --add           ", "Copy shared tasks into the project; never overwrite existing tasks.");
    output.WriteRow("  --sync          ", "Update installed shared tasks while protecting local edits.");
    output.WriteRow("  --remove        ", "Remove only tracked shared tasks that match their installed originals.");
    output.WriteRow("  --dry-run       ", "Preview --add, --sync, or --remove without changing project files.");
    output.WriteRow("  --accept-merge  ", "With --sync and explicit task names, record an already reviewed manual merge; keep project file contents.");
    output.WriteLine("Selections: dotnet/build, \"dotnet/{build,run,format}\", \"dotnet/*\", private-tasks/my-group/my-task.");
    output.WriteLine("An omitted source means dotask-official. Commit .dotasks.yaml, .dotasks-lock.yaml, and .tasks/.");
  }

  public static void Initialization( HelpText output )
  {
    output.WriteLine("Usage: dotask --init [--use-dir PATH]");
    output.WriteLine("Initialize the current directory, without searching parent projects.");
    output.WriteLine("Create missing .dotasks.yaml and .tasks/ entries; never overwrite existing files.");
    output.WriteLine("The initial name comes from the directory; description and shared settings start empty.");
    output.WriteLine();
    output.WriteRow("  --use-dir PATH  ", "Create/use a task subdirectory beneath the current directory. Pass this override on subsequent commands too; it is not saved.");
    output.WriteRow("  --help, -h      ", "Show this help without creating files.");
    output.WriteLine();
    output.WriteLine("Initialization is offline and needs no language SDK. It does not add tasks or create a lock file.");
    output.WriteLine("Then edit .dotasks.yaml and write tasks, or select shared tasks with dotask --add GROUP/TASK.");
  }

  public static void Project( HelpText output, ProjectConfiguration config, string rootDirectory, string taskDirectory )
  {
    var directoryName = new DirectoryInfo(rootDirectory).Name;
    output.WriteLine(config.Name ?? (directoryName.Length > 0 ? directoryName : rootDirectory));
    if (config.Description is not null) {
      output.WriteLine(config.Description);
    }
    output.WriteLine("\nSettings:");
    WriteSettings(output, config.Settings, "  ");
    var relativeTasks = Path.GetRelativePath(rootDirectory, taskDirectory).Replace(Path.DirectorySeparatorChar, '/');
    output.WriteRow("  tasks: ", QuoteSetting($"./{(relativeTasks == "." ? "" : relativeTasks)}"));
  }

  private static void WriteSettings( HelpText output, JsonElement settings, string indent )
  {
    foreach (var property in settings.EnumerateObject().OrderBy(p => p.Name, StringComparer.OrdinalIgnoreCase)) {
      if (property.Value.ValueKind == JsonValueKind.Object && property.Value.EnumerateObject().Any()) {
        output.WriteLine($"{indent}{property.Name}:");
        WriteSettings(output, property.Value, indent + "  ");
      } else {
        var value = property.Value.ValueKind == JsonValueKind.String
          ? QuoteSetting(property.Value.GetString()!) : property.Value.GetRawText();
        output.WriteRow($"{indent}{property.Name}: ", value);
      }
    }
  }

  private static string QuoteSetting( string value )
  {
    var escaped = string.Concat(value.Select(EscapeCharacter));
    return value.Length == 0 || value.Any(c => char.IsWhiteSpace(c) || char.IsControl(c))
      ? "'" + escaped.Replace("'", "''") + "'" : escaped;

    static string EscapeCharacter( char c ) => c switch {
      '\n' => "\\n",
      '\r' => "\\r",
      '\t' => "\\t",
      _ when char.IsControl(c) => $"\\u{(int)c:x4}",
      _ => c.ToString()
    };
  }

  public static void Target( HelpText output, TargetDefinition target, ProjectConfiguration config, string? error,
    string displayName, string rootDirectory )
  {
    output.WriteLine("Target:");
    var nameColumn = displayName.PadRight(14);
    var descriptionIndent = output.WriteRow($"  {nameColumn}  ", error ?? target.Description);
    var source = "./" + Path.GetRelativePath(rootDirectory, target.FilePath).Replace(Path.DirectorySeparatorChar, '/');
    output.WriteRow(new string(' ', descriptionIndent) + "Source: ", source.Contains(' ') ? $"'{source}'" : source);
    if (target.Remarks is not null) {
      output.WriteLine();
      output.WriteRow("  ", target.Remarks);
    }
    if (target.Options.Count > 0) {
      Section(output, "Options");
      WriteOptions(output, target.Options.Select(option => new OptionHelp(option,
        config.DefaultsFor(target.Name).GetValueOrDefault(option.Name) ?? option.Default)).ToArray());
    }
    if (target.Requirements.Count > 0) {
      Section(output, "Requires");
      foreach (var requirement in target.Requirements) {
        output.WriteRow($"  - {requirement.Kind}: ", requirement.Value);
      }
    }
    if (target.Capabilities.Count > 0) {
      Section(output, "Capabilities");
      foreach (var capability in target.Capabilities) {
        output.WriteRow("  - ", capability);
      }
    }
    if (target.Examples.Count > 0) {
      Section(output, "Examples");
      foreach (var example in target.Examples) {
        output.WriteRow("  ", example);
      }
    }
  }

  private static void Section( HelpText output, string title )
  {
    output.WriteLine();
    output.WriteLine(title + ":");
  }

  public static void CombinedOptions( HelpText output, IEnumerable<TargetDefinition> targets, ProjectConfiguration config )
  {
    var entries = new List<(OptionDefinition Option, string? Default)>();
    foreach (var target in targets.Where(t => t.Error is null)) {
      try {
        var defaults = config.DefaultsFor(target.Name);
        entries.AddRange(target.Options.Select(option => (option, defaults.GetValueOrDefault(option.Name) ?? option.Default)));
      } catch (TaskException) {
        // The target listing already contains its invalid configuration error.
      }
    }
    var groups = entries.GroupBy(x => new {
      Name = x.Option.Name.ToUpperInvariant(),
      Alias = x.Option.Alias?.ToUpperInvariant(),
      x.Option.Type,
      x.Option.Required,
      x.Option.Completion,
      Choices = string.Join('\0', x.Option.Choices.Select(c => c.ToUpperInvariant()).Order(StringComparer.Ordinal))
    });
    var options = groups.Select(group =>
    {
      var first = group.First();
      var description = group.Where(x => x.Option.Description.Length > 0)
        .GroupBy(x => x.Option.Description).OrderByDescending(descriptions => descriptions.Count())
        .FirstOrDefault()?.Key ?? "";
      var defaults = group.GroupBy(x => x.Option.Choices.Length > 0 ? x.Default?.ToUpperInvariant() : x.Default).ToArray();
      return new OptionHelp(first.Option with { Description = description }, defaults.Length == 1 ? first.Default : null);
    }).ToArray();
    if (options.Length > 0) {
      output.WriteLine("\nTarget options:");
      WriteOptions(output, options);
    }
  }

  private static void WriteOptions( HelpText output, IReadOnlyList<OptionHelp> options )
  {
    if (options.Count == 0) {
      return;
    }
    var width = Math.Min(40, options.Max(option => option.Signature.Length));
    var indent = new string(' ', width + 4);
    foreach (var entry in options) {
      var option = entry.Option;
      var required = option.Required ? " (required)" : "";
      var defaultText = entry.Default is null ? "" : $" (default: {entry.Default})";
      var description = (option.Description + required + defaultText).TrimStart();
      if (entry.Signature.Length > width) {
        output.WriteLine("  " + entry.Signature, 2);
        if (description.Length > 0) {
          output.WriteRow(indent, description);
        }
      } else {
        output.WriteRow($"  {entry.Signature.PadRight(width)}  ", description);
      }
    }
  }

  private sealed record OptionHelp( OptionDefinition Option, string? Default )
  {
    public string Signature => "--" + Option.Name + (Option.Alias is null ? "" : ", -" + Option.Alias)
      + $" <{(Option.Choices.Length > 0 ? string.Join('|', Option.Choices) : Option.Type)}>";
  }
}

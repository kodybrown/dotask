using System.Text.Json;
using DoTask;

/// <summary>Build and run the configured project, optionally passing an input file.</summary>
/// <option name="configuration" alias="c" choices="Debug,Release" default="Debug">Build configuration.</option>
/// <option name="file" alias="f" type="path">Input file passed to the application; relative paths start at the repository root.</option>
/// <option name="args" default="[]">Application arguments as a JSON array of strings, passed before the optional file.</option>
/// <requires tool="dotnet" />
/// <requires setting="project" />
/// <example>dotask run</example>
/// <example>dotask run -c Release</example>
/// <example>dotask run args='["--help"]'</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var projectFile = project.Config.GetPath("project");
    if (!File.Exists(projectFile)) {
      throw new TaskException($"Project is missing: {projectFile}");
    }

    string[]? projectArguments;
    try {
      projectArguments = JsonSerializer.Deserialize<string[]>(project.Parameters.Get<string>("args"));
    } catch (JsonException) {
      throw new TaskException("args must be a JSON array of strings, for example: args='[\"--help\"]'");
    }

    if (projectArguments is null || projectArguments.Any(argument => argument is null)) {
      throw new TaskException("args must be a JSON array of strings without null values.");
    }

    List<string> arguments = [
      "run", "--project", projectFile,
      "--configuration", project.Parameters.Get<string>("configuration"), "--"
    ];
    // Pass application arguments before the optional input file.
    arguments.AddRange(projectArguments);
    if (project.Parameters.Contains("file")) {
      arguments.Add(project.Parameters.GetPath("file"));
    }

    await project.RunAsync("dotnet", arguments);
  }
}

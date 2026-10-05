// dotask: 1
// description: "Build and run the configured project, optionally passing an input file."
// options:
//   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"], "default": "Debug", "description": "Build configuration."}
//   - {"name": "file", "alias": "f", "type": "path", "description": "Input file passed to the application; relative paths start at the repository root."}
//   - {"name": "args", "default": "[]", "description": "Application arguments as a JSON array of strings, passed before the optional file."}
// requires:
//   - {"kind": "tool", "value": "dotnet"}
//   - {"kind": "setting", "value": "project"}
// examples: ["dotask run", "dotask run -c Release", "dotask run args='[\"--help\"]'"]
// end-dotask
using System.Text.Json;
using DoTask;


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

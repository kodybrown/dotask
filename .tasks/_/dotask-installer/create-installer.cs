// dotask: 1
// description: "Package an application payload and YAML with the standalone Rust installer."
// remarks: "Build the application first. Payload paths in YAML are relative to that YAML. No application language toolchain is invoked."
// options:
//   - {"name": "config", "type": "path", "completion": "file", "description": "Installer YAML; otherwise settings.installer.config."}
//   - {"name": "engine", "type": "path", "completion": "file", "description": "Host Rust installer binary; otherwise settings.installer.engine."}
//   - {"name": "output", "type": "path", "completion": "directory", "description": "Package parent directory; otherwise settings.installer.output."}
// examples: ["dotask create-installer --config packaging/installer.yaml --engine tools/installer.exe --output artifacts/installers"]
// end-dotask
using DoTask;
using System.Text.Json;


public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    string PathValue( string option, string setting ) => project.Parameters.Contains(option)
      ? project.Parameters.GetPath(option) : project.Config.GetPath(setting);
    var config = PathValue("config", "installer.config");
    var engine = PathValue("engine", "installer.engine");
    var output = PathValue("output", "installer.output");
    if (!File.Exists(config) || !File.Exists(engine)) {
      throw new TaskException("Installer YAML and engine must both exist. Build or supply the matching Rust installer binary first.");
    }
    var parent = Path.Combine(output, project.OS.ToString().ToLowerInvariant() + "-" + project.Architecture.ToString().ToLowerInvariant());
    var resultFile = Path.Combine(Path.GetTempPath(), "dotask-package-" + Guid.NewGuid().ToString("N") + ".json");
    try {
      // The engine owns names derived from the packaged application's metadata.
      // Read its structured result instead of guessing paths or scraping output.
      await project.RunAsync(engine, ["package", "--config", config, "--output-parent", parent, "--result-file", resultFile]);
      using var result = JsonDocument.Parse(await File.ReadAllTextAsync(resultFile));
      await project.SetInstallerResultAsync(new InstallerArtifact {
        FilePath = result.RootElement.GetProperty("FilePath").GetString()!,
        Kind = InstallerKind.Executable,
        OS = project.OS,
        Architecture = project.Architecture
      });
    } finally {
      if (File.Exists(resultFile)) {
        File.Delete(resultFile);
      }
    }
    Console.WriteLine("Distribute the entire package directory, including installer.yaml and payload.");
  }
}

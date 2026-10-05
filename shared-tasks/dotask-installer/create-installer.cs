// dotask: 1
// description: "Package an application payload and YAML with the standalone Rust installer."
// remarks: "Build the application first. Payload paths in YAML are relative to that YAML. No application language toolchain is invoked."
// options:
//   - {"name": "config", "type": "path", "completion": "file", "description": "Installer YAML; otherwise settings.installer-config."}
//   - {"name": "engine", "type": "path", "completion": "file", "description": "Host Rust installer binary; otherwise settings.installer-engine."}
//   - {"name": "output", "type": "path", "completion": "directory", "description": "Package parent directory; otherwise settings.installer-output."}
// examples: ["dotask create-installer --config packaging/installer.yaml --engine tools/installer.exe --output artifacts/installers"]
// end-dotask
using DoTask;


public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    string PathValue( string option, string setting ) => project.Parameters.Contains(option)
      ? project.Parameters.GetPath(option) : project.Config.GetPath(setting);
    var config = PathValue("config", "installer-config");
    var engine = PathValue("engine", "installer-engine");
    var output = PathValue("output", "installer-output");
    if (!File.Exists(config) || !File.Exists(engine)) {
      throw new TaskException("Installer YAML and engine must both exist. Build or supply the matching Rust installer binary first.");
    }
    var package = Path.Combine(output, project.OS.ToString().ToLowerInvariant() + "-" + project.Architecture.ToString().ToLowerInvariant(), Guid.NewGuid().ToString("N"));
    await project.RunAsync(engine, ["package", "--config", config, "--output", package]);
    await project.SetInstallerResultAsync(new InstallerArtifact {
      FilePath = Path.Combine(package, project.IsWindows ? "installer.exe" : "installer"),
      Kind = InstallerKind.Executable,
      OS = project.OS,
      Architecture = project.Architecture
    });
    Console.WriteLine("Distribute the entire package directory, including installer.yaml and payload.");
  }
}

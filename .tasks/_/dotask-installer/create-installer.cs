using DoTask;

/// <summary>Package an application payload and YAML with the standalone Rust installer.</summary>
/// <option name="config" type="path" completion="file">Installer YAML; otherwise settings.installer-config.</option>
/// <option name="engine" type="path" completion="file">Host Rust installer binary; otherwise settings.installer-engine.</option>
/// <option name="output" type="path" completion="directory">Package parent directory; otherwise settings.installer-output.</option>
/// <remarks>Build the application first. Payload paths in YAML are relative to that YAML. No application language toolchain is invoked.</remarks>
/// <example>dotask create-installer --config packaging/installer.yaml --engine tools/installer.exe --output artifacts/installers</example>
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

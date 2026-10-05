// dotask: 1
// description: "Create and run the project's installer for this OS and architecture."
// remarks: "Invokes create-installer through the project catalog. Custom creators must return one InstallerArtifact and must not install."
// options:
//   - {"name": "installer-args", "type": "string", "description": "JSON array of installer argument tokens; replaces the installer's defaults."}
// examples: ["dotask install", "dotask install --installer-args '[\"--interactive\"]'"]
// end-dotask
using DoTask;


public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var arguments = project.Parameters.Contains("installer-args")
      ? InstallerRunner.ParseArguments(project.Parameters.Get<string>("installer-args")) : null;
    var installer = await project.CreateInstallerAsync();
    Console.WriteLine($"Running installer: {installer.FilePath}");
    var result = await project.RunInstallerAsync(installer, arguments);
    Console.WriteLine($"Installer exited with code {result.ExitCode}.");
  }
}

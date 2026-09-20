using DoTask;

/// <summary>Create and run the project's installer for this OS and architecture.</summary>
/// <option name="installer-args" type="string">JSON array of installer argument tokens; replaces the installer's defaults.</option>
/// <remarks>Requires the project's create-installer target. That target must build, not run, its installer and return it using SetInstallerResultAsync.</remarks>
/// <example>dotask install</example>
/// <example>dotask install --installer-args '["--scope","user"]'</example>
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

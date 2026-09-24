using DoTask;

/// <summary>Run the retained installer to uninstall an application without rebuilding it.</summary>
/// <option name="install-dir" type="path" required="true" completion="directory">Application installation root.</option>
/// <option name="interactive" type="bool" default="false">Enable confirmation and settings prompts.</option>
/// <option name="remove-settings" type="bool" default="false">Remove declared application settings; otherwise preserve them.</option>
/// <example>dotask uninstall --install-dir C:/Temp/example</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var root = project.Parameters.GetPath("install-dir");
    var executable = Path.Combine(root, "installer", project.IsWindows ? "installer.exe" : "installer");
    List<string> arguments = ["uninstall", "--install-dir", root];
    if (project.Parameters.Get<bool>("interactive"))
      arguments.Add("--interactive");
    if (project.Parameters.Get<bool>("remove-settings"))
      arguments.Add("--remove-settings");
    await project.RunInstallerAsync(new InstallerArtifact {
      FilePath = executable,
      Kind = InstallerKind.Executable,
      OS = project.OS,
      Architecture = project.Architecture
    }, arguments);
  }
}

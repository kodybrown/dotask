// dotask: 1
// description: "Run the retained installer to uninstall an application without rebuilding it."
// options:
//   - {"name": "install-dir", "type": "path", "required": true, "completion": "directory", "description": "Application installation root."}
//   - {"name": "non-interactive", "type": "bool", "default": "false", "description": "Disable prompts for unattended uninstall; otherwise use installer.yaml."}
//   - {"name": "remove-settings", "type": "bool", "default": "false", "description": "Remove declared application settings; otherwise preserve them."}
// examples: ["dotask uninstall --install-dir C:/Temp/example"]
// end-dotask
using DoTask;


public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var root = project.Parameters.GetPath("install-dir");
    var executable = Path.Combine(root, "installer", project.IsWindows ? "installer.exe" : "installer");
    List<string> arguments = ["uninstall", "--install-dir", root];
    if (project.Parameters.Get<bool>("non-interactive"))
      arguments.Add("--non-interactive");
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

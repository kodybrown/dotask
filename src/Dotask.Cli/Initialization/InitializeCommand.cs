using System.Text;
using System.Text.Json;
using DoTask.Cli.Configuration;
using DoTask.Cli.Parsing;

namespace DoTask.Cli.Initialization;

internal static class InitializeCommand
{
  public static int Run( CommandLine command, string invocationDirectory, TextWriter output, CancellationToken token )
  {
    if (command.Arguments.Length != 0) {
      throw new TaskException("Usage: dotask --init [--use-dir PATH]. Run it in the project directory; existing files are never overwritten.");
    }
    token.ThrowIfCancellationRequested();
    var root = Path.GetFullPath(invocationDirectory);
    var tasks = PortablePath.Resolve(root, command.UseDirectory ?? ".tasks");
    var relative = Path.GetRelativePath(root, tasks);
    if (relative == "." || relative == ".." || relative.StartsWith(".." + Path.DirectorySeparatorChar, StringComparison.Ordinal)
      || Path.IsPathRooted(relative)) {
      throw new TaskException("--init requires the task directory to be a subdirectory of the current project directory.");
    }
    var firstPart = relative.Split(Path.DirectorySeparatorChar)[0];
    if (firstPart.Equals(".dotasks.yaml", StringComparison.OrdinalIgnoreCase)
      || firstPart.Equals(".dotasks-lock.yaml", StringComparison.OrdinalIgnoreCase)) {
      throw new TaskException("The task directory cannot overlap .dotasks.yaml or .dotasks-lock.yaml.");
    }
    CheckEntry(root, directory: true);
    if (!Directory.Exists(root)) {
      throw new TaskException($"Project directory does not exist: {root}");
    }
    var configuration = Path.Combine(root, ".dotasks.yaml");
    CheckEntry(configuration, directory: false);
    var current = root;
    foreach (var part in relative.Split(Path.DirectorySeparatorChar)) {
      current = Path.Combine(current, part);
      CheckEntry(current, directory: true);
    }
    var legacy = Path.Combine(tasks, "config.yaml");
    if (File.Exists(legacy) || Directory.Exists(legacy) || new FileInfo(legacy).LinkTarget is not null) {
      throw new TaskException($"Legacy configuration exists at '{legacy}'. Review and move it to '{configuration}' before using --init; no files were changed.");
    }
    // Validate any existing configuration before creating missing pieces, without rewriting it.
    ProjectConfiguration.Load(tasks, root);
    token.ThrowIfCancellationRequested();
    var name = new DirectoryInfo(root).Name;
    var content = $"version: 1\nname: {JsonSerializer.Serialize(name.Length == 0 ? root : name)}\ndescription: ''\nsettings: {{}}\n";
    var createdConfig = CreateConfiguration(configuration, content);
    var existingTasks = Directory.Exists(tasks);
    Directory.CreateDirectory(tasks);
    CheckEntry(tasks, directory: true);
    output.WriteLine(createdConfig ? "Created .dotasks.yaml." : "Kept existing .dotasks.yaml unchanged.");
    var taskPath = "./" + relative.Replace(Path.DirectorySeparatorChar, '/') + "/";
    output.WriteLine(existingTasks ? $"Kept existing task directory: {taskPath}" : $"Created task directory: {taskPath}");
    output.WriteLine("Project initialized. Edit .dotasks.yaml to set the project description and shared settings.");
    output.WriteLine($"Create your own tasks in {taskPath}, or add shared tasks with dotask --add GROUP/TASK.");
    if (command.UseDirectory is not null) {
      output.WriteLine($"Continue passing --use-dir {JsonSerializer.Serialize(relative.Replace(Path.DirectorySeparatorChar, '/'))} to dotask commands; this override is not saved in the configuration.");
    }
    output.WriteLine("Commit .dotasks.yaml and your task files. Adding shared tasks will create .dotasks-lock.yaml.");
    return 0;
  }

  private static void CheckEntry( string path, bool directory )
  {
    if (new FileInfo(path).LinkTarget is not null || new DirectoryInfo(path).LinkTarget is not null) {
      throw new TaskException($"--init does not follow symbolic links or reparse points: {path}");
    }
    if (!File.Exists(path) && !Directory.Exists(path)) {
      return;
    }
    var attributes = File.GetAttributes(path);
    if ((attributes & FileAttributes.ReparsePoint) != 0) {
      throw new TaskException($"--init does not follow symbolic links or reparse points: {path}");
    }
    if ((attributes & FileAttributes.Directory) != 0 != directory) {
      throw new TaskException($"Expected a {(directory ? "directory" : "file")} at '{path}'; the existing entry was left unchanged.");
    }
  }

  private static bool CreateConfiguration( string path, string content )
  {
    if (File.Exists(path)) {
      return false;
    }
    var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
    try {
      using (var file = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None)) {
        file.Write(Encoding.UTF8.GetBytes(content));
        file.Flush(flushToDisk: true);
      }
      // A concurrent initializer or editor may win. Never replace their file.
      try {
        File.Move(temporary, path, overwrite: false);
        return true;
      } catch (IOException) when (File.Exists(path)) {
        CheckEntry(path, directory: false);
        return false;
      }
    } finally {
      File.Delete(temporary);
    }
  }
}

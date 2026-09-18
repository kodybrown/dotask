using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text.Json;

namespace DoTask;

/// <summary>Installs already-published applications for the current user without modifying PATH.</summary>
public static class UserInstaller
{
  public static async Task<InstallationResult> InstallAsync(InstallationDefinition definition, CancellationToken cancellationToken = default)
  {
    ArgumentNullException.ThrowIfNull(definition);
    if (!OperatingSystem.IsWindows() && !OperatingSystem.IsLinux() && !OperatingSystem.IsMacOS())
    {
      throw new TaskException("User installation supports Windows, Linux, and macOS.");
    }

    InstallationFiles.Name(definition.AppId, "application ID");
    InstallationFiles.Name(definition.Version, "version", version: true);
    if (definition.Commands is null || definition.Commands.Count == 0)
    {
      throw new TaskException("At least one installed command is required.");
    }

    var names = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
    foreach (var command in definition.Commands)
    {
      InstallationFiles.Name(command.Name, "command name");
      InstallationFiles.Relative(command.Executable);
      if (!names.Add(command.Name) || command.Name.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
      {
        throw new TaskException("Installed command names must be unique and omit the .exe extension.");
      }

      if (OperatingSystem.IsWindows() && !command.Executable.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
      {
        throw new TaskException("Windows command entry points must be .exe files.");
      }
    }
    var source = InstallationFiles.PhysicalDirectory(definition.SourceDirectory);
    if (!Directory.Exists(source))
    {
      throw new TaskException($"Published directory does not exist: {source}");
    }

    var root = InstallationFiles.PhysicalDirectory(definition.InstallRoot ?? DefaultInstallRoot());
    var bin = InstallationFiles.PhysicalDirectory(ResolveBinDirectory(definition.BinDirectory, Environment.GetEnvironmentVariable("BIN")));
    var app = Path.Combine(root, definition.AppId);
    if (Overlaps(source, root) || Overlaps(source, bin) || Overlaps(app, bin))
    {
      throw new TaskException("Published files, application installations, and command directories must not overlap.");
    }

    var files = await InstallationFiles.InventoryAsync(source, installed: false, cancellationToken);
    if (files.Length == 0)
    {
      throw new TaskException("Published directory is empty.");
    }

    foreach (var command in definition.Commands)
    {
      var file = files.SingleOrDefault(f => f.Path == command.Executable)
        ?? throw new TaskException($"Published command does not exist: {command.Executable}");
      if (!OperatingSystem.IsWindows() && (file.Mode & (int)(UnixFileMode.UserExecute | UnixFileMode.GroupExecute | UnixFileMode.OtherExecute)) == 0)
      {
        throw new TaskException($"Published command is not executable: {command.Executable}");
      }
    }
    var fingerprint = Convert.ToHexStringLower(SHA256.HashData(JsonSerializer.SerializeToUtf8Bytes(new
    {
      Platform = OperatingSystem.IsWindows() ? "windows" : OperatingSystem.IsLinux() ? "linux" : "macos",
      Architecture = RuntimeInformation.OSArchitecture.ToString(),
      Files = files
    })));
    var directory = Path.Combine(app, definition.Version + "-" + fingerprint[..16]);
    var desired = InstallationCommands.Desired(directory, definition.Commands);
    cancellationToken.ThrowIfCancellationRequested();
    using var rootLock = InstallationFiles.Lock(root);
    using var binLock = root.Equals(bin, InstallationFiles.PathComparison) ? null : InstallationFiles.Lock(bin);
    InstallationFiles.NotLink(app);
    var receiptPath = Path.Combine(app, ".dotask-install.json");
    InstallReceipt receipt;
    if (Directory.Exists(app))
    {
      if (!File.Exists(receiptPath))
      {
        throw new TaskException($"Application directory already exists without dotask ownership: {app}");
      }

      receipt = InstallationFiles.Read<InstallReceipt>(receiptPath);
      InstallationCommands.ValidateReceipt(receipt, definition.AppId, bin);
      InstallationCommands.Recover(app, definition.AppId, bin);
      receipt = InstallationFiles.Read<InstallReceipt>(receiptPath);
    }
    else
    {
      if (InstallationFiles.Exists(app))
      {
        throw new TaskException($"Application path is occupied: {app}");
      }

      receipt = new(1, definition.AppId, bin, null, new(StringComparer.OrdinalIgnoreCase));
      // Check commands before claiming an installation directory.
      InstallationCommands.CheckConflicts(receipt, desired, definition.Commands);
      Directory.CreateDirectory(app);
      InstallationFiles.Write(receiptPath, receipt);
    }
    InstallationCommands.CheckConflicts(receipt, desired, definition.Commands);
    var reused = Directory.Exists(directory);
    var build = new InstalledBuild(definition.AppId, definition.Version, fingerprint, files);
    if (InstallationFiles.Exists(directory))
    {
      InstallationFiles.NotLink(directory);
      var record = Path.Combine(directory, InstallationFiles.BuildRecord);
      if (!File.Exists(record))
      {
        throw new TaskException($"Build directory has no ownership record: {directory}");
      }

      var existing = InstallationFiles.Read<InstalledBuild>(record);
      if (existing.AppId != build.AppId || existing.Version != build.Version || existing.Fingerprint != fingerprint ||
          existing.Files is null || !existing.Files.SequenceEqual(files) || !(await InstallationFiles.InventoryAsync(directory, installed: true, cancellationToken)).SequenceEqual(files))
      {
        throw new TaskException($"Installed build was modified; refusing to overwrite it: {directory}");
      }
    }
    else
    {
      var staging = Path.Combine(app, ".staging-" + Guid.NewGuid().ToString("N"));
      Directory.CreateDirectory(staging);
      try
      {
        foreach (var file in files)
        {
          cancellationToken.ThrowIfCancellationRequested();
          var destination = Path.Combine(staging, file.Path);
          Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
          await using (var input = File.OpenRead(Path.Combine(source, file.Path)))
          await using (var output = new FileStream(destination, FileMode.CreateNew, FileAccess.Write, FileShare.None))
          {
            await input.CopyToAsync(output, cancellationToken);
          }

          if (!OperatingSystem.IsWindows())
          {
            File.SetUnixFileMode(destination, (UnixFileMode)file.Mode);
          }
        }
        if (!(await InstallationFiles.InventoryAsync(staging, installed: false, cancellationToken)).SequenceEqual(files))
        {
          throw new TaskException("Published files changed while copying. Publish again and retry installation.");
        }

        InstallationFiles.Write(Path.Combine(staging, InstallationFiles.BuildRecord), build);
        Directory.Move(staging, directory);
      }
      finally
      {
        if (Directory.Exists(staging))
        {
          Directory.Delete(staging, recursive: true);
        }
      }
    }
    cancellationToken.ThrowIfCancellationRequested();
    InstallationCommands.CheckConflicts(receipt, desired, definition.Commands);
    InstallationCommands.Activate(app, receipt, receipt with { ActiveDirectory = Path.GetFileName(directory), Commands = desired });
    return new(directory, bin, fingerprint, reused, PathWarnings(bin, definition.Commands));
  }

  internal static string DefaultInstallRoot() => OperatingSystem.IsWindows()
    ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Programs")
    : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".local", "lib");

  internal static string ResolveBinDirectory(string? requested, string? environmentBin) => requested ??
    (string.IsNullOrWhiteSpace(environmentBin) ? null : environmentBin) ?? (OperatingSystem.IsWindows()
      ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "bin")
      : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".local", "bin"));

  private static bool Overlaps(string first, string second) => first.Equals(second, InstallationFiles.PathComparison) ||
    first.StartsWith(second + Path.DirectorySeparatorChar, InstallationFiles.PathComparison) ||
    second.StartsWith(first + Path.DirectorySeparatorChar, InstallationFiles.PathComparison);

  private static IReadOnlyList<string> PathWarnings(string bin, IReadOnlyList<InstalledCommand> commands)
  {
    var warnings = new List<string>();
    var directories = new List<string>();
    foreach (var entry in (Environment.GetEnvironmentVariable("PATH") ?? "").Split(Path.PathSeparator))
    {
      if (string.IsNullOrWhiteSpace(entry))
      {
        continue;
      }

      try { directories.Add(InstallationFiles.PhysicalDirectory(entry.Trim('"'))); }
      catch (Exception ex) when (ex is ArgumentException or IOException or TaskException or UnauthorizedAccessException) { }
    }
    if (!directories.Contains(bin, OperatingSystem.IsWindows() ? StringComparer.OrdinalIgnoreCase : StringComparer.Ordinal))
    {
      warnings.Add($"Add the command directory to your user PATH once: {bin}. dotask did not change PATH or shell profiles.");
    }

    foreach (var command in commands)
    {
      var candidates = directories.Append(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".dotnet", "tools"));
      foreach (var candidate in candidates.Distinct())
      {
        if (candidate.Equals(bin, InstallationFiles.PathComparison))
        {
          continue;
        }

        var path = Path.Combine(candidate, command.Name + (OperatingSystem.IsWindows() ? ".exe" : ""));
        if (InstallationFiles.Exists(path))
        {
          warnings.Add($"Another '{command.Name}' command exists at {path}. Check command resolution and explicitly remove an old global-tool installation if migrating; it was not changed.");
        }
      }
    }
    return warnings;
  }
}

using System.Runtime.InteropServices;
using System.Text;

namespace DoTask;

internal sealed record CommandFile( string Kind, string Value )
{
  internal static CommandFile? Read( string path )
  {
    var info = new FileInfo(path);
    if (info.LinkTarget is { } link) {
      return new("link", link);
    }

    if (Directory.Exists(path)) {
      throw new TaskException($"Command destination is a directory: {path}");
    }

    if (!File.Exists(path)) {
      return null;
    }

    if (info.Length > 1024 * 1024) {
      throw new TaskException($"Command destination is not a dotask launcher: {path}");
    }

    return new("file", Convert.ToBase64String(File.ReadAllBytes(path)));
  }

  internal static void Apply( string path, CommandFile? value )
  {
    if (value is null) { File.Delete(path); return; }
    var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
    try {
      if (value.Kind == "link") {
        File.CreateSymbolicLink(temporary, value.Value);
      } else if (value.Kind == "file") {
        File.WriteAllBytes(temporary, Convert.FromBase64String(value.Value));
      } else {
        throw new TaskException("Unknown command record kind.");
      }

      File.Move(temporary, path, overwrite: true);
    } finally { File.Delete(temporary); }
  }
}

internal sealed record InstallReceipt( int Schema, string AppId, string BinDirectory,
  string? ActiveDirectory, Dictionary<string, CommandFile> Commands );
internal sealed record CommandChange( string Name, CommandFile? Before, CommandFile? After );
internal sealed record InstallJournal( InstallReceipt Before, InstallReceipt After, CommandChange[] Changes );

internal static class InstallationCommands
{
  internal static byte[] Shim()
  {
    var architecture = RuntimeInformation.OSArchitecture switch {
      Architecture.X64 => "x64",
      Architecture.Arm64 => "arm64",
      _ => throw new TaskException("Windows installation supports x64 and ARM64 hosts.")
    };
    using var stream = typeof(BuildContext).Assembly.GetManifestResourceStream($"DoTask.Shim.win-{architecture}.exe")
      ?? throw new TaskException("This dotask distribution does not contain the Windows shim.");
    using var buffer = new MemoryStream();
    stream.CopyTo(buffer);
    return buffer.ToArray();
  }

  internal static Dictionary<string, CommandFile> Desired( string directory, IReadOnlyList<InstalledCommand> commands )
  {
    var result = new Dictionary<string, CommandFile>(StringComparer.OrdinalIgnoreCase);
    var shim = OperatingSystem.IsWindows() ? Convert.ToBase64String(Shim()) : null;
    foreach (var command in commands) {
      var target = Path.Combine(directory, command.Executable.Replace('/', Path.DirectorySeparatorChar));
      if (shim is not null) {
        result.Add(command.Name + ".exe", new("file", shim));
        result.Add(command.Name + ".shim", new("file", Convert.ToBase64String(Encoding.UTF8.GetBytes($"path = \"{target}\"\n"))));
      } else {
        result.Add(command.Name, new("link", target));
      }
    }
    return result;
  }

  internal static void ValidateReceipt( InstallReceipt receipt, string appId, string bin )
  {
    if (receipt is null || receipt.Schema != 1 || receipt.AppId != appId ||
        !string.Equals(receipt.BinDirectory, bin, InstallationFiles.PathComparison) || receipt.Commands is null) {
      throw new TaskException("Installation ownership or command directory does not match. Preserve the installation records; use its original --bin-dir.");
    }

    foreach (var entry in receipt.Commands) {
      InstallationFiles.Name(entry.Key, "recorded command file");
      if (entry.Value is null || entry.Value.Kind is not ("file" or "link")) {
        throw new TaskException("Invalid command ownership record.");
      }
    }
  }

  internal static void VerifyOwned( InstallReceipt receipt )
  {
    foreach (var entry in receipt.Commands) {
      if (CommandFile.Read(Path.Combine(receipt.BinDirectory, entry.Key)) != entry.Value) {
        throw new TaskException($"Installed command was changed or removed; refusing to overwrite it: {Path.Combine(receipt.BinDirectory, entry.Key)}");
      }
    }
  }

  internal static void CheckConflicts( InstallReceipt receipt, Dictionary<string, CommandFile> desired,
    IReadOnlyList<InstalledCommand> commands )
  {
    VerifyOwned(receipt);
    foreach (var name in desired.Keys) {
      if (receipt.Commands.Keys.Any(old => old != name && old.Equals(name, StringComparison.OrdinalIgnoreCase))) {
        throw new TaskException($"Changing only the casing of an installed command is not supported: {name}");
      }

      if (!receipt.Commands.ContainsKey(name) && InstallationFiles.Exists(Path.Combine(receipt.BinDirectory, name))) {
        throw new TaskException($"Command destination already exists and is not owned by this installation: {Path.Combine(receipt.BinDirectory, name)}");
      }
    }
    if (OperatingSystem.IsWindows()) {
      foreach (var command in commands) {
        foreach (var extension in new[] { "", ".cmd", ".bat", ".com", ".ps1" }) {
          var name = command.Name + extension;
          if (!receipt.Commands.ContainsKey(name) && InstallationFiles.Exists(Path.Combine(receipt.BinDirectory, name))) {
            throw new TaskException($"Another command occupies this name: {Path.Combine(receipt.BinDirectory, name)}");
          }
        }
      }
    }
  }

  // A durable journal makes an interrupted activation recoverable on the next
  // install. Recovery checks every entry before touching any command.
  internal static void Recover( string app, string appId, string bin )
  {
    var path = Path.Combine(app, ".dotask-pending.json");
    if (!InstallationFiles.Exists(path)) {
      return;
    }

    var journal = InstallationFiles.Read<InstallJournal>(path);
    ValidateReceipt(journal.Before, appId, bin);
    ValidateReceipt(journal.After, appId, bin);
    var expected = Changes(journal.Before, journal.After);
    if (journal.Changes is null || !expected.SequenceEqual(journal.Changes)) {
      throw new TaskException($"Invalid installation recovery journal: {path}");
    }

    var receipt = InstallationFiles.Read<InstallReceipt>(Path.Combine(app, ".dotask-install.json"));
    if (JsonIdentity(receipt) != JsonIdentity(journal.Before) && JsonIdentity(receipt) != JsonIdentity(journal.After)) {
      throw new TaskException($"Installation ownership changed during an interrupted activation. Preserve {path} before resolving it.");
    }

    foreach (var change in expected) {
      var current = CommandFile.Read(Path.Combine(bin, change.Name));
      if (current != change.Before && current != change.After) {
        throw new TaskException($"Interrupted installation has a modified command. Preserve {path} and resolve {change.Name} before retrying.");
      }
    }
    foreach (var change in expected.Reverse()) {
      var destination = Path.Combine(bin, change.Name);
      if (CommandFile.Read(destination) != change.Before) {
        CommandFile.Apply(destination, change.Before);
      }
    }
    InstallationFiles.Write(Path.Combine(app, ".dotask-install.json"), journal.Before);
    File.Delete(path);
  }

  private static string JsonIdentity( InstallReceipt receipt ) => System.Text.Json.JsonSerializer.Serialize(receipt);

  internal static CommandChange[] Changes( InstallReceipt before, InstallReceipt after )
    => before.Commands.Keys.Union(after.Commands.Keys, StringComparer.OrdinalIgnoreCase).Order(StringComparer.Ordinal)
      .Select(name => new CommandChange(name, before.Commands.GetValueOrDefault(name), after.Commands.GetValueOrDefault(name)))
      .Where(c => c.Before != c.After).ToArray();

  internal static void Activate( string app, InstallReceipt before, InstallReceipt after )
  {
    var changes = Changes(before, after);
    var pending = Path.Combine(app, ".dotask-pending.json");
    InstallationFiles.Write(pending, new InstallJournal(before, after, changes));
    try {
      foreach (var change in changes) {
        var path = Path.Combine(before.BinDirectory, change.Name);
        if (CommandFile.Read(path) != change.Before) {
          throw new TaskException($"Command changed during installation: {path}");
        }

        CommandFile.Apply(path, change.After);
      }
      InstallationFiles.Write(Path.Combine(app, ".dotask-install.json"), after);
      File.Delete(pending);
    } catch {
      Recover(app, before.AppId, before.BinDirectory);
      throw;
    }
  }
}

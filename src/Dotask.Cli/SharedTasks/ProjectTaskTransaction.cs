using System.Text.Json;
using DoTask.Cli.Discovery;

namespace DoTask.Cli.SharedTasks;

internal sealed record TaskFileChange( string Path, string? BeforeHash, byte[]? After );

// A durable rollback journal keeps task files and their tracking data together.
// An interrupted operation is recovered before another operation can proceed.
internal sealed class ProjectTaskTransaction( TaskDirectory project )
{
  private string TaskPrefix => Path.GetRelativePath(project.RootDirectory, project.DirectoryPath).Replace(Path.DirectorySeparatorChar, '/');
  private string JournalDirectory => SharedTaskFiles.Resolve(project.RootDirectory, TaskPrefix + "/.dotask/transaction");
  private string JournalPath => Path.Combine(JournalDirectory, "journal.json");
  private const string Owner = "dotask shared-task transaction state v1\n";

  private void EnsureStateDirectory()
  {
    var state = SharedTaskFiles.Resolve(project.RootDirectory, TaskPrefix + "/.dotask");
    var marker = SharedTaskFiles.Resolve(state, "owner");
    if (Directory.Exists(state)) {
      if (!File.Exists(marker) || File.ReadAllText(marker) != Owner) {
        throw new TaskException($"Refusing to use an unrecognized internal state directory: {state}. No existing files will be deleted.");
      }
      return;
    }
    Directory.CreateDirectory(state);
    using var stream = new FileStream(marker, FileMode.CreateNew, FileAccess.Write, FileShare.None);
    stream.Write(System.Text.Encoding.UTF8.GetBytes(Owner));
    stream.Flush(flushToDisk: true);
  }

  public static async Task<FileStream> AcquireAsync( TaskDirectory project, SharedTaskOptions options, CancellationToken token )
  {
    DoTask.Cli.Execution.TargetCompiler.CreatePrivateDirectory(options.CacheDirectory);
    var identity = Path.GetFullPath(project.RootDirectory);
    var name = SharedTaskFiles.Hash(OperatingSystem.IsLinux() ? identity : identity.ToUpperInvariant()) + ".lock";
    var path = SharedTaskFiles.Resolve(options.CacheDirectory, ".operations/" + name);
    Directory.CreateDirectory(Path.GetDirectoryName(path)!);
    for (var attempt = 0; ; attempt++) {
      token.ThrowIfCancellationRequested();
      try {
        return new FileStream(path, FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
      } catch (IOException) when (attempt < 100) {
        await Task.Delay(100, token);
      }
    }
  }

  public bool HasPending => File.Exists(JournalPath);

  public void Recover()
  {
    if (!HasPending) {
      return;
    }
    EnsureStateDirectory();
    var journal = JsonSerializer.Deserialize<Journal>(File.ReadAllBytes(JournalPath), SharedTaskJson.Options)
      ?? throw new TaskException("Invalid interrupted-operation journal. Project files have been left untouched.");
    Validate(journal);
    var applied = new List<(JournalEntry Entry, byte[]? Before)>();
    foreach (var entry in journal.Entries) {
      var path = Destination(entry.Path);
      var current = SharedTaskFiles.HashFile(path);
      if (Directory.Exists(path) || (current != entry.BeforeHash && current != entry.AfterHash)) {
        throw new TaskException($"Interrupted shared-task operation needs manual recovery: '{entry.Path}' changed afterward. Preserve the originals in {JournalDirectory}; no files have been overwritten.");
      }
      byte[]? before = null;
      if (entry.BeforeHash is not null) {
        before = File.ReadAllBytes(SharedTaskFiles.Resolve(JournalDirectory, entry.Backup));
        if (SharedTaskFiles.Hash(before) != entry.BeforeHash) {
          throw new TaskException("Interrupted-operation backup failed verification. Project files have been left untouched.");
        }
      }
      if (current != entry.BeforeHash) {
        applied.Add((entry, before));
      }
    }
    foreach (var (entry, before) in applied.AsEnumerable().Reverse()) {
      var path = Destination(entry.Path);
      if (SharedTaskFiles.HashFile(path) != entry.AfterHash) {
        throw new TaskException($"File changed during recovery: {entry.Path}. Recovery data was retained.");
      }
      if (before is null) {
        File.Delete(path);
      } else {
        SharedTaskFiles.AtomicWrite(path, before);
      }
    }
    File.Delete(JournalPath);
    CleanupIdleState();
  }

  public void Apply( IReadOnlyList<TaskFileChange> changes, CancellationToken token, Action<int>? afterWrite = null )
  {
    if (changes.Count == 0) {
      CleanupIdleState();
      return;
    }
    if (HasPending) {
      throw new TaskException("An interrupted shared-task operation must be recovered first.");
    }
    foreach (var change in changes) {
      VerifyBefore(change);
    }
    var entries = changes.Select(( change, index ) => new JournalEntry(change.Path, change.BeforeHash,
      change.After is null ? null : SharedTaskFiles.Hash(change.After), index + ".original")).ToArray();
    var journal = new Journal(1, entries);
    Validate(journal);
    CleanupIdleState();
    EnsureStateDirectory();
    if (Directory.Exists(JournalDirectory)) {
      throw new TaskException($"Unrecognized files remain in {JournalDirectory}; they were preserved. Review them before retrying.");
    }
    Directory.CreateDirectory(JournalDirectory);
    var ignore = SharedTaskFiles.Resolve(project.RootDirectory, TaskPrefix + "/.dotask/.gitignore");
    if (!File.Exists(ignore)) {
      using var stream = new FileStream(ignore, FileMode.CreateNew, FileAccess.Write, FileShare.None);
      stream.Write("*\n"u8);
    }
    for (var index = 0; index < changes.Count; index++) {
      var change = changes[index];
      if (change.BeforeHash is not null) {
        var before = File.ReadAllBytes(Destination(change.Path));
        if (SharedTaskFiles.Hash(before) != change.BeforeHash) {
          throw new TaskException($"File changed while preparing the update: {change.Path}");
        }
        SharedTaskFiles.AtomicWrite(Path.Combine(JournalDirectory, entries[index].Backup), before);
      }
    }
    SharedTaskFiles.AtomicWrite(JournalPath, JsonSerializer.SerializeToUtf8Bytes(journal, SharedTaskJson.Options));
    try {
      for (var index = 0; index < changes.Count; index++) {
        token.ThrowIfCancellationRequested();
        var change = changes[index];
        VerifyBefore(change);
        var path = Destination(change.Path);
        if (change.After is null) {
          File.Delete(path);
        } else if (change.BeforeHash is null) {
          // A destination created since preflight must never be replaced.
          Directory.CreateDirectory(Path.GetDirectoryName(path)!);
          var temporary = Path.Combine(JournalDirectory, index + ".new");
          SharedTaskFiles.AtomicWrite(temporary, change.After);
          File.Move(temporary, path, overwrite: false);
        } else {
          SharedTaskFiles.AtomicWrite(path, change.After);
        }
        afterWrite?.Invoke(index);
      }
      // Removing the journal is the commit point. Backups are disposable afterward.
      File.Delete(JournalPath);
    } catch {
      Recover();
      throw;
    }
    CleanupIdleState();
  }

  private void CleanupIdleState()
  {
    if (HasPending) {
      return;
    }
    var state = SharedTaskFiles.Resolve(project.RootDirectory, TaskPrefix + "/.dotask");
    if (!Directory.Exists(state)) {
      return;
    }
    var marker = SharedTaskFiles.Resolve(state, "owner");
    if (!File.Exists(marker) || File.ReadAllText(marker) != Owner) {
      return;
    }
    if (Directory.Exists(JournalDirectory)) {
      // Without a journal, recognized staging files are no longer needed for recovery.
      foreach (var file in new DirectoryInfo(JournalDirectory).EnumerateFileSystemInfos()) {
        if (file is FileInfo && (file.Attributes & FileAttributes.ReparsePoint) == 0
          && int.TryParse(Path.GetFileNameWithoutExtension(file.Name), out var index) && index >= 0
          && Path.GetExtension(file.Name) is ".original" or ".new") {
          File.Delete(file.FullName);
        }
      }
      if (Directory.EnumerateFileSystemEntries(JournalDirectory).Any()) {
        return;
      }
      Directory.Delete(JournalDirectory);
    }
    var ignore = SharedTaskFiles.Resolve(state, ".gitignore");
    if (Directory.EnumerateFileSystemEntries(state).Any(path => path != marker && path != ignore)
      || Directory.Exists(ignore) || (File.Exists(ignore) && File.ReadAllText(ignore) != "*\n")) {
      return;
    }
    File.Delete(ignore);
    File.Delete(marker);
    Directory.Delete(state);
  }

  private void VerifyBefore( TaskFileChange change )
  {
    var path = Destination(change.Path);
    if (Directory.Exists(path) || SharedTaskFiles.HashFile(path) != change.BeforeHash) {
      throw new TaskException($"Refusing to change '{change.Path}': it changed after the operation was planned.");
    }
  }

  private string Destination( string relative )
  {
    SharedTaskFiles.ValidateRelativePath(relative);
    if (relative is not (".dotasks.yaml" or ".dotasks-lock.yaml")
      && (!relative.StartsWith(TaskPrefix + "/", StringComparison.Ordinal) || relative.StartsWith(TaskPrefix + "/.dotask/", StringComparison.Ordinal))) {
      throw new TaskException($"Invalid shared-task transaction destination '{relative}'.");
    }
    return SharedTaskFiles.Resolve(project.RootDirectory, relative);
  }

  private void Validate( Journal journal )
  {
    if (journal.Version != 1 || journal.Entries is null || journal.Entries.Length == 0) {
      throw new TaskException("Invalid shared-task transaction journal.");
    }
    var paths = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
    foreach (var entry in journal.Entries) {
      if (entry is null) {
        throw new TaskException("Invalid null shared-task recovery entry.");
      }
      _ = Destination(entry.Path);
      if (!paths.Add(entry.Path) || (entry.BeforeHash is not null && !SharedTaskFiles.IsHash(entry.BeforeHash))
        || (entry.AfterHash is not null && !SharedTaskFiles.IsHash(entry.AfterHash))
        || !int.TryParse(Path.GetFileNameWithoutExtension(entry.Backup), out _) || Path.GetExtension(entry.Backup) != ".original"
        || entry.Backup.Contains('/')) {
        throw new TaskException("Invalid shared-task recovery entry.");
      }
    }
  }

  internal sealed record Journal( int Version, JournalEntry[] Entries );
  internal sealed record JournalEntry( string Path, string? BeforeHash, string? AfterHash, string Backup );
}

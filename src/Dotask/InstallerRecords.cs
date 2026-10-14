using System.Security.Cryptography;
using System.Text.Json;

namespace DoTask;

/// <summary>Explicit successful-build state, shared with the Rust task SDK.</summary>
internal static class InstallerRecords
{
  internal static async Task RecordAsync( BuildContext context, InstallerArtifact artifact, CancellationToken cancellationToken )
  {
    cancellationToken.ThrowIfCancellationRequested();
    InstallerRunner.Validate(artifact);
    if ((File.GetAttributes(artifact.FilePath) & FileAttributes.ReparsePoint) != 0) {
      throw new TaskException("Record the actual installer file, not a link.");
    }
    var name = context.OS.ToString().ToLowerInvariant() + "-" + context.Architecture.ToString().ToLowerInvariant();
    var directory = context.Path("artifacts", ".dotask", "installers");
    NoLinks(directory, context.RootDirectory);
    Directory.CreateDirectory(directory);
    var file = Path.Combine(directory, name + ".json");
    if (Path.GetFullPath(file).Equals(Path.GetFullPath(artifact.FilePath), OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal)) {
      throw new TaskException("Installer cannot be its own successful-build record.");
    }
    var lockFile = Path.Combine(directory, name + ".lock");
    NoLinks(file, context.RootDirectory);
    NoLinks(lockFile, context.RootDirectory);
    // FileShare.None cooperates with the Rust SDK's exclusive file lock. The
    // persistent lock file avoids unlink/reopen races between concurrent builds.
    using var ownedLock = new FileStream(lockFile, FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
    if (File.Exists(file)) {
      using var existing = JsonDocument.Parse(await File.ReadAllBytesAsync(file, cancellationToken));
      var root = existing.RootElement;
      if (root.ValueKind != JsonValueKind.Object || !root.TryGetProperty("Schema", out var schema) || !schema.TryGetInt32(out var number) || number != 1 ||
          !root.TryGetProperty("Installer", out var previous) || previous.ValueKind != JsonValueKind.Object ||
          !previous.TryGetProperty("OS", out var os) || !os.TryGetInt32(out var osNumber) || osNumber != (int)context.OS ||
          !previous.TryGetProperty("Architecture", out var arch) || !arch.TryGetInt32(out var archNumber) || archNumber != (int)context.Architecture ||
          !previous.TryGetProperty("Kind", out var kind) || !kind.TryGetInt32(out var kindNumber) || kindNumber is < 0 or > 3 ||
          !previous.TryGetProperty("FilePath", out var previousPath) || previousPath.ValueKind != JsonValueKind.String || !Path.IsPathFullyQualified(previousPath.GetString()!) ||
          !previous.TryGetProperty("DefaultArguments", out var args) || args.ValueKind != JsonValueKind.Array ||
          args.EnumerateArray().Any(a => a.ValueKind != JsonValueKind.String || a.GetString()!.Contains('\0')) ||
          !root.TryGetProperty("Sha256", out var sha) || sha.ValueKind != JsonValueKind.String ||
          sha.GetString() is not { Length: 64 } hash || hash.Any(c => !Uri.IsHexDigit(c)) ||
          !root.TryGetProperty("CompletedAtUnixMilliseconds", out var completed) || !completed.TryGetUInt64(out _)) {
        throw new TaskException($"Unrecognized installer record {file}; preserve it and correct it explicitly.");
      }
    }
    await using var input = File.OpenRead(artifact.FilePath);
    var digest = Convert.ToHexString(await SHA256.HashDataAsync(input, cancellationToken)).ToLowerInvariant();
    var record = new {
      Schema = 1,
      Installer = artifact,
      Sha256 = digest,
      CompletedAtUnixMilliseconds = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()
    };
    var temporary = Path.Combine(directory, Guid.NewGuid().ToString("N") + ".tmp");
    try {
      var options = new FileStreamOptions { Mode = FileMode.CreateNew, Access = FileAccess.Write, Share = FileShare.None };
      if (!OperatingSystem.IsWindows()) {
        options.UnixCreateMode = UnixFileMode.UserRead | UnixFileMode.UserWrite;
      }
      await using (var output = new FileStream(temporary, options)) {
        await JsonSerializer.SerializeAsync(output, record, cancellationToken: cancellationToken);
        await output.FlushAsync(cancellationToken);
        output.Flush(flushToDisk: true);
      }
      cancellationToken.ThrowIfCancellationRequested();
      File.Move(temporary, file, overwrite: true);
    } finally {
      File.Delete(temporary);
    }
  }

  private static void NoLinks( string path, string root )
  {
    // The selected project root may have junction ancestors. Protect generated
    // state beneath that root without rejecting the user's checkout location.
    var boundary = Path.TrimEndingDirectorySeparator(Path.GetFullPath(root));
    for (var current = Path.GetFullPath(path); current is not null; current = Path.GetDirectoryName(current)) {
      if (current.Equals(boundary, OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal)) {
        break;
      }
      if (Path.Exists(current) && (File.GetAttributes(current) & FileAttributes.ReparsePoint) != 0) {
        throw new TaskException($"Installer state cannot contain links: {current}");
      }
    }
  }
}

using System.Net;
using System.Text.Json;
using DoTask.Cli.Discovery;
using DoTask.Cli.Metadata;

namespace DoTask.Cli.SharedTasks;

internal sealed class SharedTaskStore( SharedTaskOptions options, HttpClient? client = null )
{
  private const string OnlineRoot = "https://raw.githubusercontent.com/kodybrown/dotask/main/shared-tasks/";
  private static readonly HttpClient DefaultClient = new() { Timeout = TimeSpan.FromSeconds(30) };
  private readonly HttpClient _client = client ?? DefaultClient;

  public SharedTaskOptions Options => options;

  public async Task<SharedCatalog> CatalogAsync( string source, bool refresh, CancellationToken token )
  {
    if (source == "private-tasks") {
      return PrivateCatalog();
    }
    if (source != "_") {
      throw new TaskException($"Unknown shared task source '{source}'. This version supports _ and private-tasks; additional repositories are deferred.");
    }
    var cache = SharedTaskFiles.Resolve(options.CacheDirectory, source + "/catalog.json");
    byte[] bytes;
    if (!refresh && File.Exists(cache)) {
      bytes = await File.ReadAllBytesAsync(cache, token);
    } else {
      bytes = await ReadOnlineAsync("catalog.json", token);
    }
    var catalog = JsonSerializer.Deserialize<SharedCatalog>(bytes, SharedTaskJson.Options)
      ?? throw new TaskException("Empty online task catalog.");
    ValidateCatalog(catalog);
    if (refresh || !File.Exists(cache)) {
      DoTask.Cli.Execution.TargetCompiler.CreatePrivateDirectory(options.CacheDirectory);
      SharedTaskFiles.AtomicWrite(cache, bytes);
    }
    return catalog;
  }

  public IEnumerable<(string Source, SharedTask Task)> CachedTasks()
  {
    var catalogPath = SharedTaskFiles.Resolve(options.CacheDirectory, "_/catalog.json");
    if (File.Exists(catalogPath)) {
      var catalog = JsonSerializer.Deserialize<SharedCatalog>(File.ReadAllBytes(catalogPath), SharedTaskJson.Options)!;
      ValidateCatalog(catalog);
      foreach (var task in catalog.Tasks) {
        yield return ("_", task);
      }
    }
    if (!Directory.Exists(options.PrivateDirectory)) {
      yield break;
    }
    foreach (var file in TargetCatalog.EnumerateSources(options.PrivateDirectory, false)) {
      var relative = Path.GetRelativePath(options.PrivateDirectory, file).Replace(Path.DirectorySeparatorChar, '/');
      var id = relative[..^3];
      try {
        SharedTaskFiles.ValidateId(id, 2);
      } catch (TaskException) {
        continue;
      }
      // Completion reads metadata only, without hashing assets or creating snapshots.
      yield return ("private-tasks", new(id, relative, MetadataReader.Read(file, options.PrivateDirectory).Description, "csharp", [], []));
    }
  }

  private SharedCatalog PrivateCatalog()
  {
    if (!Directory.Exists(options.PrivateDirectory)) {
      return new(1, []);
    }
    SharedTaskFiles.CheckLink(options.PrivateDirectory);
    var tasks = new List<SharedTask>();
    foreach (var file in TargetCatalog.EnumerateSources(options.PrivateDirectory, false)) {
      var relative = Path.GetRelativePath(options.PrivateDirectory, file).Replace(Path.DirectorySeparatorChar, '/');
      var id = relative[..^3];
      SharedTaskFiles.ValidateId(id, 2);
      var manifestPath = Path.ChangeExtension(file, ".task.json");
      SharedTaskFiles.CheckLink(manifestPath);
      if (File.Exists(manifestPath)) {
        throw new TaskException($"Move '{manifestPath}' into XML <requires task=\"...\" /> / <requires file=\"...\" /> comments in '{file}', then remove the .task.json file.");
      }
      var metadata = MetadataReader.Read(file, options.PrivateDirectory);
      if (metadata.Error is not null) {
        throw new TaskException($"{relative}: {metadata.Error}");
      }
      var support = metadata.Requirements.Where(r => r.Kind == "file").Select(r => r.Value);
      var requires = metadata.Requirements.Where(r => r.Kind == "task").Select(r => r.Value).ToArray();
      var files = new[] { relative }.Concat(support).Distinct(StringComparer.Ordinal).Select(path =>
      {
        var source = SharedTaskFiles.Resolve(options.PrivateDirectory, path);
        if (!File.Exists(source)) {
          throw new TaskException($"Missing private task support file: {source}");
        }
        return new SharedFile(path, SharedTaskFiles.HashFile(source)!);
      }).OrderBy(f => f.Path, StringComparer.Ordinal).ToArray();
      tasks.Add(new(id, relative, metadata.Description, "csharp", files, requires));
    }
    var catalog = new SharedCatalog(1, tasks.ToArray());
    ValidateCatalog(catalog);
    return catalog;
  }

  public static void ValidateCatalog( SharedCatalog catalog )
  {
    if (catalog.Version != 1 || catalog.Tasks is null || catalog.Tasks.Length > 10000) {
      throw new TaskException("Unsupported or invalid shared task catalog.");
    }
    var ids = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
    var allPaths = new Dictionary<string, (string Path, string Hash)>(StringComparer.OrdinalIgnoreCase);
    foreach (var task in catalog.Tasks) {
      if (task is null) {
        throw new TaskException("Invalid null catalog entry.");
      }
      SharedTaskFiles.ValidateId(task.Id, 2);
      if (!ids.Add(task.Id) || task.Runtime != "csharp" || task.EntryPoint != task.Id + ".cs"
        || task.Files is null || task.Files.Length is 0 or > 128 || task.Requires is null || task.Description is null) {
        throw new TaskException($"Invalid or unsupported shared task '{task.Id}'. Only C# tasks are supported in this version.");
      }
      var paths = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
      foreach (var file in task.Files) {
        if (file is null) {
          throw new TaskException($"Invalid null file in shared task '{task.Id}'.");
        }
        SharedTaskFiles.ValidateRelativePath(file.Path);
        if (!paths.Add(file.Path) || !SharedTaskFiles.IsHash(file.Sha256)
          || file.Path.Split('/').Length < 2 || file.Path.Split('/').Any(p => p.StartsWith('.'))
          || (file.Path.EndsWith(".cs", StringComparison.OrdinalIgnoreCase) && file.Path != task.EntryPoint
            && !file.Path.Split('/')[..^1].Any(p => p.StartsWith('_')))) {
          throw new TaskException($"Invalid file '{file.Path}' in shared task '{task.Id}'. C# support files must be inside an _support directory.");
        }
        if (allPaths.TryGetValue(file.Path, out var previous) && (previous.Path != file.Path || previous.Hash != file.Sha256)) {
          throw new TaskException($"Conflicting file definitions for '{file.Path}' in the online catalog.");
        }
        allPaths[file.Path] = (file.Path, file.Sha256);
      }
      if (!paths.Contains(task.EntryPoint)) {
        throw new TaskException($"Missing entry point file for '{task.Id}'.");
      }
      foreach (var dependency in task.Requires) {
        SharedTaskFiles.ValidateId(dependency, 2);
      }
    }
    foreach (var task in catalog.Tasks) {
      foreach (var dependency in task.Requires) {
        if (!ids.Contains(dependency)) {
          throw new TaskException($"Shared task '{task.Id}' requires missing task '{dependency}'.");
        }
      }
    }
  }

  public async Task SaveAsync( string source, SharedTask task, CancellationToken token )
  {
    DoTask.Cli.Execution.TargetCompiler.CreatePrivateDirectory(options.CacheDirectory);
    foreach (var file in task.Files) {
      token.ThrowIfCancellationRequested();
      var objectPath = ObjectPath(source, file.Sha256);
      var bytes = File.Exists(objectPath) ? await File.ReadAllBytesAsync(objectPath, token) : null;
      if (bytes is null || SharedTaskFiles.Hash(bytes) != file.Sha256) {
        bytes = source == "private-tasks"
          ? await File.ReadAllBytesAsync(SharedTaskFiles.Resolve(options.PrivateDirectory, file.Path), token)
          : await ReadOnlineAsync(file.Path, token);
        if (SharedTaskFiles.Hash(bytes) != file.Sha256) {
          throw new TaskException($"Shared task '{source}/{task.Id}' changed during download or failed SHA-256 verification ({file.Path}). Retry --sync/--save; project files have not been changed.");
        }
        SharedTaskFiles.AtomicWrite(objectPath, bytes);
      }
      // These are disposable downloaded copies. Private originals are never written.
      if (source != "private-tasks") {
        SharedTaskFiles.AtomicWrite(SharedTaskFiles.Resolve(options.CacheDirectory, source + "/" + file.Path), bytes);
      }
    }
    var snapshot = SharedTaskFiles.Resolve(options.CacheDirectory, source + "/.revisions/" + task.Revision + ".json");
    SharedTaskFiles.AtomicWrite(snapshot, JsonSerializer.SerializeToUtf8Bytes(task, SharedTaskJson.Options));
  }

  public byte[] Original( string source, string hash )
  {
    var path = ObjectPath(source, hash);
    if (!File.Exists(path)) {
      throw new TaskException($"The original task snapshot is unavailable in the local cache ({hash}). Project files are preserved.");
    }
    var bytes = File.ReadAllBytes(path);
    if (SharedTaskFiles.Hash(bytes) != hash) {
      throw new TaskException("A local cache snapshot failed its SHA-256 check. Project files are preserved.");
    }
    return bytes;
  }

  public string ObjectPath( string source, string hash )
  {
    SharedTaskFiles.ValidateId(source, 1);
    if (!SharedTaskFiles.IsHash(hash)) {
      throw new TaskException("Invalid snapshot hash.");
    }
    return SharedTaskFiles.Resolve(options.CacheDirectory, source + "/.objects/" + hash);
  }

  private async Task<byte[]> ReadOnlineAsync( string relative, CancellationToken token )
  {
    SharedTaskFiles.ValidateRelativePath(relative);
    if (options.OnlineDirectory is { } directory) {
      if (!Path.IsPathFullyQualified(directory)) {
        throw new TaskException("DOTASK_ONLINE_TASKS must be an absolute local directory containing catalog.json (for preview/publisher testing).");
      }
      return await File.ReadAllBytesAsync(SharedTaskFiles.Resolve(directory, relative), token);
    }
    using var response = await _client.GetAsync(OnlineRoot + relative, HttpCompletionOption.ResponseHeadersRead, token);
    if (response.StatusCode == HttpStatusCode.NotFound) {
      throw new TaskException($"The official online cache has not published '{relative}'. Private tasks and committed project tasks still work. See docs/SHARED-TASKS.md for preview catalog testing.");
    }
    response.EnsureSuccessStatusCode();
    const int maximumBytes = 16 * 1024 * 1024;
    if (response.Content.Headers.ContentLength > maximumBytes) {
      throw new TaskException("Shared task download exceeds the 16 MiB per-file limit.");
    }
    await using var input = await response.Content.ReadAsStreamAsync(token);
    using var output = new MemoryStream();
    var buffer = new byte[81920];
    int count;
    while ((count = await input.ReadAsync(buffer, token)) != 0) {
      if (output.Length + count > maximumBytes) {
        throw new TaskException("Shared task download exceeds the 16 MiB per-file limit.");
      }
      output.Write(buffer, 0, count);
    }
    return output.ToArray();
  }
}

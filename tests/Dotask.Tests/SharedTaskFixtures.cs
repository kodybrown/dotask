using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using YamlDotNet.Serialization;
using YamlDotNet.Serialization.NamingConventions;

namespace DoTask.Tests;

// Independent fixture serialization and inspection keep wire-format assertions
// useful without retaining the retired CLI's validators or management engine.
internal sealed record SharedTaskOptions( string CacheDirectory, string PrivateDirectory, string? OnlineDirectory = null );
internal sealed record SharedFile( string Path, string Sha256 );
internal sealed record SharedTask( string Id, string EntryPoint, string Description, string Runtime, SharedFile[] Files, string[] Requires );
internal sealed record SharedCatalog( int Version, SharedTask[] Tasks );
internal sealed record TransactionEntry( string Path, string? BeforeHash, string? AfterHash, string? Backup );
internal sealed record TransactionJournal( int Version, TransactionEntry[] Entries );

internal static class SharedTaskJson
{
  public static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web) {
    WriteIndented = true,
    UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow
  };
}

internal static class SharedTaskFiles
{
  public static string Hash( string value ) => Convert.ToHexStringLower(SHA256.HashData(Encoding.UTF8.GetBytes(value)));
  public static string? HashFile( string path ) => File.Exists(path) ? Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(path))) : null;
}

internal sealed class TaskLock
{
  public int Version { get; set; }
  public string Directory { get; set; } = "";
  public Dictionary<string, InstalledTask> Tasks { get; set; } = [];

  public static TaskLock Read( byte[] bytes, string taskDirectory )
  {
    var result = new DeserializerBuilder().WithNamingConvention(HyphenatedNamingConvention.Instance)
      .Build().Deserialize<TaskLock>(Encoding.UTF8.GetString(bytes));
    Assert.Equal(1, result.Version);
    Assert.Equal(taskDirectory, result.Directory);
    return result;
  }
}

internal sealed class InstalledTask
{
  public string Source { get; set; } = "";
  public string Revision { get; set; } = "";
  public string[] Requires { get; set; } = [];
  public Dictionary<string, string> Files { get; set; } = [];
}

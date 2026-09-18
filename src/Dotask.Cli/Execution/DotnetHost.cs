namespace DoTask.Cli.Execution;

internal static class DotnetHost
{
  public static string Find()
  {
    var configured = Environment.GetEnvironmentVariable("DOTNET_HOST_PATH");
    if (!string.IsNullOrWhiteSpace(configured) && File.Exists(configured)) {
      return configured;
    }
    if (string.Equals(Path.GetFileNameWithoutExtension(Environment.ProcessPath), "dotnet", StringComparison.OrdinalIgnoreCase)) {
      return Environment.ProcessPath!;
    }
    var root = Environment.GetEnvironmentVariable("DOTNET_ROOT");
    if (!string.IsNullOrWhiteSpace(root)) {
      var host = Path.Combine(root, OperatingSystem.IsWindows() ? "dotnet.exe" : "dotnet");
      if (File.Exists(host)) {
        return host;
      }
    }
    return "dotnet";
  }
}

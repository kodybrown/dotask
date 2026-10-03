using System.Text.Json;
using DoTask;

internal static class RustBuild
{
  public static async Task<string> TargetDirectory( BuildContext project )
  {
    // Cargo resolves the repository config and any explicit environment override.
    // Read its evaluated path so staging, tests and packaging follow the same policy.
    var metadata = await project.RunAsync(new ProcessDefinition {
      Executable = "cargo",
      Arguments = ["metadata", "--manifest-path", project.Path("Cargo.toml"), "--format-version", "1", "--no-deps", "--locked"],
      CaptureOutput = true
    });
    using var document = JsonDocument.Parse(metadata.StandardOutput);
    return document.RootElement.GetProperty("target_directory").GetString()!;
  }
}

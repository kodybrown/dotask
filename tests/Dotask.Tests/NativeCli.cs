namespace DoTask.Tests;

internal static class DotnetHost
{
  public static string Find() => Environment.GetEnvironmentVariable("DOTNET_HOST_PATH") is { Length: > 0 } host ? host : "dotnet";
}

// Test transport only: every command and completion query runs the native
// executable. No discovery, parsing, compilation or execution logic lives here.
internal static class NativeCli
{
  public static async Task<int> RunAsync( string[] arguments, string directory, TextWriter output, TextWriter error,
    SharedTaskOptions? sharedTaskOptions = null )
  {
    var result = await RustArtifacts.Run(directory, arguments, sharedTaskOptions);
    await output.WriteAsync(result.StandardOutput);
    await error.WriteAsync(result.StandardError);
    return result.ExitCode;
  }
}

internal static class NativeCompletion
{
  internal sealed record Candidate( string Value, string Kind );

  public static Candidate[] Complete( string line, string directory, string shell = "bash", SharedTaskOptions? sharedTaskOptions = null )
  {
    var result = RustArtifacts.Run(directory, ["__complete", "--line", line, "--shell", shell], sharedTaskOptions).GetAwaiter().GetResult();
    Assert.Equal(0, result.ExitCode);
    return result.StandardOutput.Split('\n', StringSplitOptions.RemoveEmptyEntries).Select(row =>
    {
      var fields = row.Split('\t');
      return new Candidate(fields[0], fields[1]);
    }).ToArray();
  }

  public static void PrintScript( string shell, TextWriter output )
  {
    var result = RustArtifacts.Run(Path.GetTempPath(), ["completion", shell]).GetAwaiter().GetResult();
    Assert.Equal(0, result.ExitCode);
    output.Write(result.StandardOutput);
  }

  public static void Query( string[] arguments, string directory, TextWriter output )
  {
    var result = RustArtifacts.Run(directory, ["__complete", .. arguments]).GetAwaiter().GetResult();
    Assert.Equal(0, result.ExitCode);
    output.Write(result.StandardOutput);
  }
}

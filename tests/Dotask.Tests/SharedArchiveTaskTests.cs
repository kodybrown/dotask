using System.Text.Json;

namespace DoTask.Tests;

public sealed class SharedArchiveTaskTests
{
  [Theory]
  [InlineData("zip")]
  [InlineData("7z")]
  public async Task ArchivesUseStagedOutputAndNamedFolderFromNestedInvocation( string format )
  {
    using var project = new TestProject();
    CopyTask(project, format);
    var executable = await BuildProbe(project);
    var source = Path.GetDirectoryName(project.Write("payload 日本語/file.txt", "payload"))!;
    var output = Path.Combine(project.Root, "archives", "archive." + format);
    var nested = Directory.CreateDirectory(Path.Combine(project.Root, "nested")).FullName;
    var result = await Run(project, format, executable, nested, source, output);
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    using var data = JsonDocument.Parse(File.ReadAllText(output));
    var args = data.RootElement.GetProperty("Args").EnumerateArray().Select(v => v.GetString()!).ToArray();
    Assert.Equal("a", args[0]);
    Assert.Equal("-t" + format, args[1]);
    Assert.Equal("-y", args[2]);
    Assert.Equal("--", args[4]);
    Assert.Equal(Path.GetFileName(source), args[5]);
    Assert.Equal(6, args.Length);
    Assert.Equal(project.Root, data.RootElement.GetProperty("Cwd").GetString());
    Assert.Equal("output." + format, Path.GetFileName(args[3]));
    Assert.NotEqual(output, args[3]);
    Assert.False(Directory.Exists(Path.GetDirectoryName(args[3])));
    Assert.Empty(Directory.GetDirectories(Path.GetDirectoryName(output)!, "dotask-output-*"));
  }

  [Theory]
  [InlineData("zip", "fail")]
  [InlineData("7z", "fail")]
  [InlineData("zip", "missing")]
  [InlineData("7z", "missing")]
  [InlineData("zip", "race")]
  [InlineData("7z", "race")]
  public async Task FailedOrRacingCreationNeverPublishesOrReplacesOutput( string format, string mode )
  {
    using var project = new TestProject();
    CopyTask(project, format);
    var executable = await BuildProbe(project);
    var source = Path.GetDirectoryName(project.Write("payload/file.txt", "payload"))!;
    var output = Path.Combine(project.Root, "archives", "archive." + format);
    var result = await Run(project, format, executable, project.Root, source, output, mode);
    Assert.Equal(mode == "fail" ? 19 : 1, result.ExitCode);
    if (mode == "race") {
      Assert.Equal("race winner", File.ReadAllText(output));
    } else {
      Assert.False(File.Exists(output));
    }
    Assert.Empty(Directory.GetDirectories(Path.GetDirectoryName(output)!, "dotask-output-*"));
  }

  [Theory]
  [InlineData("zip")]
  [InlineData("7z")]
  public async Task InvalidSourcesAndOutputsAreRejectedBeforeStartingArchiver( string format )
  {
    using var project = new TestProject();
    CopyTask(project, format);
    var sourceFile = project.Write("payload/file.txt", "payload");
    var source = Path.GetDirectoryName(sourceFile)!;
    var archiver = Path.Combine(project.Root, "missing-archiver");
    var existing = project.Write("existing." + format, "keep");
    var result = await Run(project, format, archiver, project.Root, source, existing);
    Assert.Contains("output must be new", result.StandardError);
    Assert.Equal("keep", File.ReadAllText(existing));
    result = await Run(project, format, archiver, project.Root, source, Path.Combine(source, "new/output." + format));
    Assert.Contains("outside its source directory", result.StandardError);
    Assert.False(Directory.Exists(Path.Combine(source, "new")));
    var missingOutput = Path.Combine(project.Root, "new/output." + format);
    result = await Run(project, format, archiver, project.Root, sourceFile, missingOutput);
    Assert.Contains("must be a directory", result.StandardError);
    Assert.False(Directory.Exists(Path.Combine(project.Root, "new")));
    result = await Run(project, format, archiver, project.Root, source + "-missing", missingOutput);
    Assert.Contains("Cannot resolve process path", result.StandardError);
    Assert.False(Directory.Exists(Path.Combine(project.Root, "new")));
    var help = await project.RunAsync("help", "_/archive/create-" + format);
    Assert.Equal(0, help.ExitCode);
    Assert.Contains("output-file", help.StandardOutput);
    Assert.False(Directory.Exists(Path.Combine(project.Root, "new")));
    result = await Run(project, format, archiver, project.Root, source, missingOutput);
    Assert.Equal(1, result.ExitCode);
    Assert.Contains("missing-archiver", result.StandardError);
    Assert.False(File.Exists(missingOutput));
    Assert.Empty(Directory.GetDirectories(Path.GetDirectoryName(missingOutput)!, "dotask-output-*"));
  }

  private static void CopyTask( TestProject project, string format )
  {
    using var stream = typeof(SharedArchiveTaskTests).Assembly.GetManifestResourceStream($"Shared/archive/create-{format}.task")!;
    using var reader = new StreamReader(stream);
    project.Write($".tasks/_/archive/create-{format}.task", reader.ReadToEnd());
  }

  private static async Task<string> BuildProbe( TestProject project )
  {
    var dll = await project.BuildProcessProbeAsync("""
      var mode = Environment.GetEnvironmentVariable("ARCHIVE_PROBE_MODE");
      var data = System.Text.Json.JsonSerializer.Serialize(new { Args = args, Cwd = Environment.CurrentDirectory });
      if (mode != "missing") File.WriteAllText(args[3], data);
      if (mode == "race") File.WriteAllText(Environment.GetEnvironmentVariable("ARCHIVE_PROBE_DESTINATION")!, "race winner");
      if (mode == "fail") Environment.Exit(19);
      """);
    return Path.Combine(Path.GetDirectoryName(dll)!, "Probe" + (OperatingSystem.IsWindows() ? ".exe" : ""));
  }

  private static Task<ProcessResult> Run( TestProject project, string format, string archiver, string cwd,
    string source, string output, string mode = "success" ) => ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = RustArtifacts.Binary("dotask"),
      Arguments = ["_/archive/create-" + format, "--archiver", archiver, "--source", source, "--output", output],
      WorkingDirectory = cwd,
      CaptureOutput = true,
      ThrowOnError = false,
      Environment = new Dictionary<string, string?> {
        ["ARCHIVE_PROBE_MODE"] = mode,
        ["ARCHIVE_PROBE_DESTINATION"] = output,
        ["DOTNET_HOST_PATH"] = "not-a-task-host",
        ["DOTASK_SOURCE_RUST_SDK"] = "not-a-task-sdk"
      }
    }, CancellationToken.None);
}

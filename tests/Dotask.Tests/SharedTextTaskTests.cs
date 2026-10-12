using System.Text.Json;

namespace DoTask.Tests;

public sealed class SharedTextTaskTests
{
  [Theory]
  [InlineData("lf")]
  [InlineData("crlf")]
  public async Task FixeolPreservesPatternsExclusionsAndBatchEolFromNestedInvocation( string eol )
  {
    using var project = new TestProject();
    CopyTask(project);
    var executable = await BuildFixeolProbe(project);
    var nested = Directory.CreateDirectory(Path.Combine(project.Root, "nested")).FullName;
    var result = await Run(project, executable, nested, ["fixeol", "--eol", eol]);
    Assert.True(result.ExitCode == 0, result.StandardError);
    var calls = result.StandardOutput.Split('\n', StringSplitOptions.RemoveEmptyEntries)
      .Select(line => JsonDocument.Parse(line)).ToArray();
    try {
      Assert.Equal(3, calls.Length);
      string[] patterns = [
        "*.code-workspace", "*.cs", "*.csproj", "*.css", "*.editorconfig",
        "*.gitattributes", "*.gitignore", "*.htm", "*.html", "*.ini", "*.js",
        "*.json", "*.md", "*.prettierrc", "*.ps1", "*.psm1", "*.pubxml",
        "*.py", "*.razor", "*.rest", "*.rs", "*.scss", "*.sh", "*.sln",
        "*.slnx", "*.sql", "*.text", "*.toml", "*.ts", "*.txt", "*.vue",
        "*.xml", "*.xsl", "*.yaml", "*.yml", "Makefile"
      ];
      string[] exclusions = [
        ".mcp_file_state.json", "*/.git/*", "*/.tasks/*", "*/node_modules/*",
        "*/bin/*", "*/obj/*", "*/dist/*", "*/artifacts/*", "*/target/*",
        "*/coverage/*", "*/playwright-report/*", "*/test-results/*", "*/TestResults/*"
      ];
      for (var index = 0; index < calls.Length; index++) {
        Assert.Equal(project.Root, calls[index].RootElement.GetProperty("Cwd").GetString());
        var expected = new[] { "--encoding", "utf8", "--eol", index == 0 ? eol : "crlf", "--recursive" }
          .Concat(index == 0 ? patterns : [index == 1 ? "*.cmd" : "*.bat"])
          .Concat(exclusions.SelectMany(pattern => new[] { "--exclude", pattern }));
        Assert.Equal(expected, calls[index].RootElement.GetProperty("Args").EnumerateArray().Select(v => v.GetString()));
      }
    } finally {
      foreach (var call in calls) {
        call.Dispose();
      }
    }
  }

  [Fact]
  public async Task FixeolFailureStopsBeforeBatchCommandsAndPreservesExitCode()
  {
    using var project = new TestProject();
    CopyTask(project);
    var executable = await BuildFixeolProbe(project);
    var result = await Run(project, executable, project.Root, ["fixeol"], "19");
    Assert.Equal(19, result.ExitCode);
    Assert.Single(result.StandardOutput.Split('\n', StringSplitOptions.RemoveEmptyEntries));
  }

  [Fact]
  public async Task FixeolRequiresItsToolButHelpAndOptionValidationDoNotExecuteIt()
  {
    using var project = new TestProject();
    CopyTask(project);
    var unavailable = Path.Combine(project.Root, "no tools");
    Directory.CreateDirectory(unavailable);
    var help = await Run(project, Path.Combine(unavailable, "fixeol"), project.Root, ["help", "fixeol"]);
    Assert.Equal(0, help.ExitCode);
    Assert.Contains("--eol", help.StandardOutput);
    Assert.Contains("tool: fixeol", help.StandardOutput);
    var missing = await Run(project, Path.Combine(unavailable, "fixeol"), project.Root, ["fixeol"]);
    Assert.Equal(1, missing.ExitCode);
    Assert.Contains("fixeol", missing.StandardError);
    var invalid = await Run(project, Path.Combine(unavailable, "fixeol"), project.Root, ["fixeol", "--eol", "invalid"]);
    Assert.Equal(1, invalid.ExitCode);
    Assert.Contains("eol", invalid.StandardError);
  }

  private static void CopyTask( TestProject project )
  {
    using var stream = typeof(SharedTextTaskTests).Assembly.GetManifestResourceStream("Shared/text/fixeol.task")!;
    using var reader = new StreamReader(stream);
    project.Write(".tasks/_/text/fixeol.task", reader.ReadToEnd());
  }

  private static async Task<string> BuildFixeolProbe( TestProject project )
  {
    var dll = await project.BuildProcessProbeAsync("""
      Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new { Args = args, Cwd = Environment.CurrentDirectory }));
      Environment.Exit(int.Parse(Environment.GetEnvironmentVariable("FIXEOL_PROBE_EXITCODE") ?? "0"));
      """);
    // Rename only the probe apphost; its embedded DLL name still resolves beside
    // it. Tests control PATH without requiring the real normalization utility.
    var extension = OperatingSystem.IsWindows() ? ".exe" : "";
    var executable = Path.Combine(Path.GetDirectoryName(dll)!, "fixeol" + extension);
    File.Copy(Path.Combine(Path.GetDirectoryName(dll)!, "Probe" + extension), executable);
    if (!OperatingSystem.IsWindows()) {
      File.SetUnixFileMode(executable, File.GetUnixFileMode(Path.Combine(Path.GetDirectoryName(dll)!, "Probe")));
    }
    return executable;
  }

  private static Task<ProcessResult> Run( TestProject project, string executable, string cwd, string[] arguments, string exitCode = "0" )
    => ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = RustArtifacts.Binary("dotask"),
      Arguments = arguments,
      WorkingDirectory = cwd,
      CaptureOutput = true,
      ThrowOnError = false,
      Environment = new Dictionary<string, string?> {
        ["PATH"] = Path.GetDirectoryName(executable),
        ["FIXEOL_PROBE_EXITCODE"] = exitCode
      }
    }, CancellationToken.None);
}

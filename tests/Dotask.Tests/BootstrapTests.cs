using System.Text.Json;

namespace DoTask.Tests;

public sealed class BootstrapTests
{
  [Fact]
  public async Task LauncherStagesRunnerForwardsArgumentsAndCleansUpOnSuccessAndTaskFailure()
  {
    using var fixture = new Fixture();
    foreach (var args in new[] { Array.Empty<string>(), new[] { "inspect", "-c", "Debug", "value with spaces", "literal! value" }, new[] { "", "inspect" }, new[] { "rebuild" }, new[] { "exit", "23" } }) {
      var result = await fixture.Run(args);
      var expected = args.FirstOrDefault() == "exit" ? 23 : 0;
      Assert.True(result.ExitCode == expected, result.StandardOutput + result.StandardError);
      var report = result.StandardOutput.Split('\n').Single(line => line.StartsWith("REPORT:", StringComparison.Ordinal))[7..];
      using var json = JsonDocument.Parse(report);
      Assert.Equal(args.Length == 0 ? ["verify"] : args,
        json.RootElement.GetProperty("args").EnumerateArray().Select(item => item.GetString()!).ToArray());
      // macOS can canonicalize /var to /private/var; compare directory identity
      // through a unique marker instead of comparing path spellings.
      Assert.Equal(fixture.Identity, File.ReadAllText(Path.Combine(json.RootElement.GetProperty("cwd").GetString()!, "bootstrap-owner")));
      var assembly = json.RootElement.GetProperty("assembly").GetString()!;
      var stagedDirectory = Path.GetDirectoryName(assembly)!;
      Assert.StartsWith("dotask-bootstrap", Path.GetFileName(stagedDirectory));
      Assert.Equal(fixture.Identity, File.ReadAllText(Path.Combine(Path.GetDirectoryName(stagedDirectory)!, "bootstrap-owner")));
      Assert.False(File.Exists(assembly));
      Assert.DoesNotContain("stale-publish-file.txt", json.RootElement.GetProperty("supportFiles").EnumerateArray().Select(item => item.GetString()));
      Assert.Empty(Directory.EnumerateDirectories(fixture.Temporary, "dotask-bootstrap*"));
    }
  }

  [Fact]
  public async Task LauncherDoesNotRunStaleOutputsAfterCompilationFails()
  {
    using var fixture = new Fixture();
    Assert.Equal(0, (await fixture.Run("inspect")).ExitCode);
    fixture.Project.Write("src/main.rs", "this is not valid Rust;");
    var failed = await fixture.Run("inspect");
    Assert.NotEqual(0, failed.ExitCode);
    Assert.DoesNotContain("REPORT:", failed.StandardOutput);
    Assert.Empty(Directory.EnumerateDirectories(fixture.Temporary, "dotask-bootstrap*"));
  }

  private sealed class Fixture : IDisposable
  {
    public TestProject Project { get; } = new();
    public string Temporary => Path.Combine(Project.Root, "temporary output with spaces");
    public string Identity { get; } = Guid.NewGuid().ToString("N");

    public Fixture()
    {
      foreach (var file in new[] { "build.sh", "build.cmd", "prepare-bootstrap.ps1" }) {
        using var stream = typeof(BootstrapTests).Assembly.GetManifestResourceStream("Bootstrap/" + file)!;
        using var reader = new StreamReader(stream);
        Project.Write(file == "prepare-bootstrap.ps1" ? ".tasks/misc/" + file : file, reader.ReadToEnd());
      }
      // Preserve the user output policy while exercising a custom publish path.
      Project.WriteBuildProperties();
      Project.Write("src/Dotask/Dotask.csproj", """
        <Project Sdk="Microsoft.NET.Sdk">
          <PropertyGroup>
            <TargetFramework>net10.0</TargetFramework>
            <OutputType>Library</OutputType>
            <AssemblyName>Dotask.dotnet</AssemblyName>
            <ImplicitUsings>enable</ImplicitUsings>
            <PublishDir>$(FixtureOutputRoot)/custom publish output/</PublishDir>
          </PropertyGroup>
        </Project>
        """);
      Project.Write("src/Dotask/Helpers.cs", "public static class Helpers { }");
      Project.Write("Cargo.toml", """
        [package]
        name = "dotask-cli"
        version = "0.0.0"
        edition = "2024"
        [workspace]
        [dependencies]
        serde_json = "=1.0.151"
        [[bin]]
        name = "dotask"
        path = "src/main.rs"
        """);
      var cargoOutput = Path.Combine(Path.GetTempPath(), "_rust", "dotask-bootstrap-tests", Identity).Replace('\\', '/');
      Project.Write(".cargo/config.toml", "[build]\ntarget-dir = \"" + cargoOutput + "\"\n");
      Project.Write("src/main.rs", """
        fn main() {
          let args: Vec<_> = std::env::args().skip(1).collect();
          println!("REPORT:{}", serde_json::json!({
            "args":args, "cwd":std::env::current_dir().unwrap(), "assembly":std::env::current_exe().unwrap(),
            "supportFiles":std::fs::read_dir(std::env::current_exe().unwrap().parent().unwrap()).unwrap()
              .map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect::<Vec<_>>()
          }));
          if args.first().is_some_and(|s| s == "rebuild") {
            // The live runner must be a physical copy so Cargo can overwrite its
            // normal output on Windows while this process remains alive.
            let code = std::process::Command::new("cargo").args(["build", "--release", "--locked"]).status().unwrap();
            if !code.success() { std::process::exit(code.code().unwrap_or(1)); }
          }
          if args.len() == 2 && args[0] == "exit" { std::process::exit(args[1].parse().unwrap()); }
        }
        """);
      Project.Write("src/dotask-sdk/Cargo.toml", "[package]\nname = \"dotask-sdk\"\nversion = \"0.0.0\"\n");
      Project.Write("src/dotask-sdk/src/lib.rs", "// bootstrap fixture SDK\n");
      Directory.CreateDirectory(Temporary);
      File.WriteAllText(Path.Combine(Temporary, "bootstrap-owner"), Identity);
      Project.Write("bootstrap-owner", Identity);
      Directory.CreateDirectory(Path.Combine(Project.Root, "called from here"));
    }

    public async Task<ProcessResult> Run( params string[] arguments )
    {
      using var timeout = new CancellationTokenSource(TimeSpan.FromMinutes(2));
      if (!File.Exists(Path.Combine(Project.Root, "Cargo.lock"))) {
        await ProcessRunner.RunAsync(new ProcessDefinition {
          Executable = "cargo",
          Arguments = ["generate-lockfile", "--offline"],
          WorkingDirectory = Project.Root,
          CaptureOutput = true
        }, timeout.Token);
        var properties = await ProcessRunner.RunAsync(new ProcessDefinition {
          Executable = "dotnet",
          Arguments = ["msbuild", "src/Dotask/Dotask.csproj", "-p:Configuration=Release", "-getProperty:PublishDir"],
          WorkingDirectory = Project.Root,
          CaptureOutput = true
        }, timeout.Token);
        var publish = Path.GetFullPath(properties.StandardOutput.Trim(), Path.Combine(Project.Root, "src/Dotask"));
        Directory.CreateDirectory(publish);
        File.WriteAllText(Path.Combine(publish, "stale-publish-file.txt"), "Must not enter the runner or a payload.");
      }
      // CALL is deliberately avoided so CMD does not expand arguments twice.
      var command = OperatingSystem.IsWindows()
        ? new[] { "/d", "/c", "..\\build.cmd" }.Concat(arguments).ToArray()
        : new[] { Path.Combine(Project.Root, "build.sh") }.Concat(arguments).ToArray();
      return await ProcessRunner.RunAsync(new ProcessDefinition {
        Executable = OperatingSystem.IsWindows() ? Environment.GetEnvironmentVariable("ComSpec") ?? "cmd.exe" : "/bin/bash",
        Arguments = command,
        WorkingDirectory = Path.Combine(Project.Root, "called from here"),
        Environment = new Dictionary<string, string?> { ["TMPDIR"] = Temporary, ["TEMP"] = Temporary, ["TMP"] = Temporary },
        CaptureOutput = true,
        ThrowOnError = false
      }, timeout.Token);
    }

    public void Dispose() => Project.Dispose();
  }
}

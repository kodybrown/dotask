using System.Text.Json;

namespace DoTask.Tests;

public sealed class BootstrapTests
{
  [Fact]
  public async Task LauncherStagesRunnerForwardsArgumentsAndCleansUpOnSuccessAndTaskFailure()
  {
    using var fixture = new Fixture();
    foreach (var args in new[] { Array.Empty<string>(), new[] { "inspect", "-c", "Debug", "value with spaces", "literal! value" }, new[] { "", "inspect" }, new[] { "rebuild" }, new[] { "exit", "23" } })
    {
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
      Assert.Empty(Directory.EnumerateDirectories(fixture.Temporary, "dotask-bootstrap*"));
    }
  }

  [Fact]
  public async Task LauncherDoesNotRunStaleOutputsAfterCompilationFails()
  {
    using var fixture = new Fixture();
    Assert.Equal(0, (await fixture.Run("inspect")).ExitCode);
    fixture.Project.Write("src/Dotask.Cli/Program.cs", "this is not valid C#;");
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
      foreach (var file in new[] { "build.sh", "build.cmd", "bootstrap.targets" })
      {
        using var stream = typeof(BootstrapTests).Assembly.GetManifestResourceStream("Bootstrap/" + file)!;
        using var reader = new StreamReader(stream);
        Project.Write(file == "bootstrap.targets" ? ".tasks/misc/" + file : file, reader.ReadToEnd());
      }
      // Isolate this synthetic SDK project from any ancestor's build policy.
      Project.Write("Directory.Build.props", "<Project />");
      Project.Write("src/Dotask.Cli/Dotask.Cli.csproj", """
        <Project Sdk="Microsoft.NET.Sdk">
          <PropertyGroup>
            <TargetFramework>net10.0</TargetFramework>
            <OutputType>Exe</OutputType>
            <AssemblyName>dotask</AssemblyName>
            <ImplicitUsings>enable</ImplicitUsings>
            <PublishDir>../../custom publish output/</PublishDir>
          </PropertyGroup>
          <Import Project="../../.tasks/misc/bootstrap.targets" />
        </Project>
        """);
      Project.Write("src/Dotask.Cli/Program.cs", """
        Console.WriteLine("REPORT:" + System.Text.Json.JsonSerializer.Serialize(new {
          args, cwd = Environment.CurrentDirectory, assembly = typeof(Program).Assembly.Location
        }));
        if (args.FirstOrDefault() == "rebuild") {
          // Recreate the original build AND publish output while the staged
          // runner is alive. Running from either original output fails on Windows.
          foreach (var verb in new[] { "clean", "publish" }) {
            var start = new System.Diagnostics.ProcessStartInfo("dotnet") { UseShellExecute = false };
            foreach (var argument in new[] { verb, "src/Dotask.Cli/Dotask.Cli.csproj", "-c", "Release", "--nologo" })
              start.ArgumentList.Add(argument);
            using var child = System.Diagnostics.Process.Start(start)!;
            child.WaitForExit();
            if (child.ExitCode != 0) return child.ExitCode;
          }
        }
        return args.Length == 2 && args[0] == "exit" ? int.Parse(args[1]) : 0;
        """);
      Directory.CreateDirectory(Temporary);
      File.WriteAllText(Path.Combine(Temporary, "bootstrap-owner"), Identity);
      Project.Write("bootstrap-owner", Identity);
      Directory.CreateDirectory(Path.Combine(Project.Root, "called from here"));
    }

    public async Task<ProcessResult> Run(params string[] arguments)
    {
      using var timeout = new CancellationTokenSource(TimeSpan.FromMinutes(2));
      // CALL is deliberately avoided so CMD does not expand arguments twice.
      var command = OperatingSystem.IsWindows()
        ? new[] { "/d", "/c", "..\\build.cmd" }.Concat(arguments).ToArray()
        : new[] { Path.Combine(Project.Root, "build.sh") }.Concat(arguments).ToArray();
      return await ProcessRunner.RunAsync(new ProcessDefinition
      {
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

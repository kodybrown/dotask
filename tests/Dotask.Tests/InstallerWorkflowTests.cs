using System.Runtime.InteropServices;
using System.Text.Json;

namespace DoTask.Tests;

public sealed class InstallerWorkflowTests
{
  [Fact]
  public async Task MissingFailedAmbiguousAndResultlessCreatorsNeverFallBackToDirectInstallation()
  {
    using var project = new TestProject { NativeRunner = true };
    CopyInstall(project);
    var result = await project.RunAsync("install");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Contains("Installation requires a 'create-installer' target", result.StandardError);
    project.Target("one/create-installer");
    project.Target("two/create-installer");
    result = await project.RunAsync("install");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Contains("Ambiguous target", result.StandardError);
    project.Target("create-installer", "Environment.Exit(17);");
    result = await project.RunAsync("install");
    Assert.Equal(17, result.ExitCode);
    project.Target("create-installer");
    result = await project.RunAsync("install");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Contains("did not return an installer", result.StandardError);
    project.Target("create-installer", "await BuildContext.Current.ExecTargetAsync(\"_/installer/install\");", async: true);
    result = await project.RunAsync("install");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Contains("Target cycle", result.StandardError);
  }

  [Fact]
  public async Task SharedInstallAlwaysBuildsAndRunsReturnedInstallerWithExactArgumentsAndCustomOverride()
  {
    using var project = new TestProject { NativeRunner = true };
    CopyInstall(project);
    project.WriteBuildProperties();
    project.Write("installer/Probe.csproj", """
      <Project Sdk="Microsoft.NET.Sdk"><PropertyGroup>
        <OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework>
        <ImplicitUsings>enable</ImplicitUsings>
      </PropertyGroup></Project>
      """);
    project.Write("installer/Program.cs", """
      System.IO.File.WriteAllText("invocation.json", System.Text.Json.JsonSerializer.Serialize(args));
      return args.Length == 1 && args[0] == "fail" ? 23 : 0;
      """);
    var output = Path.Combine(project.OutputRoot, "installer files 日本語");
    var build = await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = "dotnet",
      Arguments = ["publish", Path.Combine(project.Root, "installer/Probe.csproj"), "-o", output],
      CaptureOutput = true
    }, CancellationToken.None);
    Assert.Equal(0, build.ExitCode);
    project.Write(".dotasks.yaml", "version: 1\nsettings: { project: unused-gui.csproj }\n");
    project.Write("unused-gui.csproj", "<Project><PropertyGroup><OutputType>WinExe</OutputType></PropertyGroup></Project>");
    project.Target("create-installer", $$"""
      var project = BuildContext.Current;
      File.AppendAllText(project.Path("builds"), "built\n");
      await project.SetInstallerResultAsync(new InstallerArtifact {
        FilePath = {{JsonSerializer.Serialize(Path.Combine(output, "Probe.dll"))}}, Kind = InstallerKind.DotNetAssembly,
        OS = project.OS, Architecture = project.Architecture, DefaultArguments = ["default", "two words"]
      });
      """, async: true);
    var result = await project.RunAsync("install");
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.Equal(new[] { "default", "two words" }, ReadArguments());
    string[] arguments = ["", "two words", "a\"b", "'single'", @"trailing\", "日本語", "%BIN%", "& | ; < > $(nope)", "line1\nline2"];
    result = await project.RunAsync("install", "--installer-args", JsonSerializer.Serialize(arguments));
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Equal(arguments, ReadArguments());
    project.Target("install", """
      await BuildContext.Current.ExecTargetAsync("_/installer/install", new Dictionary<string, string> { ["installer-args"] = "[]" });
      """, async: true);
    result = await project.RunAsync("install");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Empty(ReadArguments());
    result = await project.RunAsync("_/installer/install", "--installer-args", "[\"fail\"]");
    Assert.Equal(23, result.ExitCode);
    Assert.Equal(4, File.ReadAllLines(Path.Combine(project.Root, "builds")).Length);
    result = await project.RunAsync("_/installer/install", "--installer-args", "not json");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Equal(4, File.ReadAllLines(Path.Combine(project.Root, "builds")).Length);
    // Exercise native apphost launch too (Windows uses shell activation for EXEs).
    await project.Context().RunInstallerAsync(new InstallerArtifact {
      FilePath = Path.Combine(output, OperatingSystem.IsWindows() ? "Probe.exe" : "Probe"),
      Kind = InstallerKind.Executable,
      OS = project.Context().OS,
      Architecture = project.Context().Architecture,
      DefaultArguments = arguments
    });
    Assert.Equal(arguments, ReadArguments());
    // A previous successful result cannot satisfy a later build that reports none.
    project.Target("create-installer");
    result = await project.RunAsync("install");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Contains("did not return an installer", result.StandardError);

    string[] ReadArguments() => JsonSerializer.Deserialize<string[]>(File.ReadAllText(Path.Combine(output, "invocation.json")))!;
  }

  [Fact]
  public async Task ResultValidationRejectsMissingArtifactsWrongPlatformsAndDuplicateResults()
  {
    using var project = new TestProject { NativeRunner = true };
    var context = project.Context();
    var file = project.Write(OperatingSystem.IsWindows() ? "installer.exe" : "installer", "not executed");
    var artifact = new InstallerArtifact { FilePath = file, Kind = InstallerKind.Executable, OS = context.OS, Architecture = context.Architecture };
    InstallerRunner.Validate(artifact);
    Assert.Throws<TaskException>(() => InstallerRunner.Validate(artifact with { FilePath = file + ".missing" }));
    Assert.Throws<TaskException>(() => InstallerRunner.Validate(artifact with { FilePath = "installer" }));
    Assert.Throws<TaskException>(() => InstallerRunner.Validate(artifact with { OS = HostOS.Unknown }));
    Assert.Throws<TaskException>(() => InstallerRunner.Validate(artifact with { OS = context.IsWindows ? HostOS.Linux : HostOS.Windows }));
    Assert.Throws<TaskException>(() => InstallerRunner.Validate(artifact with { Architecture = context.Architecture == Architecture.X64 ? Architecture.Arm64 : Architecture.X64 }));
    Assert.Throws<TaskException>(() => InstallerRunner.Validate(artifact with { Kind = (InstallerKind)99 }));
    foreach (var invalid in new[] { "null", "{}", "[null]", "[1]", "[\"\\u0000\"]" }) {
      Assert.Throws<TaskException>(() => InstallerRunner.ParseArguments(invalid));
    }
    await context.SetInstallerResultAsync(artifact);
    await Assert.ThrowsAsync<IOException>(() => context.SetInstallerResultAsync(artifact));
    using var cancelled = new CancellationTokenSource();
    cancelled.Cancel();
    await Assert.ThrowsAnyAsync<OperationCanceledException>(() => context.CreateInstallerAsync(cancellationToken: cancelled.Token));
  }

  [Fact]
  public async Task ShellInstallerRunsWithoutExecutablePermissionAndPropagatesFailure()
  {
    if (OperatingSystem.IsWindows()) {
      return;
    }

    using var project = new TestProject { NativeRunner = true };
    var file = project.Write("installer with spaces.sh", "printf '%s' \"$1\" > result\nexit 19\n");
    var context = project.Context();
    var error = await Assert.ThrowsAsync<ProcessFailedException>(() => context.RunInstallerAsync(new InstallerArtifact {
      FilePath = file,
      Kind = InstallerKind.ShellScript,
      OS = context.OS,
      Architecture = context.Architecture,
      DefaultArguments = ["literal $(no shell) 日本語"]
    }));
    Assert.Equal(19, error.ExitCode);
    Assert.Equal("literal $(no shell) 日本語", File.ReadAllText(Path.Combine(project.Root, "result")));
  }

  private static void CopyInstall( TestProject project )
  {
    using var stream = typeof(InstallerWorkflowTests).Assembly.GetManifestResourceStream("Shared/installer/install.rs")!;
    using var reader = new StreamReader(stream);
    project.Write(".tasks/_/installer/install.rs", reader.ReadToEnd());
  }
}

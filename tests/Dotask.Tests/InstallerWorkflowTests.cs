using System.Runtime.InteropServices;
using System.Text.Json;

namespace DoTask.Tests;

public sealed class InstallerWorkflowTests
{
  [Fact]
  public async Task MissingConfigurationExplainsSetupWithoutBuildingOrChangingUserFiles()
  {
    using var project = new TestProject();
    CopyInstall(project);
    var configuration = project.Write(".dotasks.yaml", "# user-owned\nversion: 1\nsettings: {}\n");
    var before = File.ReadAllBytes(configuration);
    project.Target("create-installer", "File.WriteAllText(BuildContext.Current.Path(\"must-not-build\"), \"bad\");");
    var result = await project.RunAsync("install");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Contains("installer_file", result.StandardOutput);
    Assert.Contains("YOUR-APP", result.StandardOutput);
    Assert.Contains("interactive terminal", result.StandardError);
    Assert.Equal(before, File.ReadAllBytes(configuration));
    Assert.False(File.Exists(Path.Combine(project.Root, "must-not-build")));
    Assert.False(Directory.Exists(Path.Combine(project.Root, "artifacts")));
  }

  [Fact]
  public async Task SharedInstallUsesCSharpRecordedOutputWithoutCallingACreatorAndPreservesArguments()
  {
    using var project = new TestProject();
    CopyInstall(project);
    var probe = await project.BuildProcessProbeAsync("""
      File.WriteAllText("invocation.json", System.Text.Json.JsonSerializer.Serialize(args));
      return args.Length == 1 && args[0] == "fail" ? 23 : 0;
      """);
    var configuration = project.Write(".dotasks.yaml", JsonSerializer.Serialize(new {
      version = 1,
      settings = new { installer = new { installer_file = probe } }
    }));
    var before = File.ReadAllBytes(configuration);
    project.Target("create-installer", "Environment.Exit(99);");
    var context = project.Context();
    var artifact = new InstallerArtifact {
      FilePath = probe,
      Kind = InstallerKind.DotNetAssembly,
      OS = context.OS,
      Architecture = context.Architecture,
      DefaultArguments = ["default", "two words"]
    };
    await context.RecordInstallerAsync(artifact);
    var record = Directory.GetFiles(Path.Combine(project.Root, "artifacts/.dotask/installers"), "*.json").Single();
    var recorded = File.ReadAllBytes(record);
    var result = await project.RunAsync("install");
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.Equal(["default", "two words"], ReadArguments());
    string[] arguments = ["", "two words", "a\"b", "'single'", @"trailing\", "日本語", "%BIN%", "& | ; < > $(nope)", "line1\nline2"];
    result = await project.RunAsync("install", "--installer-args", JsonSerializer.Serialize(arguments));
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Equal(arguments, ReadArguments());
    result = await project.RunAsync("install", "--installer-args", "[\"fail\"]");
    Assert.Equal(23, result.ExitCode);
    result = await project.RunAsync("install", "--installer-args", "not json");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Equal(before, File.ReadAllBytes(configuration));
    Assert.Equal(recorded, File.ReadAllBytes(record));
    var native = Path.Combine(Path.GetDirectoryName(probe)!, OperatingSystem.IsWindows() ? "Probe.exe" : "Probe");
    project.Write(".dotasks.yaml", JsonSerializer.Serialize(new {
      version = 1,
      settings = new { installer = new { installer_file = native } }
    }));
    result = await project.RunAsync("install", "--installer-args", JsonSerializer.Serialize(arguments));
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.Equal(arguments, ReadArguments());
    string[] ReadArguments() => JsonSerializer.Deserialize<string[]>(File.ReadAllText(Path.Combine(Path.GetDirectoryName(probe)!, "invocation.json")))!;
  }

  [Fact]
  public async Task RecordingIsExplicitAndFailedOrUnrecognizedUpdatesPreservePreviousState()
  {
    using var project = new TestProject();
    var context = project.Context();
    var file = project.Write(OperatingSystem.IsWindows() ? "setup.exe" : "setup", "not executed");
    var artifact = new InstallerArtifact {
      FilePath = file,
      Kind = InstallerKind.Executable,
      OS = context.OS,
      Architecture = context.Architecture
    };
    await context.SetInstallerResultAsync(artifact);
    Assert.False(Directory.Exists(Path.Combine(project.Root, "artifacts")));
    await context.RecordInstallerAsync(artifact);
    var record = Directory.GetFiles(Path.Combine(project.Root, "artifacts/.dotask/installers"), "*.json").Single();
    var before = File.ReadAllBytes(record);
    await Assert.ThrowsAsync<TaskException>(() => context.RecordInstallerAsync(artifact with { FilePath = file + ".missing" }));
    Assert.Equal(before, File.ReadAllBytes(record));
    File.WriteAllText(record, "{\"Schema\":42}");
    await Assert.ThrowsAsync<TaskException>(() => context.RecordInstallerAsync(artifact));
    Assert.Equal("{\"Schema\":42}", File.ReadAllText(record));
  }
  [Fact]
  public async Task ResultValidationRejectsMissingArtifactsWrongPlatformsAndDuplicateResults()
  {
    using var project = new TestProject();
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

    using var project = new TestProject();
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

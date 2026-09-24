using System.Runtime.InteropServices;
using System.Text.Json;

namespace DoTask.Tests;

public sealed class InstallerWorkflowTests
{
  [Fact]
  public async Task MissingFailedAmbiguousAndResultlessCreatorsNeverFallBackToDirectInstallation()
  {
    using var project = new TestProject();
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
    project.Target("create-installer", "await BuildContext.Current.ExecTargetAsync(\"_/dotnet/install\");", async: true);
    result = await project.RunAsync("install");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Contains("Target cycle", result.StandardError);
  }

  [Fact]
  public async Task SharedInstallAlwaysBuildsAndRunsReturnedInstallerWithExactArgumentsAndCustomOverride()
  {
    using var project = new TestProject();
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
      await BuildContext.Current.ExecTargetAsync("_/dotnet/install", new Dictionary<string, string> { ["installer-args"] = "[]" });
      """, async: true);
    result = await project.RunAsync("install");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Empty(ReadArguments());
    result = await project.RunAsync("_/dotnet/install", "--installer-args", "[\"fail\"]");
    Assert.Equal(23, result.ExitCode);
    Assert.Equal(4, File.ReadAllLines(Path.Combine(project.Root, "builds")).Length);
    result = await project.RunAsync("_/dotnet/install", "--installer-args", "not json");
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

  [Fact]
  public async Task StandaloneDotaskInstallerUpdatesLegacyOwnedInstallationAndReusesBuild()
  {
    using var project = new TestProject();
    var executable = OperatingSystem.IsWindows() ? "dotask.exe" : "dotask";
    var source = project.Write("payload/" + executable, "original build");
    if (!OperatingSystem.IsWindows()) {
      File.SetUnixFileMode(source, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
    }

    var definition = new InstallationDefinition {
      AppId = "dotask",
      Version = "0.1.0",
      SourceDirectory = Path.GetDirectoryName(source)!,
      Commands = [new InstalledCommand("dotask", executable)],
      InstallRoot = Path.Combine(project.Root, "apps"),
      BinDirectory = Path.Combine(project.Root, "bin")
    };
    var legacy = await UserInstaller.InstallAsync(definition);
    var package = Path.Combine(project.Root, "installer package 日本語");
    Directory.CreateDirectory(package);
    foreach (var name in new[] { "dotask-installer.dll", "dotask-installer.deps.json", "dotask-installer.runtimeconfig.json", "Dotask.Library.dll" }) {
      File.Copy(Path.Combine(AppContext.BaseDirectory, name), Path.Combine(package, name));
    }
    var payload = project.Write("installer package 日本語/payload/" + executable, "new build");
    if (!OperatingSystem.IsWindows()) {
      File.SetUnixFileMode(payload, File.GetUnixFileMode(source));
    }

    project.Write("installer package 日本語/installer.json", JsonSerializer.Serialize(definition with {
      SourceDirectory = "payload",
      InstallRoot = null,
      BinDirectory = null
    }));
    async Task<ProcessResult> Run( params string[] args ) => await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = "dotnet",
      Arguments = [Path.Combine(package, "dotask-installer.dll"), .. args],
      WorkingDirectory = project.Root,
      CaptureOutput = true,
      ThrowOnError = false
    }, CancellationToken.None);
    string[] options = ["--install-root", definition.InstallRoot, "--bin-dir", definition.BinDirectory];
    var result = await Run(options);
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.Contains("Installed dotask", result.StandardOutput);
    Assert.Equal("original build", File.ReadAllText(Path.Combine(legacy.InstallDirectory, executable)));
    Assert.Equal(2, Directory.GetDirectories(Path.Combine(definition.InstallRoot, "dotask")).Length);
    result = await Run(options);
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Contains("Activated existing", result.StandardOutput);
    Assert.Equal(2, Directory.GetDirectories(Path.Combine(definition.InstallRoot, "dotask")).Length);
    result = await Run("--unknown");
    Assert.Equal(1, result.ExitCode);
    Assert.Contains("Usage:", result.StandardError);
  }

  private static void CopyInstall( TestProject project )
  {
    using var stream = typeof(InstallerWorkflowTests).Assembly.GetManifestResourceStream("Shared/dotnet/install.cs")!;
    using var reader = new StreamReader(stream);
    project.Write(".tasks/_/dotnet/install.cs", reader.ReadToEnd());
  }
}

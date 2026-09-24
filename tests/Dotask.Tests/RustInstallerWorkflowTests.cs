using System.Text.Json;
using DoTask.Cli;

namespace DoTask.Tests;

public sealed class RustInstallerWorkflowTests
{
  [Fact]
  public async Task SharedTasksPackageInstallLaunchAndUninstallGuiPayloadWithoutOriginalPackage()
  {
    using var project = new TestProject();
    var engine = OperatingSystem.IsWindows() ? "C:/tmp/_dotnet/dotask-rust/release/dotask-installer.exe" : "/tmp/_dotnet/dotask-rust/release/dotask-installer";
    Assert.True(File.Exists(engine), "Build the Rust engine first: build.cmd installer-engine (Unix: ./build.sh installer-engine).");
    project.WriteBuildProperties();
    var source = project.Write("GuiProbe/GuiProbe.csproj", """
      <Project Sdk="Microsoft.NET.Sdk">
        <PropertyGroup>
          <TargetFramework>net10.0</TargetFramework>
          <OutputType>WinExe</OutputType>
          <AssemblyName>gui-probe</AssemblyName>
          <UseAppHost>true</UseAppHost>
        </PropertyGroup>
      </Project>
      """);
    project.Write("GuiProbe/Program.cs", "System.IO.File.WriteAllText(args[0], \"GUI payload launched\");");
    async Task<ProcessResult> Run( string executable, params string[] args ) => await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = executable,
      Arguments = args,
      WorkingDirectory = project.Root,
      CaptureOutput = true,
      ThrowOnError = false
    }, CancellationToken.None);
    var published = await Run("dotnet", "publish", source, "-c", "Release", "--nologo", "--verbosity", "quiet", "-getProperty:PublishDir");
    Assert.True(published.ExitCode == 0, published.StandardOutput + published.StandardError);
    var payload = Path.GetFullPath(published.StandardOutput.Trim(), Path.GetDirectoryName(source)!);
    Assert.True(Directory.Exists(payload), published.StandardOutput);

    // Exercise the supported tool-managed consumer workflow, not hand-copied tasks.
    var assembly = typeof(RustInstallerWorkflowTests).Assembly;
    var catalog = Path.Combine(project.Root, "shared");
    foreach (var resource in assembly.GetManifestResourceNames().Where(n => n.StartsWith("Shared/", StringComparison.Ordinal))) {
      using var stream = assembly.GetManifestResourceStream(resource)!;
      var path = Path.Combine(catalog, resource["Shared/".Length..]);
      Directory.CreateDirectory(Path.GetDirectoryName(path)!);
      using var output = File.Create(path);
      await stream.CopyToAsync(output);
    }
    var added = await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = "dotnet",
      Arguments = [typeof(CliApplication).Assembly.Location, "--add", "_/dotask-installer/*"],
      Environment = new Dictionary<string, string?> { ["DOTASK_ONLINE_TASKS"] = catalog },
      WorkingDirectory = project.Root,
      CaptureOutput = true,
      ThrowOnError = false
    }, CancellationToken.None);
    Assert.True(added.ExitCode == 0, added.StandardOutput + added.StandardError);
    var installed = Path.Combine(project.Root, "installed app");
    var bin = Path.Combine(project.Root, "commands");
    var desktop = Path.Combine(project.Root, "desktop");
    var marker = Path.Combine(project.Root, "launched");
    var config = project.Write("installer.yaml", JsonSerializer.Serialize(new {
      schema = 1,
      application = new { id = "gui-probe", name = "GUI Probe", version = "1.0.0" },
      platform = OperatingSystem.IsWindows() ? "windows" : OperatingSystem.IsMacOS() ? "macos" : "linux",
      architecture = System.Runtime.InteropServices.RuntimeInformation.OSArchitecture == System.Runtime.InteropServices.Architecture.Arm64 ? "arm64" : "x64",
      payload,
      commands = new[] { new { name = "gui-probe", executable = OperatingSystem.IsWindows() ? "gui-probe.exe" : "gui-probe" } },
      shortcuts = new[] { new { name = "GUI Probe", executable = OperatingSystem.IsWindows() ? "gui-probe.exe" : "gui-probe", arguments = new[] { marker }, desktop = true, local = true, terminal = OperatingSystem.IsMacOS() } },
      values = new Dictionary<string, object> { ["install-dir"] = installed, ["bin-dir"] = bin, ["desktop-dir"] = desktop, ["desktop-shortcuts"] = true, ["local-shortcuts"] = true }
    }));
    project.Write(".dotasks.yaml", JsonSerializer.Serialize(new {
      version = 1,
      settings = new Dictionary<string, string> {
        ["installer-config"] = config,
        ["installer-engine"] = engine,
        ["installer-output"] = Path.Combine(project.Root, "packages")
      }
    }));
    var installation = await project.RunAsync("install");
    Assert.True(installation.ExitCode == 0, installation.StandardOutput + installation.StandardError);
    var launcher = Path.Combine(installed, OperatingSystem.IsWindows() ? "gui-probe.exe" : "gui-probe");
    var launched = await Run(launcher, marker);
    Assert.True(launched.ExitCode == 0, launched.StandardError);
    Assert.Equal("GUI payload launched", File.ReadAllText(marker));
    var extension = OperatingSystem.IsWindows() ? ".lnk" : OperatingSystem.IsMacOS() ? ".command" : ".desktop";
    Assert.True(File.Exists(Path.Combine(installed, "GUI Probe" + extension)));
    Assert.True(File.Exists(Path.Combine(desktop, "GUI Probe" + extension)));
    Directory.Delete(Path.Combine(project.Root, "packages"), recursive: true);
    File.Delete(config);
    var removed = await project.RunAsync("uninstall", "--install-dir", installed);
    Assert.True(removed.ExitCode == 0, removed.StandardOutput + removed.StandardError);
    Assert.False(Directory.Exists(installed));
    Assert.False(File.Exists(Path.Combine(desktop, "GUI Probe" + extension)));
  }
}

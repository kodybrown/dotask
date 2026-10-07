using System.Text.Json;

namespace DoTask.Tests;

public sealed class RustInstallerWorkflowTests
{
  [Fact]
  public async Task SharedTasksPackageInstallLaunchAndUninstallGuiPayloadWithoutOriginalPackage()
  {
    using var project = new TestProject { NativeRunner = true };
    var engine = RustArtifacts.Binary("doinstall");
    var builder = RustArtifacts.Binary("doinstall-builder");
    Assert.True(File.Exists(engine) && File.Exists(builder), "Build the installer first: build.cmd verify, or set DOINSTALL_TOOL_DIR to a built doinstall tool pair.");
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
    var cache = Path.Combine(project.Root, "cache");
    foreach (var resource in assembly.GetManifestResourceNames().Where(n => n.StartsWith("Shared/", StringComparison.Ordinal))) {
      using var stream = assembly.GetManifestResourceStream(resource)!;
      var path = Path.Combine(catalog, resource["Shared/".Length..]);
      Directory.CreateDirectory(Path.GetDirectoryName(path)!);
      using var output = File.Create(path);
      await stream.CopyToAsync(output);
    }
    var added = await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = RustArtifacts.Binary("dotask"),
      Arguments = ["--add", "_/doinstall/*", "_/installer/install"],
      // --add prefers a cached catalog even when the online source is overridden.
      // Keep all shared-task locations fixture-owned so an older user catalog
      // cannot hide the installer tasks or receive test downloads and locks.
      Environment = new Dictionary<string, string?> {
        ["DOTASK_ONLINE_TASKS"] = catalog,
        ["DOTASK_CACHE_HOME"] = cache,
        ["DOTASK_PRIVATE_TASKS"] = Path.Combine(project.Root, "private")
      },
      WorkingDirectory = project.Root,
      CaptureOutput = true,
      ThrowOnError = false
    }, CancellationToken.None);
    Assert.True(added.ExitCode == 0, added.StandardOutput + added.StandardError);
    var cachedCatalog = Path.Combine(cache, "_", "catalog.json");
    Assert.True(File.Exists(cachedCatalog), "Shared-task installation must populate the fixture-owned cache.");
    Assert.Equal(File.ReadAllBytes(Path.Combine(catalog, "catalog.json")), File.ReadAllBytes(cachedCatalog));
    var installed = Path.Combine(project.Root, "installed app");
    var bin = Path.Combine(project.Root, "commands");
    var desktop = Path.Combine(project.Root, "desktop");
    var marker = Path.Combine(project.Root, "launched");
    var sourceConfig = JsonSerializer.SerializeToNode(new {
      schema = 1,
      application = new { id = "gui-probe", name = "GUI Probe", version = "1.0.0" },
      runtime = new {
        interactive = false,
        commands = new[] { new { name = "gui-probe", executable = OperatingSystem.IsWindows() ? "gui-probe.exe" : "gui-probe" } },
        shortcuts = new[] { new { name = "GUI Probe", executable = OperatingSystem.IsWindows() ? "gui-probe.exe" : "gui-probe", arguments = new[] { marker }, desktop = true, local = true, terminal = OperatingSystem.IsMacOS() } },
        values = new Dictionary<string, object> { ["install-dir"] = installed, ["bin-dir"] = bin, ["additional-command"] = true, ["add-to-path"] = false, ["desktop-dir"] = desktop, ["desktop-shortcuts"] = true, ["local-shortcuts"] = true }
      }
    })!;
    sourceConfig["build-steps"] = JsonSerializer.SerializeToNode(new object[] {
      new { gather = new[] { new { from = payload, files = Directory.EnumerateFiles(payload).Select(Path.GetFileName).ToArray() } } },
      new {
        run = "_/doinstall/assemble",
        with = new Dictionary<string, object> {
          ["builder"] = builder, ["installer"] = engine, ["config"] = "${config}", ["output"] = "${package}"
        }
      }
    });
    var config = project.Write("installer.yaml", sourceConfig.ToJsonString());
    project.Write(".tasks/create-installer.rs", """
      use dotask_sdk::{json, BuildContext, Result};
      fn main() { dotask_sdk::run(task); }
      fn task(project: &BuildContext) -> Result<()> {
        let artifact = project.create_installer("_/doinstall/create-installer", json!({}))?;
        project.set_installer_result(&artifact)
      }
      """);
    project.Write(".dotasks.yaml", JsonSerializer.Serialize(new {
      version = 1,
      settings = new Dictionary<string, object> {
        ["installer"] = new { config, output = Path.Combine(project.Root, "packages") }
      }
    }));
    var installation = await project.RunAsync("install");
    Assert.True(installation.ExitCode == 0, installation.StandardOutput + installation.StandardError);
    var packagedConfig = Assert.Single(Directory.EnumerateFiles(Path.Combine(project.Root, "packages"), "installer.yaml", SearchOption.AllDirectories));
    var shippedConfig = File.ReadAllText(packagedConfig);
    Assert.Contains("runtime:", shippedConfig);
    Assert.DoesNotContain("build-steps:", shippedConfig);
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

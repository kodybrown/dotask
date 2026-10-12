using System.Text.Json;

namespace DoTask.Tests;

public sealed class RustInstallerWorkflowTests
{
  [Theory]
  [InlineData(false)]
  [InlineData(true)]
  public async Task DeclarativeAssemblyUsesSeparateToolsWithoutLanguageSdks( bool explicitBuilder )
  {
    using var project = new TestProject();
    var engine = RustArtifacts.Binary("doinstall");
    var builder = RustArtifacts.Binary("doinstall-builder");
    using var resource = typeof(RustInstallerWorkflowTests).Assembly.GetManifestResourceStream("Shared/doinstall/assemble.task")!;
    using var reader = new StreamReader(resource);
    project.Write(".tasks/_/doinstall/assemble.task", await reader.ReadToEndAsync());
    project.Write(".dotasks.yaml", "version: 1\n");
    var payloadFile = project.Write("payload 日本語/hello.txt", "A consumer-owned payload.");
    var payloadCommand = "probe" + (OperatingSystem.IsWindows() ? ".exe" : "");
    File.Copy(RustArtifacts.Binary("dotask"), Path.Combine(Path.GetDirectoryName(payloadFile)!, payloadCommand));
    var config = project.Write(".tasks/resolved installer.yaml", JsonSerializer.Serialize(new {
      schema = 1,
      application = new { id = "adapter-probe", name = "Adapter Probe", version = "1.0.0" },
      platform = project.Context().OS.ToString().ToLowerInvariant(),
      architecture = project.Context().Architecture.ToString().ToLowerInvariant(),
      payload = Path.GetDirectoryName(payloadFile),
      runtime = new {
        interactive = false,
        commands = new[] { new { name = "adapter-probe", executable = payloadCommand } },
        shortcuts = Array.Empty<object>()
      }
    }));
    var native = Path.Combine(project.Root, "standalone", Path.GetFileName(RustArtifacts.Binary("dotask")));
    Directory.CreateDirectory(Path.GetDirectoryName(native)!);
    File.Copy(RustArtifacts.Binary("dotask"), native);
    var nested = Path.Combine(project.Root, "nested");
    Directory.CreateDirectory(nested);
    var output = Path.Combine(project.Root, "artifacts", "package 日本語");
    var arguments = new List<string> {
      "_/doinstall/assemble.task", "--installer", engine, "--config", Path.GetRelativePath(project.Root, config),
      "--output", Path.GetRelativePath(project.Root, output)
    };
    if (explicitBuilder) {
      arguments.AddRange(["--builder", builder]);
    }
    // Only the native CLI is copied. PATH contains the prebuilt pair, no Cargo
    // or .NET, and invalid SDK overrides expose accidental code-task execution.
    async Task<ProcessResult> Run( IReadOnlyList<string> args ) => await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = native,
      Arguments = args,
      WorkingDirectory = nested,
      CaptureOutput = true,
      ThrowOnError = false,
      Environment = new Dictionary<string, string?> {
        ["PATH"] = Path.GetDirectoryName(builder),
        ["DOTNET_HOST_PATH"] = "not-a-host",
        ["DOTASK_SOURCE_RUST_SDK"] = "not-an-sdk"
      }
    }, CancellationToken.None);
    var help = await Run(["help", "_/doinstall/assemble.task"]);
    Assert.Equal(0, help.ExitCode);
    Assert.Contains("doinstall-builder", help.StandardOutput);
    Assert.False(Directory.Exists(output));
    var assembled = await Run(arguments);
    Assert.True(assembled.ExitCode == 0, assembled.StandardOutput + assembled.StandardError);
    Assert.Equal("A consumer-owned payload.", File.ReadAllText(Path.Combine(output, "payload", "hello.txt")));
    Assert.True(File.Exists(Path.Combine(output, "installer.yaml")));
    Assert.True(File.Exists(Path.Combine(output, OperatingSystem.IsWindows() ? "installer.exe" : "installer")));
    Assert.False(File.Exists(Path.Combine(output, Path.GetFileName(builder))));
    var before = File.ReadAllBytes(Path.Combine(output, "installer.yaml"));
    Assert.NotEqual(0, (await Run(arguments)).ExitCode);
    Assert.Equal(before, File.ReadAllBytes(Path.Combine(output, "installer.yaml")));
    var missing = await Run(["_/doinstall/assemble.task", "--builder", "missing-doinstall-builder", "--installer", engine,
      "--config", config, "--output", output + "-missing"]);
    Assert.NotEqual(0, missing.ExitCode);
    Assert.Contains("missing-doinstall-builder", missing.StandardError);
    Assert.False(Directory.Exists(output + "-missing"));
    Assert.False(Directory.Exists(Path.Combine(project.Root, "standalone", "sdk")));
  }

  [Fact]
  public async Task SharedTasksPackageInstallLaunchAndUninstallGuiPayloadWithoutOriginalPackage()
  {
    using var project = new TestProject();
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
          ["builder"] = "${builder}", ["installer"] = "${installer}", ["config"] = "${config}", ["output"] = "${package}"
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
        ["installer"] = new { config, output = Path.Combine(project.Root, "packages") },
        ["doinstall"] = new { builder, installer = engine }
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

using System.Text.Json;

namespace DoTask.Tests;

public sealed class RustInstallerWorkflowTests
{
  [Theory]
  [InlineData(false, false)]
  [InlineData(true, false)]
  [InlineData(false, true)]
  [InlineData(true, true)]
  public async Task DeclarativeAssemblyUsesSeparateToolsWithoutLanguageSdks( bool explicitBuilder, bool selfExtracting )
  {
    using var project = new TestProject();
    var engine = RustArtifacts.Binary("doinstall");
    var builder = RustArtifacts.Binary("doinstall-builder");
    var taskName = selfExtracting ? "assemble-self-extracting.task" : "assemble.task";
    using var resource = typeof(RustInstallerWorkflowTests).Assembly.GetManifestResourceStream("Shared/doinstall/" + taskName)!;
    using var reader = new StreamReader(resource);
    project.Write(".tasks/_/doinstall/" + taskName, await reader.ReadToEndAsync());
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
    var resultFile = Path.Combine(project.Root, "result 日本語.json");
    var arguments = new List<string> {
      "_/doinstall/" + taskName, "--installer", engine, "--config", Path.GetRelativePath(project.Root, config),
      "--output", Path.GetRelativePath(project.Root, output), "--result-file", Path.GetRelativePath(project.Root, resultFile)
    };
    if (selfExtracting) {
      arguments.AddRange(["--launcher-template", RustArtifacts.Binary("doinstall-launcher")]);
    }
    if (explicitBuilder) {
      arguments.AddRange(["--builder", builder]);
    }
    // Only the native CLI is copied. PATH contains the prebuilt tools, no Cargo
    // or .NET, and invalid SDK overrides expose accidental code-task execution.
    async Task<ProcessResult> Run( IReadOnlyList<string> args ) => await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = native,
      Arguments = args,
      WorkingDirectory = nested,
      CaptureOutput = true,
      ThrowOnError = false,
      Environment = new Dictionary<string, string?> {
        // macOS packaging invokes its built-in codesign, never a language SDK.
        ["PATH"] = Path.GetDirectoryName(builder) + (OperatingSystem.IsMacOS() ? ":/usr/bin:/bin" : ""),
        ["DOTNET_HOST_PATH"] = "not-a-host",
        ["DOTASK_SOURCE_RUST_SDK"] = "not-an-sdk"
      }
    }, CancellationToken.None);
    var help = await Run(["help", "_/doinstall/" + taskName]);
    Assert.Equal(0, help.ExitCode);
    Assert.Contains("doinstall-builder", help.StandardOutput);
    Assert.False(Directory.Exists(output));
    var assembled = await Run(arguments);
    Assert.True(assembled.ExitCode == 0, assembled.StandardOutput + assembled.StandardError);
    using var result = JsonDocument.Parse(File.ReadAllText(resultFile));
    var extension = OperatingSystem.IsWindows() ? ".exe" : "";
    var runnable = result.RootElement.GetProperty("FilePath").GetString()!;
    Assert.Equal("adapter-probe-setup-v1.0.0" + (selfExtracting ? "-se" : "") + extension, Path.GetFileName(runnable));
    Assert.True(File.Exists(runnable));
    var extracted = Path.Combine(project.Root, "retained 日本語");
    var validated = await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = runnable,
      Arguments = selfExtracting ? ["--extract-to", extracted, "--", "--validate"] : ["--validate"],
      WorkingDirectory = nested,
      CaptureOutput = true,
      ThrowOnError = false
    }, CancellationToken.None);
    Assert.True(validated.ExitCode == 0, validated.StandardOutput + validated.StandardError);
    var sources = selfExtracting ? extracted : output;
    Assert.Equal("A consumer-owned payload.", File.ReadAllText(Path.Combine(sources, "payload", "hello.txt")));
    Assert.True(File.Exists(Path.Combine(sources, "adapter-probe-setup-v1.0.0.yaml")));
    if (selfExtracting) {
      Assert.Single(Directory.EnumerateFileSystemEntries(output));
      var invalidTemplate = arguments.ToArray();
      invalidTemplate[Array.IndexOf(invalidTemplate, "--output") + 1] = output + "-invalid";
      invalidTemplate[Array.IndexOf(invalidTemplate, "--launcher-template") + 1] = engine;
      Assert.NotEqual(0, (await Run(invalidTemplate)).ExitCode);
      Assert.False(Directory.Exists(output + "-invalid"));
    }
    Assert.False(File.Exists(Path.Combine(output, Path.GetFileName(builder))));
    var before = File.ReadAllBytes(resultFile);
    Assert.NotEqual(0, (await Run(arguments)).ExitCode);
    Assert.Equal(before, File.ReadAllBytes(resultFile));
    var missingArguments = arguments.ToList();
    if (explicitBuilder) {
      missingArguments[missingArguments.IndexOf("--builder") + 1] = "missing-doinstall-builder";
    } else {
      missingArguments.AddRange(["--builder", "missing-doinstall-builder"]);
    }
    missingArguments[missingArguments.IndexOf("--output") + 1] = output + "-missing";
    var missing = await Run(missingArguments);
    Assert.NotEqual(0, missing.ExitCode);
    Assert.Contains("missing-doinstall-builder", missing.StandardError);
    Assert.False(Directory.Exists(output + "-missing"));
    Assert.False(Directory.Exists(Path.Combine(project.Root, "standalone", "sdk")));
  }

  [Theory]
  [InlineData(false, false)]
  [InlineData(true, false)]
  [InlineData(false, true)]
  [InlineData(true, true)]
  public async Task SharedTasksPackageInstallLaunchAndUninstallGuiPayloadWithoutOriginalPackage( bool selfExtracting, bool discoverTools )
  {
    using var project = new TestProject();
    var engine = RustArtifacts.Binary("doinstall");
    var builder = RustArtifacts.Binary("doinstall-builder");
    Assert.True(File.Exists(engine) && File.Exists(builder), "Supply prebuilt doinstall tools through --doinstall-tools or DOINSTALL_TOOL_DIR.");
    async Task<ProcessResult> RunTask( params string[] arguments )
    {
      using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(90));
      return await ProcessRunner.RunAsync(new ProcessDefinition {
        Executable = RustArtifacts.Binary("dotask"),
        Arguments = arguments,
        WorkingDirectory = project.Root,
        CaptureOutput = true,
        ThrowOnError = false,
        // Only this fixture's child receives the PATH entry. Discovery cases
        // have no doinstall settings, matching the installed-builder workflow.
        Environment = new Dictionary<string, string?> {
          ["PATH"] = Path.GetDirectoryName(builder) + Path.PathSeparator + Environment.GetEnvironmentVariable("PATH")
        }
      }, timeout.Token);
    }
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
    sourceConfig["packaging"] = JsonSerializer.SerializeToNode(new Dictionary<string, string> {
      ["installer-name"] = "custom-gui-v${version}${exe-extension}",
      ["launcher-name"] = "custom-gui-v${version}-se${exe-extension}"
    });
    sourceConfig["build-steps"] = JsonSerializer.SerializeToNode(new object[] {
      new { gather = new[] { new { from = payload, files = Directory.EnumerateFiles(payload).Select(Path.GetFileName).ToArray() } } },
      new {
        run = selfExtracting ? "_/doinstall/assemble-self-extracting" : "_/doinstall/assemble",
        with = new Dictionary<string, object> {
          ["builder"] = "${builder}", ["installer"] = "${installer}", ["config"] = "${config}", ["output"] = "${package}",
          ["result-file"] = "${result-file}"
        }
      }
    });
    if (selfExtracting) {
      sourceConfig["build-steps"]![1]!["with"]!["launcher-template"] = "${launcher-template}";
    }
    var config = project.Write("installer.yaml", sourceConfig.ToJsonString());
    project.Write(".tasks/package-gui.rs", """
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
        ["installer"] = new {
          config,
          output = Path.Combine(project.Root, "packages"),
          installer_file = "packages/${os}-${arch}/gui-probe-*/custom-gui-v${version}" + (selfExtracting ? "-se" : "") + "${exe-extension}"
        },
        ["doinstall"] = discoverTools ? new Dictionary<string, object>() : new Dictionary<string, object> { ["builder"] = builder, ["installer"] = engine, ["launcher-template"] = RustArtifacts.Binary("doinstall-launcher") }
      }
    }));
    var packageResult = await RunTask("package-gui");
    Assert.True(packageResult.ExitCode == 0, packageResult.StandardOutput + packageResult.StandardError);
    var recordFile = Path.Combine(project.Root, "artifacts", ".dotask", "installers",
      project.Context().OS.ToString().ToLowerInvariant() + "-" + project.Context().Architecture.ToString().ToLowerInvariant() + ".json");
    var recorded = File.ReadAllBytes(recordFile);
    Assert.Contains("custom-gui-v1.0.0" + (selfExtracting ? "-se" : ""), File.ReadAllText(recordFile));
    var sources = Path.Combine(project.Root, "package sources");
    var installArguments = new List<string> { "install" };
    if (selfExtracting) {
      installArguments.AddRange(["--installer-args", JsonSerializer.Serialize(new[] { "--extract-to", sources, "--", "--non-interactive" })]);
    }
    // Launch the recorded custom artifact through the generic installer task.
    // Its settings template independently exercises versioned launcher selection.
    var installation = await RunTask(installArguments.ToArray());
    Assert.True(installation.ExitCode == 0, installation.StandardOutput + installation.StandardError);
    var packagedConfig = Assert.Single(Directory.EnumerateFiles(selfExtracting ? sources : Path.Combine(project.Root, "packages"), "custom-gui-v1.0.0.yaml", SearchOption.AllDirectories));
    var shippedConfig = File.ReadAllText(packagedConfig);
    Assert.Contains("runtime:", shippedConfig);
    Assert.DoesNotContain("build-steps:", shippedConfig);
    Assert.DoesNotContain("packaging:", shippedConfig);
    var launcher = Path.Combine(installed, OperatingSystem.IsWindows() ? "gui-probe.exe" : "gui-probe");
    var launched = await Run(launcher, marker);
    Assert.True(launched.ExitCode == 0, launched.StandardError);
    Assert.Equal("GUI payload launched", File.ReadAllText(marker));
    var extension = OperatingSystem.IsWindows() ? ".lnk" : OperatingSystem.IsMacOS() ? ".command" : ".desktop";
    Assert.True(File.Exists(Path.Combine(installed, "GUI Probe" + extension)));
    Assert.True(File.Exists(Path.Combine(desktop, "GUI Probe" + extension)));
    // A failure after successful assembly must preserve the prior successful
    // build record; returning or recording a guessed filename would mask it.
    var assemblyStep = sourceConfig["build-steps"]![1]!.DeepClone();
    sourceConfig["application"]!["version"] = "2.0.0";
    sourceConfig["build-steps"]!.AsArray().Add(JsonSerializer.SerializeToNode(new { run = "reject-package" }));
    project.Write(".tasks/reject-package.task", "steps:\n  - exec: {executable: missing-packaging-tool}\n");
    File.WriteAllText(config, sourceConfig.ToJsonString());
    var failedPackage = await RunTask("package-gui");
    Assert.NotEqual(0, failedPackage.ExitCode);
    Assert.Equal(recorded, File.ReadAllBytes(recordFile));
    sourceConfig["application"]!["version"] = "3.0.0";
    while (sourceConfig["build-steps"]!.AsArray().Count > 1) {
      sourceConfig["build-steps"]!.AsArray().RemoveAt(1);
    }
    File.WriteAllText(config, sourceConfig.ToJsonString());
    var missingResult = await RunTask("package-gui");
    Assert.NotEqual(0, missingResult.ExitCode);
    Assert.Contains("Assembly must write '${result-file}'", missingResult.StandardError);
    Assert.Equal(recorded, File.ReadAllBytes(recordFile));
    // Omitted optional runtime lists must stay absent, not turn into null YAML
    // while the adapter expands producer executable names.
    sourceConfig["application"]!["version"] = "4.0.0";
    sourceConfig["runtime"]!.AsObject().Remove("shortcuts");
    sourceConfig["build-steps"]!.AsArray().Add(assemblyStep);
    File.WriteAllText(config, sourceConfig.ToJsonString());
    var withoutShortcuts = await RunTask("package-gui");
    Assert.True(withoutShortcuts.ExitCode == 0, withoutShortcuts.StandardOutput + withoutShortcuts.StandardError);
    Assert.Contains("custom-gui-v4.0.0" + (selfExtracting ? "-se" : ""), File.ReadAllText(recordFile));
    Directory.Delete(Path.Combine(project.Root, "packages"), recursive: true);
    File.Delete(config);
    var removed = await RunTask("uninstall", "--install-dir", installed);
    Assert.True(removed.ExitCode == 0, removed.StandardOutput + removed.StandardError);
    Assert.False(Directory.Exists(installed));
    Assert.False(File.Exists(Path.Combine(desktop, "GUI Probe" + extension)));
  }
}

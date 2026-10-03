#:include _support/RustBuild.cs
using System.Runtime.InteropServices;
using System.Text.Json;
using DoTask;

/// <summary>Create dotask's standalone installer for the current OS and architecture without installing it.</summary>
/// <option name="configuration" alias="c" choices="Debug,Release" default="Release">Build configuration.</option>
/// <option name="self-contained" type="bool" default="true">Include the .NET runtime in the application payload.</option>
/// <requires tool="dotnet" />
/// <requires setting="installer-output" />
/// <example>dotask create-installer</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    if (string.IsNullOrWhiteSpace(project.Config.Get<string>("installer-output"))) {
      throw new TaskException("settings.installer-output must name a directory.");
    }
    var outputDirectory = project.Config.GetPath("installer-output");
    await project.ExecTargetAsync("installer-engine");
    var system = project.OS switch {
      HostOS.Windows => "win",
      HostOS.Linux => RuntimeInformation.RuntimeIdentifier.StartsWith("linux-musl-", StringComparison.Ordinal) ? "linux-musl" : "linux",
      HostOS.MacOS => "osx",
      _ => throw new TaskException("Unsupported installer platform.")
    };
    var architecture = project.Architecture switch {
      Architecture.X64 => "x64",
      Architecture.Arm64 => "arm64",
      _ => throw new TaskException("The dotask installer supports x64 and ARM64 hosts.")
    };
    var host = "dotnet";
    string[] properties = [
      "-p:Configuration=" + project.Parameters.Get<string>("configuration"),
      "-p:RuntimeIdentifier=" + system + "-" + architecture,
      "-p:SelfContained=" + project.Parameters.Get<bool>("self-contained").ToString().ToLowerInvariant(),
      "-p:UseAppHost=true"
    ];
    var application = project.Path("src/Dotask.Cli/Dotask.Cli.csproj");
    var published = await Query(["publish", application, "--nologo", "--verbosity", "quiet", .. properties,
      "-getProperty:PublishDir,Version"]);
    var applicationDirectory = Path.GetFullPath(published["PublishDir"], Path.GetDirectoryName(application)!);
    var temporary = Directory.CreateTempSubdirectory("dotask-package-config-");
    try {
      var config = Path.Combine(temporary.FullName, "installer.yaml");
      // JSON is a YAML subset. The Rust packager writes the final human-readable
      // YAML, keeping this wrapper free of a second YAML serialization dependency.
      await File.WriteAllTextAsync(config, JsonSerializer.Serialize(new {
        schema = 1,
        application = new { id = "dotask", name = "dotask", version = published["Version"], author = "Kody Brown", description = "Portable project tasks" },
        platform = project.OS.ToString().ToLowerInvariant(),
        architecture,
        payload = applicationDirectory,
        commands = new[] { new { name = "dotask", executable = project.IsWindows ? "dotask.exe" : "dotask" } }
      }), project.CancellationToken);
      var engine = Path.Combine(await RustBuild.TargetDirectory(project), "release", project.IsWindows ? "dotask-installer.exe" : "dotask-installer");
      var artifact = await project.CreateInstallerAsync("_/dotask-installer/create-installer", new { Config = config, Engine = engine, Output = outputDirectory });
      await project.SetInstallerResultAsync(artifact);
    } finally {
      temporary.Delete(recursive: true);
    }

    async Task<Dictionary<string, string>> Query( string[] arguments )
    {
      var result = await project.RunAsync(new ProcessDefinition {
        Executable = host,
        Arguments = arguments,
        CaptureOutput = true,
        ThrowOnError = false
      });
      if (result.ExitCode != 0) {
        Console.Error.WriteLine(result.StandardOutput.Trim());
        Console.Error.WriteLine(result.StandardError.Trim());
        throw new ProcessFailedException(host, result.ExitCode);
      }
      // MSBuild prints evaluated properties as one JSON object. Warnings may
      // precede it; keep those visible without interpreting them as metadata.
      var start = result.StandardOutput.IndexOf("{\n", StringComparison.Ordinal);
      if (start < 0) {
        start = result.StandardOutput.IndexOf("{\r\n", StringComparison.Ordinal);
      }
      if (start < 0) {
        throw new TaskException("The .NET SDK did not return evaluated publish metadata.");
      }
      var notices = result.StandardOutput[..start].Trim();
      if (notices.Length > 0) {
        Console.WriteLine(notices);
      }
      if (!string.IsNullOrWhiteSpace(result.StandardError)) {
        Console.Error.WriteLine(result.StandardError.Trim());
      }
      using var json = JsonDocument.Parse(result.StandardOutput[start..]);
      return json.RootElement.GetProperty("Properties").EnumerateObject()
        .ToDictionary(p => p.Name, p => p.Value.GetString() ?? "");
    }
  }

}

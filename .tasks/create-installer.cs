using System.Runtime.InteropServices;
using System.Text.Json;
using DoTask;

/// <summary>Create dotask's standalone installer for the current OS and architecture without installing it.</summary>
/// <option name="configuration" alias="c" choices="Debug,Release" default="Release">Build configuration.</option>
/// <option name="self-contained" type="bool" default="true">Include the .NET runtime in the application and installer.</option>
/// <requires tool="dotnet" />
/// <example>dotask create-installer</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
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
    var installerProject = project.Path("src/Dotask.Installer/Dotask.Installer.csproj");
    var published = await Query(["publish", application, "--nologo", "--verbosity", "quiet", .. properties,
      "-getProperty:PublishDir,Version"]);
    var applicationDirectory = Path.GetFullPath(published["PublishDir"], Path.GetDirectoryName(application)!);
    var installer = await Query(["publish", installerProject, "--nologo", "--verbosity", "quiet", .. properties,
      "-getProperty:PublishDir,AssemblyName"]);
    var installerDirectory = Path.GetFullPath(installer["PublishDir"], Path.GetDirectoryName(installerProject)!);
    // A physical snapshot survives the next publish and can be distributed as a directory.
    var package = Path.Combine(Path.GetDirectoryName(Path.TrimEndingDirectorySeparator(installerDirectory))!,
      "installers", system + "-" + architecture, Guid.NewGuid().ToString("N"));
    CopyTree(installerDirectory, package);
    CopyTree(applicationDirectory, Path.Combine(package, "payload"));
    var definition = new InstallationDefinition {
      AppId = "dotask",
      Version = published["Version"],
      SourceDirectory = "payload",
      Commands = [new InstalledCommand("dotask", project.IsWindows ? "dotask.exe" : "dotask")]
    };
    await File.WriteAllTextAsync(Path.Combine(package, "installer.json"), JsonSerializer.Serialize(definition), project.CancellationToken);
    var executable = Path.Combine(package, project.IsWindows ? "dotask-installer.exe" : "dotask-installer");
    await project.SetInstallerResultAsync(new InstallerArtifact {
      FilePath = executable,
      Kind = InstallerKind.Executable,
      OS = project.OS,
      Architecture = project.Architecture
    });
    Console.WriteLine($"Created installer: {executable}");
    Console.WriteLine("Distribute the entire installer directory, including payload and runtime files.");

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

  private static void CopyTree( string source, string destination )
  {
    Directory.CreateDirectory(destination);
    foreach (var entry in Directory.EnumerateFileSystemEntries(source)) {
      if ((File.GetAttributes(entry) & FileAttributes.ReparsePoint) != 0) {
        throw new TaskException($"Installer payload cannot contain links: {entry}");
      }
      var target = Path.Combine(destination, Path.GetFileName(entry));
      if (Directory.Exists(entry)) {
        CopyTree(entry, target);
      } else {
        File.Copy(entry, target);
        if (!OperatingSystem.IsWindows()) {
          File.SetUnixFileMode(target, File.GetUnixFileMode(entry));
        }
      }
    }
  }
}

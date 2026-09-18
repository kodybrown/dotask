using System.Runtime.InteropServices;
using System.Text.Json;
using DoTask;

/// <summary>Publish and install the configured application for the current user.</summary>
/// <option name="configuration" alias="c" choices="Debug,Release" default="Release">Build configuration.</option>
/// <option name="self-contained" type="bool" default="true">Include the .NET runtime in the installed application.</option>
/// <option name="framework" type="string">Target framework for projects that declare multiple frameworks.</option>
/// <option name="dotnet" default="dotnet">.NET CLI executable name or path.</option>
/// <option name="bin-dir" type="path" completion="directory">Command directory; overrides BIN and the platform default.</option>
/// <option name="install-root" type="path" completion="directory">Parent directory for installed applications; defaults to the user programs directory.</option>
/// <option name="app-id" type="string">Stable installation ID; defaults to the assembly name.</option>
/// <option name="command" type="string">Command name; defaults to ToolCommandName or the assembly name.</option>
/// <requires tool="dotnet" />
/// <requires setting="project" />
/// <example>dotask install</example>
/// <example>dotask install --bin-dir ./artifacts/install-test/bin --install-root ./artifacts/install-test/apps</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var definition = await PublishAsync(project);
    var result = await project.InstallAsync(definition);
    Console.WriteLine($"{(result.Reused ? "Activated existing" : "Installed")} {definition.AppId} {definition.Version}");
    Console.WriteLine($"  Files: {result.InstallDirectory}");
    Console.WriteLine($"  Commands: {result.BinDirectory}");
    foreach (var warning in result.Warnings) {
      Console.WriteLine($"  Note: {warning}");
    }
  }

  private static async Task<InstallationDefinition> PublishAsync( BuildContext project )
  {
    var parameters = project.Parameters;
    var projectFile = project.Config.GetPath("project");
    if (!File.Exists(projectFile)) {
      throw new TaskException($"Project file does not exist: {projectFile}");
    }
    var system = project.OS switch {
      HostOS.Windows => "win",
      HostOS.Linux => RuntimeInformation.RuntimeIdentifier.StartsWith("linux-musl-", StringComparison.Ordinal) ? "linux-musl" : "linux",
      HostOS.MacOS => "osx",
      _ => throw new TaskException("Installation supports Windows, Linux, and macOS.")
    };
    var architecture = RuntimeInformation.OSArchitecture switch {
      Architecture.X64 => "x64",
      Architecture.Arm64 => "arm64",
      _ => throw new TaskException("The .NET installation task supports x64 and ARM64 hosts.")
    };
    var properties = new List<string>
    {
      "-p:Configuration=" + parameters.Get<string>("configuration"),
      "-p:RuntimeIdentifier=" + system + "-" + architecture,
      "-p:SelfContained=" + parameters.Get<bool>("self-contained").ToString().ToLowerInvariant(),
      "-p:UseAppHost=true"
    };
    if (parameters.Contains("framework")) {
      properties.Add("-p:TargetFramework=" + parameters.Get<string>("framework"));
    }
    var host = parameters.Get<string>("dotnet");
    var info = await Query(["msbuild", projectFile, "--nologo", .. properties,
      "-getProperty:TargetFramework,TargetFrameworks,OutputType"]);
    if (string.IsNullOrWhiteSpace(info["TargetFramework"])) {
      throw new TaskException("Select a single target framework with --framework before installing this project.");
    }
    if (info["OutputType"] != "Exe") {
      throw new TaskException("The installation task currently supports console applications (OutputType Exe).");
    }
    Console.WriteLine($"Publishing {Path.GetFileName(projectFile)} for {system}-{architecture}...");
    // Query after Publish, using the same properties, so custom output policies
    // and target-time PublishDir changes are respected. Never assume bin/obj.
    var published = await Query(["publish", projectFile, "--nologo", "--verbosity", "quiet", .. properties,
      "-getProperty:PublishDir,AssemblyName,Version,ToolCommandName"]);
    var directory = Path.GetFullPath(published["PublishDir"], Path.GetDirectoryName(projectFile)!);
    return new InstallationDefinition {
      AppId = parameters.Get("app-id", published["AssemblyName"]),
      Version = published["Version"],
      SourceDirectory = directory,
      Commands = [new InstalledCommand(parameters.Get("command", string.IsNullOrWhiteSpace(published["ToolCommandName"])
        ? published["AssemblyName"] : published["ToolCommandName"]), published["AssemblyName"] + (project.IsWindows ? ".exe" : ""))],
      BinDirectory = parameters.Contains("bin-dir") ? parameters.GetPath("bin-dir") : null,
      InstallRoot = parameters.Contains("install-root") ? parameters.GetPath("install-root") : null
    };

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

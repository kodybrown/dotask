using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace DoTask;

public enum InstallerKind { Executable, Msi, ShellScript, DotNetAssembly }

/// <summary>A project-built installer, not an application publish directory. Creating it must not install the application.</summary>
public sealed record InstallerArtifact
{
  public required string FilePath { get; init; }
  public required InstallerKind Kind { get; init; }
  public required HostOS OS { get; init; }
  public required Architecture Architecture { get; init; }
  public IReadOnlyList<string> DefaultArguments { get; init; } = [];
}

/// <summary>Validates and launches installer artifacts without shell interpretation or implicit elevation.</summary>
public static class InstallerRunner
{
  public static void Validate( InstallerArtifact artifact )
  {
    ArgumentNullException.ThrowIfNull(artifact);
    if (!Enum.IsDefined(artifact.Kind) || !Enum.IsDefined(artifact.OS) || artifact.OS == HostOS.Unknown ||
        !Enum.IsDefined(artifact.Architecture)) {
      throw new TaskException("Invalid installer kind or target platform.");
    }
    var host = OperatingSystem.IsWindows() ? HostOS.Windows : OperatingSystem.IsLinux() ? HostOS.Linux :
      OperatingSystem.IsMacOS() ? HostOS.MacOS : HostOS.Unknown;
    if (artifact.OS != host || artifact.Architecture != RuntimeInformation.OSArchitecture) {
      throw new TaskException($"Installer targets {artifact.OS}/{artifact.Architecture}; this host is {host}/{RuntimeInformation.OSArchitecture}.");
    }
    if (string.IsNullOrWhiteSpace(artifact.FilePath) || !Path.IsPathFullyQualified(artifact.FilePath) || !File.Exists(artifact.FilePath)) {
      throw new TaskException($"Installer artifact must be an existing absolute file: {artifact.FilePath}");
    }
    if (artifact.Kind == InstallerKind.Executable && OperatingSystem.IsWindows() &&
        !artifact.FilePath.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)) {
      throw new TaskException("Windows executable installers must be .exe files; shell associations and command scripts are not supported.");
    }
    if ((artifact.Kind == InstallerKind.Msi && (!OperatingSystem.IsWindows() || !artifact.FilePath.EndsWith(".msi", StringComparison.OrdinalIgnoreCase))) ||
        (artifact.Kind == InstallerKind.ShellScript && OperatingSystem.IsWindows())) {
      throw new TaskException("Installer kind is incompatible with this platform or artifact.");
    }
    ValidateArguments(artifact.DefaultArguments);
  }

  /// <summary>Decode a JSON array to preserve exact argument boundaries, including empty strings.</summary>
  public static IReadOnlyList<string> ParseArguments( string json )
  {
    try {
      var arguments = JsonSerializer.Deserialize<string[]>(json);
      ValidateArguments(arguments);
      return arguments!;
    } catch (JsonException) {
      throw new TaskException("installer-args must be a JSON array of strings, for example [\"--scope\",\"user\"].");
    }
  }

  public static async Task<ProcessResult> RunAsync( InstallerArtifact artifact, IReadOnlyList<string>? arguments = null,
    CancellationToken cancellationToken = default )
  {
    Validate(artifact);
    var options = arguments ?? artifact.DefaultArguments;
    ValidateArguments(options);
    var (executable, prefix) = artifact.Kind switch {
      InstallerKind.Executable => (artifact.FilePath, Array.Empty<string>()),
      InstallerKind.Msi => (Path.Combine(Environment.SystemDirectory, "msiexec.exe"), new[] { "/i", artifact.FilePath }),
      InstallerKind.ShellScript => ("/bin/sh", new[] { artifact.FilePath }),
      InstallerKind.DotNetAssembly => ("dotnet", new[] { artifact.FilePath }),
      _ => throw new TaskException("Unsupported installer kind.")
    };
    var definition = new ProcessDefinition {
      Executable = executable,
      Arguments = [.. prefix, .. options],
      WorkingDirectory = Path.GetDirectoryName(artifact.FilePath),
      ThrowOnError = false
    };
    // Shell activation honors an EXE's own elevation manifest. Never force runas.
    var result = OperatingSystem.IsWindows() && artifact.Kind == InstallerKind.Executable
      ? await RunWindowsExecutableAsync(definition, cancellationToken)
      : await ProcessRunner.RunAsync(definition, cancellationToken);
    // MSI distinguishes successful installation requiring (or initiating) a reboot.
    if (artifact.Kind == InstallerKind.Msi && result.ExitCode is 1641 or 3010) {
      Console.WriteLine($"Installer succeeded; a restart is required (exit code {result.ExitCode}).");
    } else if (result.ExitCode != 0) {
      throw new ProcessFailedException(executable, result.ExitCode);
    }
    return result;
  }

  private static async Task<ProcessResult> RunWindowsExecutableAsync( ProcessDefinition definition, CancellationToken cancellationToken )
  {
    cancellationToken.ThrowIfCancellationRequested();
    var start = new ProcessStartInfo(definition.Executable) {
      UseShellExecute = true,
      WorkingDirectory = definition.WorkingDirectory
    };
    foreach (var argument in definition.Arguments) {
      start.ArgumentList.Add(argument);
    }

    Process? process;
    try {
      process = Process.Start(start);
    } catch (Win32Exception ex) {
      throw new TaskException($"Cannot start installer '{definition.Executable}': {ex.Message}");
    }
    using (process) {
      if (process is null) {
        throw new TaskException("Installer launched, but no process handle was returned; completion cannot be observed.");
      }

      try {
        await process.WaitForExitAsync(cancellationToken);
      } catch (OperationCanceledException) {
        try { process.Kill(entireProcessTree: true); } catch (InvalidOperationException) { } catch (Win32Exception) {
          Console.Error.WriteLine("The installer could not be stopped. It may still be running with elevated permissions.");
        }
        throw;
      }
      return new ProcessResult(process.ExitCode, "", "");
    }
  }

  private static void ValidateArguments( IReadOnlyList<string>? arguments )
  {
    if (arguments is null || arguments.Any(a => a is null || a.Contains('\0'))) {
      throw new TaskException("Installer arguments must be strings without NUL characters.");
    }
  }
}

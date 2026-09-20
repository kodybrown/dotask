using System.Runtime.InteropServices;
using System.Text.Json;
using DoTask.Runtime;

namespace DoTask;

public enum HostOS { Windows, Linux, MacOS, Unknown }

/// <summary>The immutable project and target context for this process.</summary>
public sealed class BuildContext
{
  private static readonly Lazy<BuildContext> Ambient = new(Load);
  private readonly ExecutionContextData _data;
  private readonly CancellationTokenSource _cancellation = new();

  public static BuildContext Current => Ambient.Value;
  public string RootDirectory => _data.RootDirectory;
  public string InvocationDirectory => _data.InvocationDirectory;
  public string WorkingDirectory => RootDirectory;
  public string TaskDirectory => _data.TaskDirectory;
  public string TargetFile => _data.TargetFile;
  public string TargetDirectory => System.IO.Path.GetDirectoryName(TargetFile)!;
  public string TargetName => _data.TargetName;
  public HostOS OS { get; } = OperatingSystem.IsWindows() ? HostOS.Windows :
    OperatingSystem.IsLinux() ? HostOS.Linux : OperatingSystem.IsMacOS() ? HostOS.MacOS : HostOS.Unknown;
  public Architecture Architecture => RuntimeInformation.OSArchitecture;
  public bool IsWindows => OS == HostOS.Windows;
  public bool IsLinux => OS == HostOS.Linux;
  public bool IsMacOS => OS == HostOS.MacOS;
  public char DirSeparator => System.IO.Path.DirectorySeparatorChar;
  public IReadOnlyList<char> InvalidFileChars => System.IO.Path.GetInvalidFileNameChars();
  public IReadOnlyList<char> InvalidDirChars => System.IO.Path.GetInvalidFileNameChars();
  public Values Config { get; }
  public Values Parameters { get; }
  public FileSystemHelpers Files { get; }
  public CancellationToken CancellationToken => _cancellation.Token;

  internal BuildContext( ExecutionContextData data )
  {
    _data = data;
    Config = new Values(data.Settings, RootDirectory);
    Parameters = new Values(data.Parameters, RootDirectory);
    Files = new FileSystemHelpers(RootDirectory);
  }

  public string Path( params string[] parts ) => PortablePath.Resolve(RootDirectory, parts);

  /// <summary>Install published files for the current user; relative input directories start at the project root.</summary>
  public async Task<InstallationResult> InstallAsync( InstallationDefinition definition, CancellationToken cancellationToken = default )
  {
    using var linked = CancellationTokenSource.CreateLinkedTokenSource(CancellationToken, cancellationToken);
    return await UserInstaller.InstallAsync(definition with {
      SourceDirectory = Path(definition.SourceDirectory),
      BinDirectory = definition.BinDirectory is null ? null : Path(definition.BinDirectory),
      InstallRoot = definition.InstallRoot is null ? null : Path(definition.InstallRoot)
    }, linked.Token);
  }

  /// <summary>Build a fresh installer using the project's target and read its invocation-scoped result.</summary>
  public async Task<InstallerArtifact> CreateInstallerAsync( string target = "create-installer", object? parameters = null,
    CancellationToken cancellationToken = default )
  {
    if (!await TargetExistsAsync(target, cancellationToken)) {
      throw new TaskException($"Installation requires a '{target}' target. Create one that builds an installer and calls SetInstallerResultAsync.");
    }
    return (await InvokeTargetAsync(target, parameters, TargetCallOperation.CreateInstaller, cancellationToken))!.Installer!;
  }

  /// <summary>Return one installer artifact to the caller; relative paths start at the project root.</summary>
  public async Task SetInstallerResultAsync( InstallerArtifact artifact, CancellationToken cancellationToken = default )
  {
    using var linked = CancellationTokenSource.CreateLinkedTokenSource(CancellationToken, cancellationToken);
    artifact = artifact with { FilePath = Path(artifact.FilePath) };
    InstallerRunner.Validate(artifact);
    await ContextFile.WriteToAsync(System.IO.Path.Combine(_data.SessionDirectory, "installer-result.json"), artifact, linked.Token);
  }

  /// <summary>Run an installer with its defaults, or replace those defaults with explicit argument tokens.</summary>
  public async Task<ProcessResult> RunInstallerAsync( InstallerArtifact artifact, IReadOnlyList<string>? arguments = null,
    CancellationToken cancellationToken = default )
  {
    using var linked = CancellationTokenSource.CreateLinkedTokenSource(CancellationToken, cancellationToken);
    return await InstallerRunner.RunAsync(artifact with { FilePath = Path(artifact.FilePath) }, arguments, linked.Token);
  }

  public Task<ProcessResult> RunAsync( string executable, IReadOnlyList<string> arguments,
    CancellationToken cancellationToken = default ) => RunAsync(new ProcessDefinition {
      Executable = executable,
      Arguments = arguments
    }, cancellationToken);

  public async Task<ProcessResult> RunAsync( ProcessDefinition definition, CancellationToken cancellationToken = default )
  {
    using var linked = CancellationTokenSource.CreateLinkedTokenSource(CancellationToken, cancellationToken);
    var workingDirectory = definition.WorkingDirectory is null ? RootDirectory : Path(definition.WorkingDirectory);
    var executable = definition.Executable.IndexOfAny(['/', '\\']) >= 0
      ? PortablePath.Resolve(workingDirectory, definition.Executable) : definition.Executable;
    return await ProcessRunner.RunAsync(definition with {
      Executable = executable,
      WorkingDirectory = workingDirectory
    }, linked.Token);
  }

  public async Task ExecTargetAsync( string target, object? parameters = null, CancellationToken cancellationToken = default )
    => await InvokeTargetAsync(target, parameters, TargetCallOperation.Execute, cancellationToken);

  /// <summary>Check for a target using current full-name/alias resolution, without compiling or running it.</summary>
  public async Task<bool> TargetExistsAsync( string target, CancellationToken cancellationToken = default )
    => (await InvokeTargetAsync(target, null, TargetCallOperation.Exists, cancellationToken))!.Exists;

  /// <summary>Run a target if present, returning absence and target failures as distinct results.</summary>
  public async Task<TargetExecutionResult> ExecTargetIfExistsAsync( string target, object? parameters = null,
    CancellationToken cancellationToken = default )
  {
    var reply = (await InvokeTargetAsync(target, parameters, TargetCallOperation.ExecuteIfExists, cancellationToken))!;
    var status = !reply.Exists ? TargetExecutionStatus.NotFound
      : reply.ExitCode == 0 ? TargetExecutionStatus.Succeeded : TargetExecutionStatus.Failed;
    return new(status, reply.ExitCode, reply.Error);
  }

  private async Task<TargetCallReply?> InvokeTargetAsync( string target, object? parameters,
    TargetCallOperation operation, CancellationToken cancellationToken )
  {
    ArgumentException.ThrowIfNullOrWhiteSpace(target);
    using var linked = CancellationTokenSource.CreateLinkedTokenSource(CancellationToken, cancellationToken);
    linked.Token.ThrowIfCancellationRequested();
    var callDirectory = System.IO.Path.Combine(_data.SessionDirectory, Guid.NewGuid().ToString("N"));
    try {
      var call = new TargetCall(_data with { SessionDirectory = callDirectory }, target,
        JsonSerializer.SerializeToElement(parameters ?? new { }), operation);
      var file = await ContextFile.WriteAsync(callDirectory, call, linked.Token);
      var replyFile = file + ".result";
      await ProcessRunner.RunAsync(new ProcessDefinition {
        Executable = _data.DotnetExecutable,
        Arguments = [_data.CliAssembly, "__exec", file],
        WorkingDirectory = RootDirectory
      }, linked.Token);
      if (operation == TargetCallOperation.CreateInstaller) {
        var resultPath = System.IO.Path.Combine(callDirectory, "installer-result.json");
        if (!File.Exists(resultPath)) {
          throw new TaskException($"Target '{target}' did not return an installer. Call SetInstallerResultAsync after creating it.");
        }
        var artifact = JsonSerializer.Deserialize<InstallerArtifact>(await File.ReadAllTextAsync(resultPath, linked.Token))
          ?? throw new TaskException("Invalid installer result.");
        InstallerRunner.Validate(artifact);
        return new TargetCallReply(true, Installer: artifact);
      }
      if (operation == TargetCallOperation.Execute) {
        return null;
      }
      await using var stream = File.OpenRead(replyFile);
      return await JsonSerializer.DeserializeAsync<TargetCallReply>(stream, cancellationToken: linked.Token)
        ?? throw new TaskException("Invalid target-call response.");
    } finally {
      if (Directory.Exists(callDirectory)) {
        Directory.Delete(callDirectory, recursive: true);
      }
    }
  }

  private static BuildContext Load()
  {
    var path = Environment.GetEnvironmentVariable(ExecutionContextData.EnvironmentVariable);
    if (string.IsNullOrEmpty(path)) {
      throw new TaskException("No dotask context is available. Run this target using dotask.");
    }
    var data = JsonSerializer.Deserialize<ExecutionContextData>(File.ReadAllText(path))
      ?? throw new TaskException("Invalid dotask execution context.");
    var context = new BuildContext(data);
    Console.CancelKeyPress += ( _, args ) =>
    {
      args.Cancel = true;
      context._cancellation.Cancel();
    };
    return context;
  }
}

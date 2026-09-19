using System.Text.Json;
using DoTask.Cli.Configuration;
using DoTask.Cli.Discovery;
using DoTask.Cli.Metadata;
using DoTask.Cli.Parsing;
using DoTask.Runtime;

namespace DoTask.Cli.Execution;

internal sealed class TargetExecutor( TaskDirectory directory, ProjectConfiguration config, TargetCompiler compiler )
{
  public async Task<int> ExecuteAsync( TargetDefinition target, IReadOnlyList<string> arguments,
    CancellationToken cancellationToken, string[]? parentChain = null, string? sessionDirectory = null )
  {
    if (File.Exists(Path.Combine(directory.DirectoryPath, ".dotask", "transaction", "journal.json"))) {
      throw new TaskException("A shared-task update is incomplete. Run dotask --sync to recover it before executing project tasks.");
    }
    cancellationToken.ThrowIfCancellationRequested();
    if (target.Error is not null) {
      throw new TaskException(target.Error);
    }
    var chain = parentChain ?? [];
    if (chain.Contains(target.Name, StringComparer.OrdinalIgnoreCase)) {
      throw new TaskException($"Target cycle: {string.Join(" -> ", chain.Append(target.Name))}");
    }
    if (Requirements.HostName() == "unknown") {
      throw new TaskException("Target execution supports Windows, Linux, and macOS.");
    }
    var parameters = OptionBinder.Bind(target, config, arguments, directory.RootDirectory);
    Requirements.Validate(target, config, directory.RootDirectory);
    var ownsSession = sessionDirectory is null;
    sessionDirectory ??= Path.Combine(Path.GetTempPath(), "dotask-sessions", Guid.NewGuid().ToString("N"));
    TargetCompiler.CreatePrivateDirectory(sessionDirectory);
    var executionDirectory = Path.Combine(sessionDirectory, Guid.NewGuid().ToString("N"));
    string? contextPath = null;
    try {
      if (target.Group is { } group) {
        var executed = 0;
        foreach (var step in group.Steps) {
          cancellationToken.ThrowIfCancellationRequested();
          var catalog = new TargetCatalog(directory.DirectoryPath);
          var child = catalog.Find(step.Run);
          if (child is null && step.Optional) {
            continue;
          }
          child ??= catalog.Get(step.Run);
          var childArguments = step.Parameters.EnumerateObject()
            .Select(p => p.Name + "=" + Values.ToArgument(p.Value)).ToArray();
          var exitCode = await ExecuteAsync(child, childArguments, cancellationToken,
            [.. chain, target.Name], sessionDirectory);
          if (exitCode != 0) {
            return exitCode;
          }
          executed++;
        }
        if (group.RequireAtLeastOneStep && executed == 0) {
          throw new TaskException($"Target '{target.Name}' requires at least one step to execute; no steps were executed.");
        }
        return 0;
      }
      var compilation = await compiler.CompileAsync(target, cancellationToken, executionDirectory);
      if (!compilation.Success) {
        throw new TaskException($"Compilation failed for '{target.Name}':\n{compilation.Diagnostics}");
      }
      var data = new ExecutionContextData {
        RootDirectory = directory.RootDirectory,
        InvocationDirectory = directory.InvocationDirectory,
        TaskDirectory = directory.DirectoryPath,
        TargetFile = target.FilePath,
        TargetName = target.Name,
        DotnetExecutable = DotnetHost.Find(),
        CliAssembly = typeof(TargetExecutor).Assembly.Location,
        SessionDirectory = sessionDirectory,
        Settings = config.Settings,
        Parameters = parameters,
        TargetDefaults = config.TargetDefaults,
        CallChain = [.. chain, target.Name]
      };
      contextPath = await ContextFile.WriteAsync(sessionDirectory, data, cancellationToken);
      var result = await ProcessRunner.RunAsync(new ProcessDefinition {
        Executable = data.DotnetExecutable,
        Arguments = [compilation.AssemblyPath!],
        WorkingDirectory = directory.RootDirectory,
        ThrowOnError = false,
        Environment = new Dictionary<string, string?> { [ExecutionContextData.EnvironmentVariable] = contextPath }
      }, cancellationToken);
      if (result.ExitCode != 0) {
        Console.Error.WriteLine($"Target '{target.Name}' failed with exit code {result.ExitCode}.");
      }
      return result.ExitCode;
    } finally {
      if (contextPath is not null) {
        File.Delete(contextPath);
      }
      if (Directory.Exists(executionDirectory)) {
        Directory.Delete(executionDirectory, recursive: true);
      }
      if (ownsSession && Directory.Exists(sessionDirectory)) {
        Directory.Delete(sessionDirectory, recursive: true);
      }
    }
  }

  public static async Task<int> ExecuteCallAsync( string path, CancellationToken cancellationToken )
  {
    var call = JsonSerializer.Deserialize<TargetCall>(await File.ReadAllTextAsync(path, cancellationToken))
      ?? throw new TaskException("Invalid nested target request.");
    if (!Enum.IsDefined(call.Operation)) {
      throw new TaskException("Invalid target-call operation.");
    }
    if (call.Parameters.ValueKind != JsonValueKind.Object) {
      throw new TaskException("ExecTargetAsync parameters must be an object with named properties.");
    }
    var context = call.Context;
    var directory = new TaskDirectory(context.TaskDirectory, context.RootDirectory, context.InvocationDirectory);
    if (!Directory.Exists(directory.DirectoryPath)) {
      throw new TaskException($"The task directory disappeared during execution: {directory.DirectoryPath}");
    }
    var config = new ProjectConfiguration(context.Settings, context.TargetDefaults);
    var catalog = new TargetCatalog(directory.DirectoryPath);
    if (call.Operation == TargetCallOperation.Exists) {
      await ContextFile.WriteToAsync(path + ".result", new TargetCallReply(catalog.Find(call.Target) is not null), cancellationToken);
      return 0;
    }
    if (call.Operation == TargetCallOperation.ExecuteIfExists) {
      TargetCallReply reply;
      try {
        var target = catalog.Find(call.Target);
        if (target is null) {
          reply = new(false);
        } else {
          if (target.Error is not null) {
            throw new TaskException(target.Error);
          }
          var exitCode = await ExecuteCallTargetAsync(call, directory, config, target, cancellationToken);
          reply = new(true, exitCode, exitCode == 0 ? null : $"Target '{target.Name}' failed with exit code {exitCode}.");
        }
      } catch (Exception ex) when (ex is TaskException or IOException or UnauthorizedAccessException or ArgumentException or JsonException) {
        reply = new(true, Error: ex.Message);
      }
      await ContextFile.WriteToAsync(path + ".result", reply, cancellationToken);
      return 0;
    }
    return await ExecuteCallTargetAsync(call, directory, config, catalog.Get(call.Target), cancellationToken);
  }

  private static async Task<int> ExecuteCallTargetAsync( TargetCall call, TaskDirectory directory,
    ProjectConfiguration config, TargetDefinition target, CancellationToken cancellationToken )
  {
    var arguments = call.Parameters.EnumerateObject().Select(p => p.Name + "=" + Values.ToArgument(p.Value)).ToArray();
    return await new TargetExecutor(directory, config, new TargetCompiler()).ExecuteAsync(target,
      arguments, cancellationToken, call.Context.CallChain, call.Context.SessionDirectory);
  }
}

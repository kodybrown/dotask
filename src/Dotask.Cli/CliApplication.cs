using DoTask.Cli.Completion;
using DoTask.Cli.Configuration;
using DoTask.Cli.Discovery;
using DoTask.Cli.Execution;
using DoTask.Cli.Metadata;
using DoTask.Cli.Parsing;
using DoTask.Cli.SharedTasks;

namespace DoTask.Cli;

public static class CliApplication
{
  public static async Task<int> RunAsync( string[] arguments, string invocationDirectory,
    TextWriter output, TextWriter error, CancellationToken cancellationToken = default, int? consoleWidth = null,
    SharedTaskOptions? sharedTaskOptions = null )
  {
    try {
      if (arguments.FirstOrDefault() == "__complete") {
        CompletionCommand.Query(arguments[1..], invocationDirectory, output);
        return 0;
      }
      if (arguments.FirstOrDefault() == "__exec") {
        return arguments.Length == 2 ? await TargetExecutor.ExecuteCallAsync(arguments[1], cancellationToken)
          : throw new TaskException("Invalid nested target invocation.");
      }
      var command = CommandLine.Parse(arguments);
      if (command.Version) {
        output.WriteLine("dotask " + typeof(CliApplication).Assembly.GetName().Version?.ToString(3));
        return 0;
      }
      var helpCommand = command.Command?.Equals("help", StringComparison.OrdinalIgnoreCase) == true;
      var helpOutput = new HelpText(output, consoleWidth ?? HelpText.GetConsoleWidth(output));
      if (helpCommand && command.Arguments.Length > 1) {
        throw new TaskException("Usage: dotask help [TARGET]");
      }
      var sharedCommand = SharedTaskCommand.Commands.Contains(command.Command, StringComparer.OrdinalIgnoreCase);
      if (command.Help && (command.Command is null || sharedCommand || (helpCommand && command.Arguments.Length == 0))) {
        HelpWriter.Usage(helpOutput);
        return 0;
      }
      if (command.Command?.Equals("completion", StringComparison.OrdinalIgnoreCase) == true) {
        if (command.Arguments.Length != 1) {
          throw new TaskException("Usage: dotask completion <bash|zsh|fish|powershell>");
        }
        CompletionCommand.PrintScript(command.Arguments[0], output);
        return 0;
      }
      if (sharedCommand) {
        return await new SharedTaskCommand(new SharedTaskStore(sharedTaskOptions ?? SharedTaskOptions.FromEnvironment()), output)
          .RunAsync(command, invocationDirectory, cancellationToken);
      }
      var directory = TaskDirectory.Locate(invocationDirectory, command.UseDirectory);
      var catalog = new TargetCatalog(directory.DirectoryPath);
      var config = ProjectConfiguration.Load(directory.DirectoryPath, directory.RootDirectory);
      var targetName = helpCommand ? command.Arguments.FirstOrDefault() : command.Command;
      if (targetName is null) {
        HelpWriter.Project(helpOutput, config, directory.RootDirectory, directory.DirectoryPath);
        helpOutput.WriteLine("\nTargets:");
        foreach (var target in catalog.Targets) {
          cancellationToken.ThrowIfCancellationRequested();
          var diagnostic = GetHelpDiagnostic(target, config, directory.RootDirectory);
          helpOutput.WriteRow($"  {catalog.AliasFor(target) ?? target.Name,-18} ", diagnostic ?? target.Description);
        }
        if (catalog.Targets.Count == 0) {
          helpOutput.WriteLine("  (no C# targets)");
        }
        HelpWriter.CombinedOptions(helpOutput, catalog.Targets, config);
        helpOutput.WriteLine();
        helpOutput.WriteLine("See `dotask help <target>` for detailed information on each target.");
        return 0;
      }
      var selected = catalog.Get(targetName);
      if (helpCommand || command.Help) {
        cancellationToken.ThrowIfCancellationRequested();
        var diagnostic = GetHelpDiagnostic(selected, config, directory.RootDirectory);
        HelpWriter.Target(helpOutput, selected, config, diagnostic, catalog.AliasFor(selected) ?? selected.Name,
          directory.RootDirectory);
        return 0;
      }
      return await new TargetExecutor(directory, config, new TargetCompiler()).ExecuteAsync(selected, command.Arguments, cancellationToken);
    } catch (OperationCanceledException) {
      await error.WriteLineAsync("Cancelled.");
      return 130;
    } catch (Exception ex) when (ex is TaskException or IOException or UnauthorizedAccessException or ArgumentException or System.Text.Json.JsonException or HttpRequestException) {
      if (arguments.FirstOrDefault() != "__complete") {
        await error.WriteLineAsync("dotask: " + ex.Message);
      }
      return arguments.FirstOrDefault() == "__complete" ? 0 : 1;
    }
  }

  private static string? GetHelpDiagnostic( TargetDefinition target, ProjectConfiguration config, string root )
  {
    if (target.Error is not null) {
      return target.Error;
    }
    try {
      OptionBinder.Bind(target, config, [], root, requireValues: false);
      return null;
    } catch (TaskException ex) {
      return ex.Message;
    }
  }
}

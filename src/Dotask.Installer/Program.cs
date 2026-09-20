using System.Text.Json;
using DoTask;

namespace DoTask.Installer;

internal static class Program
{
  public static async Task<int> Main( string[] args )
  {
    using var cancellation = new CancellationTokenSource();
    Console.CancelKeyPress += ( _, e ) => { e.Cancel = true; cancellation.Cancel(); };
    try {
      string? bin = null;
      string? root = null;
      for (var i = 0; i < args.Length; i++) {
        if (args[i] is not ("--bin-dir" or "--install-root") || i + 1 == args.Length) {
          throw new TaskException("Usage: dotask-installer [--bin-dir PATH] [--install-root PATH]");
        }
        var option = args[i++];
        var value = Path.GetFullPath(args[i]);
        if (option == "--bin-dir") {
          bin = value;
        } else {
          root = value;
        }
      }
      var definition = JsonSerializer.Deserialize<InstallationDefinition>(
        await File.ReadAllTextAsync(Path.Combine(AppContext.BaseDirectory, "installer.json"), cancellation.Token))
        ?? throw new TaskException("Invalid dotask installer package.");
      if (definition.AppId != "dotask" || definition.SourceDirectory != "payload" ||
          definition.InstallRoot is not null || definition.BinDirectory is not null) {
        throw new TaskException("Invalid dotask installer identity or locations.");
      }
      var result = await UserInstaller.InstallAsync(definition with {
        SourceDirectory = Path.Combine(AppContext.BaseDirectory, "payload"),
        BinDirectory = bin,
        InstallRoot = root
      }, cancellation.Token);
      Console.WriteLine($"{(result.Reused ? "Activated existing" : "Installed")} dotask {definition.Version}");
      Console.WriteLine($"  Files: {result.InstallDirectory}");
      Console.WriteLine($"  Commands: {result.BinDirectory}");
      foreach (var warning in result.Warnings) {
        Console.WriteLine($"  Note: {warning}");
      }

      return 0;
    } catch (OperationCanceledException) {
      Console.Error.WriteLine("Installation cancelled.");
      return 130;
    } catch (Exception ex) when (ex is TaskException or IOException or UnauthorizedAccessException or JsonException or ArgumentException) {
      Console.Error.WriteLine(ex.Message);
      return 1;
    }
  }
}

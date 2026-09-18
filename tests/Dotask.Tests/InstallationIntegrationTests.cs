using System.Diagnostics;
using System.Reflection.PortableExecutable;
using System.Security.Cryptography;
using System.Text.Json;

namespace DoTask.Tests;

public sealed class InstallationIntegrationTests
{
  [Fact]
  public void BundledShimsMatchTheirHashesArchitecturesAndHaveNoManagedRuntimeDependency()
  {
    using var stream = typeof(InstallationIntegrationTests).Assembly.GetManifestResourceStream("Shim/hashes.json")!;
    var hashes = JsonSerializer.Deserialize<Dictionary<string, string>>(stream)!;
    foreach (var entry in hashes) {
      using var source = typeof(InstallationIntegrationTests).Assembly.GetManifestResourceStream("Shim/" + entry.Key)!;
      Assert.Equal(entry.Value, Convert.ToHexStringLower(SHA256.HashData(source)));
    }
    foreach (var (architecture, machine) in new[] { ("x64", Machine.Amd64), ("arm64", Machine.Arm64) }) {
      using var binary = typeof(BuildContext).Assembly.GetManifestResourceStream($"DoTask.Shim.win-{architecture}.exe")!;
      Assert.Equal(hashes[$"assets/win-{architecture}.exe"], Convert.ToHexStringLower(SHA256.HashData(binary)));
      binary.Position = 0;
      using var pe = new PEReader(binary);
      Assert.Equal(machine, pe.PEHeaders.CoffHeader.Machine);
      Assert.Null(pe.PEHeaders.CorHeader);
      Assert.Equal(Subsystem.WindowsCui, pe.PEHeaders.PEHeader!.Subsystem);
    }
  }

  [Fact]
  public async Task SharedInstallTaskPublishesRunsAndUpdatesWithCustomOutputPaths()
  {
    using var project = new TestProject();
    CopyShared(project, "dotnet/install.cs");
    project.Write(".dotasks.yaml", "version: 1\nsettings:\n  project: 'source app/Probe.csproj'\n");
    // Isolated fixture properties, outside the source checkout; intentionally
    // use a nonstandard PublishDir to catch any bin/Release assumptions.
    project.Write("Directory.Build.props", "<Project />");
    project.Write("source app/Probe.csproj", """
      <Project Sdk="Microsoft.NET.Sdk">
        <PropertyGroup>
          <TargetFramework>net10.0</TargetFramework>
          <OutputType>Exe</OutputType>
          <ImplicitUsings>enable</ImplicitUsings>
          <Version>2.3.4</Version>
          <AssemblyName>installed-probe</AssemblyName>
          <ToolCommandName>probe</ToolCommandName>
          <PublishDir>../custom published files/</PublishDir>
        </PropertyGroup>
        <ItemGroup><None Include="data.txt" CopyToPublishDirectory="Always" /></ItemGroup>
      </Project>
      """);
    project.Write("source app/data.txt", "first build");
    project.Write("source app/Program.cs", """
      using System.Text.Json;
      using System.Diagnostics;
      using System.Runtime.InteropServices;
      if (args.Length == 2 && args[0] == "--exit-code") return int.Parse(args[1]);
      if (args.Length > 0 && args[0] == "--wait-for-control") {
        using var received = new ManualResetEventSlim();
        Console.CancelKeyPress += (_, e) => { e.Cancel = true; received.Set(); };
        File.WriteAllText(args[1], "ready");
        return received.Wait(TimeSpan.FromSeconds(15)) ? 42 : 93;
      }
      if (args.Length > 0 && args[0] == "--control-driver") {
        // Own a console so a generated Ctrl+C cannot reach the test runner.
        Native.FreeConsole();
        if (!Native.AllocConsole()) return 94;
        Native.SetConsoleCtrlHandler(IntPtr.Zero, false);
        Console.CancelKeyPress += (_, e) => e.Cancel = true;
        var ready = Path.Combine(Environment.CurrentDirectory, "control-ready");
        var start = new ProcessStartInfo(args[1]) { UseShellExecute = false };
        start.ArgumentList.Add("--wait-for-control"); start.ArgumentList.Add(ready);
        using var child = Process.Start(start)!;
        var until = DateTime.UtcNow.AddSeconds(15);
        while (!File.Exists(ready) && !child.HasExited && DateTime.UtcNow < until) Thread.Sleep(20);
        if (!File.Exists(ready)) { if (!child.HasExited) child.Kill(true); return 95; }
        if (!Native.GenerateConsoleCtrlEvent(0, 0)) { child.Kill(true); return 96; }
        if (!child.WaitForExit(15000)) { child.Kill(true); return 97; }
        return child.ExitCode;
      }
      Console.WriteLine(JsonSerializer.Serialize(new {
        Args = args, Cwd = Environment.CurrentDirectory,
        Environment = Environment.GetEnvironmentVariable("DOTASK_INSTALL_TEST"),
        Input = Console.ReadLine(), Data = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "data.txt"))
      }));
      Console.Error.WriteLine("probe stderr");
      return 23;
      static class Native {
        [DllImport("kernel32.dll")] public static extern bool FreeConsole();
        [DllImport("kernel32.dll")] public static extern bool AllocConsole();
        [DllImport("kernel32.dll")] public static extern bool SetConsoleCtrlHandler(IntPtr handler, bool add);
        [DllImport("kernel32.dll")] public static extern bool GenerateConsoleCtrlEvent(uint control, uint group);
      }
      """);
    var bin = Path.Combine(project.Root, "commands with spaces 日本語");
    var root = Path.Combine(project.Root, "installed apps 日本語");
    string[] install = ["install", "--self-contained=false", "--bin-dir", bin, "--install-root", root];
    var result = await project.RunAsync(install);
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    var app = Path.Combine(root, "installed-probe");
    var first = Assert.Single(Directory.GetDirectories(app));
    Assert.StartsWith("2.3.4-", Path.GetFileName(first));
    var command = Path.Combine(bin, OperatingSystem.IsWindows() ? "probe.exe" : "probe");
    string[] arguments = ["", "two words", "a\"b", "'single'", @"trailing\", "日本語", "%BIN%", "& | ; < > $(nope)", "line1\nline2"];
    var probe = await RunProbe(command, project.Root, arguments);
    Assert.Equal(23, probe.ExitCode);
    Assert.Equal("probe stderr", probe.StandardError.Trim());
    using (var output = JsonDocument.Parse(probe.StandardOutput)) {
      Assert.Equal(arguments, output.RootElement.GetProperty("Args").Deserialize<string[]>());
      Assert.Equal("inherit me", output.RootElement.GetProperty("Environment").GetString());
      Assert.Equal("stdin text", output.RootElement.GetProperty("Input").GetString());
      // macOS may resolve /var to /private/var in the process working directory.
      Assert.Equal(InstallationFiles.PhysicalDirectory(project.Root),
        InstallationFiles.PhysicalDirectory(output.RootElement.GetProperty("Cwd").GetString()!));
      Assert.Equal("first build", output.RootElement.GetProperty("Data").GetString());
    }
    result = await project.RunAsync(install);
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.Contains("Activated existing", result.StandardOutput);
    Assert.Single(Directory.GetDirectories(app));
    project.Write("source app/data.txt", "second build");
    result = await project.RunAsync(install);
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.Equal(2, Directory.GetDirectories(app).Length);
    Assert.Equal("first build", File.ReadAllText(Path.Combine(first, "data.txt")));
    Assert.Contains("second build", (await RunProbe(command, project.Root, [])).StandardOutput);

    // A compiler failure must leave the existing command usable.
    project.Write("source app/Program.cs", "this is not valid C#;");
    result = await project.RunAsync(install);
    Assert.NotEqual(0, result.ExitCode);
    Assert.Contains("error", result.StandardError);
    Assert.Contains("second build", (await RunProbe(command, project.Root, [])).StandardOutput);

    if (OperatingSystem.IsWindows()) {
      var control = await RunProbe(Path.Combine(first, "installed-probe.exe"), project.Root, ["--control-driver", command]);
      Assert.Equal(42, control.ExitCode);
      Assert.Equal(unchecked((int)0xc0000005), (await RunProbe(command, project.Root, ["--exit-code", unchecked((int)0xc0000005).ToString()])).ExitCode);
      var shim = Path.Combine(bin, "probe.shim");
      var saved = File.ReadAllText(shim);
      File.Delete(shim);
      var missing = await RunProbe(command, project.Root, []);
      Assert.Equal(1, missing.ExitCode);
      Assert.Contains(".shim", missing.StandardError);
      File.WriteAllText(shim, "path = \"relative.exe\"\n");
      Assert.Equal(1, (await RunProbe(command, project.Root, [])).ExitCode);
      File.WriteAllText(shim, $"path = \"{command}\"\n");
      Assert.Contains("itself", (await RunProbe(command, project.Root, [])).StandardError);
      File.WriteAllText(shim, saved);
    }
  }

  private static async Task<ProcessResult> RunProbe( string executable, string cwd, string[] arguments )
  {
    var start = new ProcessStartInfo(executable) {
      WorkingDirectory = cwd,
      UseShellExecute = false,
      RedirectStandardOutput = true,
      RedirectStandardError = true,
      RedirectStandardInput = true
    };
    foreach (var argument in arguments) {
      start.ArgumentList.Add(argument);
    }

    start.Environment["DOTASK_INSTALL_TEST"] = "inherit me";
    using var process = Process.Start(start)!;
    var stdout = process.StandardOutput.ReadToEndAsync();
    var stderr = process.StandardError.ReadToEndAsync();
    try {
      await process.StandardInput.WriteLineAsync("stdin text");
      process.StandardInput.Close();
    } catch (IOException) { } // Invalid shim descriptions can fail before reading stdin.
    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(30));
    try { await process.WaitForExitAsync(timeout.Token); } catch { process.Kill(entireProcessTree: true); throw; }
    return new(process.ExitCode, await stdout, await stderr);
  }

  private static void CopyShared( TestProject project, string relative )
  {
    using var stream = typeof(InstallationIntegrationTests).Assembly.GetManifestResourceStream("Shared/" + relative)!;
    using var reader = new StreamReader(stream);
    project.Write(".tasks/dotask-official/" + relative, reader.ReadToEnd());
  }
}

using System.Diagnostics;
using System.Text.Json;

namespace DoTask.Tests;

public sealed class RustCliTests
{
  private static string Executable => OperatingSystem.IsWindows()
    ? "C:/tmp/_dotnet/dotask-rust/release/dotask.exe" : "/tmp/_dotnet/dotask-rust/release/dotask";

  private static async Task<ProcessResult> Run( TestProject project, string[] args, string? directory = null )
  {
    Assert.True(File.Exists(Executable), "Build the complete Rust preview first: build.cmd rust-cli (Unix: ./build.sh rust-cli).");
    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(90));
    // Native stdout is UTF-8. ProcessStartInfo otherwise chooses the test
    // runner's legacy Windows console encoding and corrupts captured text.
    using var child = new Process {
      StartInfo = new ProcessStartInfo(Executable) {
        WorkingDirectory = directory ?? project.Root,
        UseShellExecute = false,
        RedirectStandardOutput = true,
        RedirectStandardError = true,
        StandardOutputEncoding = System.Text.Encoding.UTF8,
        StandardErrorEncoding = System.Text.Encoding.UTF8
      }
    };
    foreach (var argument in args) {
      child.StartInfo.ArgumentList.Add(argument);
    }
    child.Start();
    var output = child.StandardOutput.ReadToEndAsync();
    var error = child.StandardError.ReadToEndAsync();
    try {
      await child.WaitForExitAsync(timeout.Token);
      return new ProcessResult(child.ExitCode, await output, await error);
    } finally {
      if (!child.HasExited) {
        child.Kill(entireProcessTree: true);
        await child.WaitForExitAsync();
      }
    }
  }

  [Fact]
  public async Task NativeExecutionPreservesContextDefaultsArgumentTokensAndStandardStreams()
  {
    using var project = new TestProject();
    project.Write(".dotasks.yaml", "settings: { label: configured }\ntargets: { '_/tools/probe': { defaults: { count: 7 } } }");
    var source = project.Target("_/tools/probe", """
      var p = BuildContext.Current;
      Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new {
        p.RootDirectory, p.InvocationDirectory, p.TaskDirectory, p.TargetFile, p.TargetName,
        Cwd = Environment.CurrentDirectory, Label = p.Config.Get<string>("label"),
        Count = p.Parameters.Get<int>("count"), Text = p.Parameters.Get<string>("text"),
        Empty = p.Parameters.Get<string>("empty"), Path = p.Parameters.GetPath("path"),
        Context = Environment.GetEnvironmentVariable("DOTASK_EXECUTION_CONTEXT")
      }));
      Console.Error.WriteLine("task stderr");
      """, """
      /// <option name="count" type="int" default="2" />
      /// <option name="text" required="true" />
      /// <option name="empty" />
      /// <option name="path" type="path" />
      """);
    var subdirectory = Directory.CreateDirectory(Path.Combine(project.Root, "nested 日本語")).FullName;
    const string text = "spaces 日本語 \"quotes\" \\literal\\ $dollar; pipe| & *";
    var result = await Run(project, ["tools/probe", "--text", text, "empty=", "path=assets\\file.txt"], subdirectory);
    Assert.True(result.ExitCode == 0, result.StandardError);
    using var json = JsonDocument.Parse(result.StandardOutput);
    var data = json.RootElement;
    Assert.Equal(project.Root, data.GetProperty("RootDirectory").GetString());
    Assert.Equal(project.Root, data.GetProperty("Cwd").GetString());
    Assert.Equal(subdirectory, data.GetProperty("InvocationDirectory").GetString());
    Assert.Equal(project.Tasks, data.GetProperty("TaskDirectory").GetString());
    Assert.Equal(source, data.GetProperty("TargetFile").GetString());
    Assert.Equal("_/tools/probe", data.GetProperty("TargetName").GetString());
    Assert.Equal("configured", data.GetProperty("Label").GetString());
    Assert.Equal(7, data.GetProperty("Count").GetInt32());
    Assert.Equal(text, data.GetProperty("Text").GetString());
    Assert.Equal("", data.GetProperty("Empty").GetString());
    Assert.Equal(Path.Combine(project.Root, "assets", "file.txt"), data.GetProperty("Path").GetString());
    Assert.False(File.Exists(data.GetProperty("Context").GetString()));
    Assert.Contains("task stderr", result.StandardError);
    Assert.False(Directory.Exists(Path.Combine(project.Tasks, "bin")));
    Assert.False(Directory.Exists(Path.Combine(project.Tasks, "obj")));
    // The native distribution deliberately has no managed CLI to fall back to.
    Assert.False(File.Exists(Path.Combine(Path.GetDirectoryName(Executable)!, "csharp", "dotask.dll")));
  }

  [Fact]
  public async Task NativeHelpReadsMetadataWithoutCompilingOrRunningAndValidatesDefaults()
  {
    using var project = new TestProject();
    project.Target("broken", "DoesNotCompile(); File.WriteAllText(\"ran\", \"bad\");", """
      /// <summary>Visible metadata.</summary>
      /// <option name="message" required="true" />
      """);
    foreach (var args in new[] { Array.Empty<string>(), new[] { "help" }, new[] { "broken", "--help" } }) {
      var result = await Run(project, args);
      Assert.True(result.ExitCode == 0, result.StandardError);
      Assert.Contains("Visible metadata", result.StandardOutput);
      Assert.DoesNotContain("CS0103", result.StandardError);
    }
    project.Write(".dotasks.yaml", "targets: { broken: { defaults: { invalid: value } } }");
    var invalid = await Run(project, ["broken", "--help"]);
    Assert.Equal(1, invalid.ExitCode);
    Assert.Contains("no option 'invalid'", invalid.StandardError);
    Assert.False(File.Exists(Path.Combine(project.Root, "ran")));
  }

  [Fact]
  public async Task NativeExecutionUsesExactDirectoryAndExactNamesBeforeAmbiguousAliases()
  {
    using var project = new TestProject();
    project.Target("build", "throw new Exception(\"wrong directory\");");
    project.Write("custom/one/build.cs", "Console.WriteLine(\"one\");");
    project.Write("custom/two/build.cs", "Console.WriteLine(\"two\");");
    var ambiguous = await Run(project, ["--use-dir", "custom", "build"]);
    Assert.Equal(1, ambiguous.ExitCode);
    Assert.Contains("Ambiguous target", ambiguous.StandardError);
    var selected = await Run(project, ["--use-dir=custom", "one/build"]);
    Assert.True(selected.ExitCode == 0, selected.StandardError);
    Assert.Equal("one", selected.StandardOutput.Trim());
    project.Write("custom/build.cs", "Console.WriteLine(\"exact\");");
    var exact = await Run(project, ["build", "--use-dir", "custom"]);
    Assert.True(exact.ExitCode == 0, exact.StandardError);
    Assert.Equal("exact", exact.StandardOutput.Trim());
    project.Write("custom/.dotask/transaction/journal.json", "{}");
    var pending = await Run(project, ["--use-dir", "custom", "build"]);
    Assert.Equal(1, pending.ExitCode);
    Assert.Contains("shared-task update is incomplete", pending.StandardError);
  }

  [Fact]
  public async Task NativeNestedCallsReturnToRustAndPreserveConfigurationSnapshotAndFailureKinds()
  {
    using var project = new TestProject();
    project.Write(".dotasks.yaml", "settings: { label: original }\ntargets: { child: { defaults: { message: inherited } } }");
    project.Target("child", "Console.WriteLine(BuildContext.Current.Config.Get<string>(\"label\") + \":\" + BuildContext.Current.Parameters.Get<string>(\"message\"));",
      "/// <option name=\"message\" required=\"true\" />");
    project.Target("failure", "Environment.Exit(27);");
    project.Target("broken", "DoesNotCompile();");
    project.Target("one/check");
    project.Target("two/check");
    project.Target("outer", """
      var p = BuildContext.Current;
      var context = System.Text.Json.JsonDocument.Parse(File.ReadAllText(Environment.GetEnvironmentVariable("DOTASK_EXECUTION_CONTEXT")!));
      Console.WriteLine("native=" + (context.RootElement.GetProperty("CliArguments").GetArrayLength() == 0));
      File.WriteAllText(p.Path(".dotasks.yaml"), "this is invalid YAML: [");
      Console.WriteLine("exists=" + await p.TargetExistsAsync("broken"));
      await p.ExecTargetAsync("child");
      await p.ExecTargetAsync("child", new { Message = "override 日本語" });
      foreach (var name in new[] { "missing", "failure", "broken", "check", "OUTER" }) {
        var result = await p.ExecTargetIfExistsAsync(name);
        Console.WriteLine(name + ":" + result.Status + ":" + result.ExitCode + ":" + result.Error);
      }
      """, async: true);
    var result = await Run(project, ["outer", "--verbose"]);
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Contains("native=True", result.StandardOutput);
    Assert.Contains("exists=True", result.StandardOutput);
    Assert.Contains("original:inherited", result.StandardOutput);
    Assert.True(result.StandardOutput.Contains("original:override 日本語", StringComparison.Ordinal), result.StandardOutput);
    Assert.Contains("missing:NotFound", result.StandardOutput);
    Assert.Contains("failure:Failed:27:", result.StandardOutput);
    Assert.Contains("broken:Failed::Compilation failed", result.StandardOutput);
    Assert.Contains("check:Failed::Ambiguous target", result.StandardOutput);
    Assert.Contains("OUTER:Failed::Target cycle: outer -> outer", result.StandardOutput);
    Assert.Contains("[dotask] Executing: child", result.StandardError);
  }

  [Fact]
  public async Task NativeGroupsStopAtFailureSkipOnlyAbsenceAndDetectCycles()
  {
    using var project = new TestProject();
    project.Target("first", "File.AppendAllText(\"order\", BuildContext.Current.Parameters.Get<string>(\"text\"));", "/// <option name=\"text\" />");
    project.Target("fail", "Environment.Exit(23);");
    project.Write(".tasks/group.task", """
      steps:
        - run: missing
          optional: true
        - run: first
          with: { text: 'ordered' }
        - run: fail
        - run: first
          with: { text: 'wrong' }
      """);
    var failure = await Run(project, ["group"]);
    Assert.Equal(23, failure.ExitCode);
    Assert.Equal("ordered", File.ReadAllText(Path.Combine(project.Root, "order")));
    project.Write(".tasks/empty.task", "require_at_least_1_step: true\nsteps: [{ run: missing, optional: true }]");
    Assert.Contains("requires at least one step", (await Run(project, ["empty"])).StandardError);
    project.Write(".tasks/cycle.task", "steps: [{ run: cycle }]");
    Assert.Contains("Target cycle: cycle -> cycle", (await Run(project, ["cycle"])).StandardError);
  }

  [Fact]
  public async Task NativeExecutionReportsValidationCompilationAndTaskFailures()
  {
    using var project = new TestProject();
    project.Target("required", metadata: "/// <option name=\"input\" required=\"true\" />");
    project.Target("requirements", metadata: "/// <requires setting=\"missing\" />");
    project.Target("metadata", metadata: "/// <summary>Invalid XML");
    project.Target("compile", "DoesNotCompile();");
    project.Target("exception", "throw new TaskException(\"task failure\");");
    foreach (var (name, expected) in new[] { ("required", "requires --input"), ("requirements", "requires setting"),
      ("metadata", "Metadata error"), ("compile", "CS0103"), ("exception", "task failure"), ("unknown", "Unknown target") }) {
      var result = await Run(project, [name]);
      Assert.Equal(1, result.ExitCode);
      Assert.Contains(expected, result.StandardError);
    }
    // Windows process exit codes are not limited to a byte.
    var code = OperatingSystem.IsWindows() ? 3010 : 29;
    project.Target("exit", $"Environment.Exit({code});");
    Assert.Equal(code, (await Run(project, ["exit"])).ExitCode);
  }

  [Fact]
  public async Task NativeNestedInstallerResultUsesTheExistingStructuredResultProtocol()
  {
    using var project = new TestProject();
    project.Write("artifact.dll", "fixture result, never executed");
    project.Target("create-installer", """
      var p = BuildContext.Current;
      await p.SetInstallerResultAsync(new InstallerArtifact {
        FilePath = p.Path("artifact.dll"), Kind = InstallerKind.DotNetAssembly,
        OS = p.OS, Architecture = p.Architecture, DefaultArguments = ["日本語", "", "two words"]
      });
      """, async: true);
    project.Target("outer", """
      var artifact = await BuildContext.Current.CreateInstallerAsync();
      Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(artifact));
      """, async: true);
    var result = await Run(project, ["outer"]);
    Assert.True(result.ExitCode == 0, result.StandardError);
    var artifact = JsonSerializer.Deserialize<InstallerArtifact>(result.StandardOutput)!;
    Assert.Equal(Path.Combine(project.Root, "artifact.dll"), artifact.FilePath);
    Assert.Equal(["日本語", "", "two words"], artifact.DefaultArguments);
  }

  [Fact]
  public async Task NativeTaskInheritsStandardInput()
  {
    using var project = new TestProject();
    project.Target("input", "Console.WriteLine(\"read=\" + Console.ReadLine());");
    using var child = new Process {
      StartInfo = new ProcessStartInfo(Executable) {
        WorkingDirectory = project.Root,
        UseShellExecute = false,
        RedirectStandardInput = true,
        RedirectStandardOutput = true,
        RedirectStandardError = true
      }
    };
    child.StartInfo.ArgumentList.Add("input");
    child.Start();
    var output = child.StandardOutput.ReadToEndAsync();
    var error = child.StandardError.ReadToEndAsync();
    await child.StandardInput.WriteLineAsync("input with spaces");
    child.StandardInput.Close();
    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(60));
    try {
      await child.WaitForExitAsync(timeout.Token);
      Assert.True(child.ExitCode == 0, await error);
      Assert.Contains("read=input with spaces", await output);
    } finally {
      if (!child.HasExited) {
        child.Kill(entireProcessTree: true);
        await child.WaitForExitAsync();
      }
    }
  }

  [Theory]
  [InlineData(false)]
  [InlineData(true)]
  public async Task NativeCancellationStopsUncooperativeDescendantsAndRemovesContext( bool nested )
  {
    using var project = new TestProject();
    project.Write(".tasks/wait.cs", """
      using System.Diagnostics;
      using DoTask;
      Console.CancelKeyPress += (_, e) => e.Cancel = true;
      if (args.Length != 0) {
        File.WriteAllText("child-ready", Environment.ProcessId.ToString());
        Thread.Sleep(Timeout.Infinite);
        return;
      }
      // Deliberately ignore cancellation in both processes, requiring the
      // native runner's process-tree fallback rather than library cooperation.
      var start = new ProcessStartInfo(Environment.ProcessPath!) { UseShellExecute = false };
      start.ArgumentList.Add(System.Reflection.Assembly.GetExecutingAssembly().Location);
      start.ArgumentList.Add("child");
      using var child = Process.Start(start)!;
      var until = DateTime.UtcNow.AddSeconds(20);
      while (!File.Exists("child-ready") && !child.HasExited && DateTime.UtcNow < until) Thread.Sleep(20);
      if (!File.Exists("child-ready")) { child.Kill(true); throw new Exception("Child failed to start"); }
      File.WriteAllText("context-path", Environment.GetEnvironmentVariable("DOTASK_EXECUTION_CONTEXT"));
      File.WriteAllText("task-ready", Environment.ProcessId.ToString());
      Thread.Sleep(Timeout.Infinite);
      """);
    project.Target("nested", "await BuildContext.Current.ExecTargetAsync(\"wait\");", async: true);
    project.Write(".tasks/control-driver.cs", """
      using System.Diagnostics;
      using System.Runtime.InteropServices;
      using DoTask;
      /// <option name="executable" type="path" required="true" />
      /// <option name="target" required="true" />
      public static class Target {
        public static int Main() {
          if (OperatingSystem.IsWindows()) {
            // Own a hidden console so generated control events cannot reach
            // the test runner or the user's terminal.
            Native.FreeConsole();
            if (!Native.AllocConsole()) return 94;
            Native.ShowWindow(Native.GetConsoleWindow(), 0);
            Native.SetConsoleCtrlHandler(IntPtr.Zero, false);
          }
          Console.CancelKeyPress += (_, e) => e.Cancel = true;
          var p = BuildContext.Current;
          var start = new ProcessStartInfo(p.Parameters.GetPath("executable")) { UseShellExecute = false };
          start.ArgumentList.Add(p.Parameters.Get<string>("target"));
          using var child = Process.Start(start)!;
          try {
            var until = DateTime.UtcNow.AddSeconds(60);
            while (!File.Exists("task-ready") && !child.HasExited && DateTime.UtcNow < until) Thread.Sleep(20);
            if (!File.Exists("task-ready")) return 95;
            if (OperatingSystem.IsWindows()) {
              if (!Native.GenerateConsoleCtrlEvent(0, 0)) return 96;
            } else if (Native.kill(child.Id, 2) != 0) return 96;
            if (!child.WaitForExit(20000)) return 97;
            return child.ExitCode;
          } finally {
            if (!child.HasExited) { child.Kill(true); child.WaitForExit(); }
          }
        }
      }
      static class Native {
        [DllImport("kernel32.dll")] public static extern bool FreeConsole();
        [DllImport("kernel32.dll")] public static extern bool AllocConsole();
        [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow();
        [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int show);
        [DllImport("kernel32.dll")] public static extern bool SetConsoleCtrlHandler(IntPtr handler, bool add);
        [DllImport("kernel32.dll")] public static extern bool GenerateConsoleCtrlEvent(uint control, uint group);
        [DllImport("libc")] public static extern int kill(int pid, int signal);
      }
      """);
    // Compile/launch the isolated control driver through the maintained CLI;
    // the driver starts and signals the native CLI under test.
    var result = await project.RunAsync("control-driver", "executable=" + Executable, "target=" + (nested ? "nested" : "wait"));
    Assert.True(result.ExitCode == 130, result.StandardOutput + result.StandardError);
    foreach (var name in new[] { "task-ready", "child-ready" }) {
      var pid = int.Parse(File.ReadAllText(Path.Combine(project.Root, name)));
      Process? remaining = null;
      try { remaining = Process.GetProcessById(pid); } catch (ArgumentException) { }
      using (remaining) {
        Assert.True(remaining is null || remaining.HasExited, $"Descendant {pid} survived cancellation.");
      }
    }
    var contextPath = File.ReadAllText(Path.Combine(project.Root, "context-path"));
    Assert.False(File.Exists(contextPath), "The cancelled execution context was not removed.");
  }
}

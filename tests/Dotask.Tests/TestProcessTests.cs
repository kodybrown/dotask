using System.Diagnostics;
using System.Text.Json;
using DoTask.Shared.DotNet;

namespace DoTask.Tests;

public sealed class TestProcessTests
{
  [Fact]
  public async Task PropagatesExitAndPreservesArgumentBoundaries()
  {
    using var project = new TestProject();
    var output = Path.Combine(project.Root, "arguments.json");
    var probe = await project.BuildProcessProbeAsync("File.WriteAllText(args[0], System.Text.Json.JsonSerializer.Serialize(args.Skip(1))); Environment.Exit(17);");
    Assert.Equal(17, TestProcess.Run("dotnet", [probe, output, "", "a b", "é", "A|B&x"], project.Root, 15, 0, "", default));
    Assert.Equal(new[] { "", "a b", "é", "A|B&x" }, JsonSerializer.Deserialize<string[]>(File.ReadAllText(output)));
  }

  [Fact]
  public void RefusesConcurrentRunsWithoutStartingAChild()
  {
    var name = "dotask-test-" + Guid.NewGuid().ToString("N");
    // Hold the mutex on another thread: same-thread acquisition is reentrant.
    using var held = new ManualResetEventSlim();
    using var release = new ManualResetEventSlim();
    var thread = new Thread(() =>
    {
      using var mutex = new Mutex(false, OperatingSystem.IsWindows() ? @"Local\" + name : name);
      mutex.WaitOne();
      held.Set();
      release.Wait();
      mutex.ReleaseMutex();
    });
    thread.Start();
    held.Wait();
    try {
      var error = Assert.Throws<TaskException>(() => TestProcess.Run("missing-command", [], Path.GetTempPath(), 10, 0, name, default));
      Assert.Contains("Another test run is active", error.Message);
    } finally {
      release.Set();
      thread.Join();
    }
  }

  [Theory]
  [InlineData(false)]
  [InlineData(true)]
  public async Task TimeoutAndCancellationTerminateChildrenAndReleaseTheLock( bool cancel )
  {
    using var project = new TestProject();
    var pidFile = Path.Combine(project.Root, "child.pid");
    var probe = await project.BuildProcessProbeAsync("""
      if (args.Length == 0) { Thread.Sleep(30000); return; }
      var start = new System.Diagnostics.ProcessStartInfo("dotnet") { UseShellExecute = false };
      start.ArgumentList.Add(typeof(Program).Assembly.Location);
      using var child = System.Diagnostics.Process.Start(start)!;
      File.WriteAllText(args[0], child.Id.ToString());
      Thread.Sleep(30000);
      """);
    using var cancellation = new CancellationTokenSource();
    if (cancel)
      cancellation.CancelAfter(1500);
    var name = "dotask-test-" + Guid.NewGuid().ToString("N");
    Assert.ThrowsAny<Exception>(() => TestProcess.Run("dotnet", [probe, pidFile], project.Root, cancel ? 20 : 2, 0, name, cancellation.Token));
    Assert.True(File.Exists(pidFile), "The probe must spawn its child before testing cleanup.");
    var pid = int.Parse(File.ReadAllText(pidFile));
    using var child = GetProcess(pid);
    Assert.True(child is null || child.WaitForExit(5000), "A descendant survived the guard.");
    // A failure must not leave the single-run lock held.
    using var mutex = new Mutex(false, OperatingSystem.IsWindows() ? @"Local\" + name : name);
    Assert.True(mutex.WaitOne(0));
    mutex.ReleaseMutex();
  }

  [Fact]
  public async Task WindowsMemoryCapIsEnforcedAcrossTheJob()
  {
    if (!OperatingSystem.IsWindows())
      return;
    using var project = new TestProject();
    var probe = await project.BuildProcessProbeAsync("""
      try {
        var memory = new byte[256 * 1024 * 1024];
        for (var i = 0; i < memory.Length; i += 4096) memory[i] = 1;
        GC.KeepAlive(memory);
      } catch (OutOfMemoryException) { Environment.Exit(42); }
      """);
    Assert.Equal(0, TestProcess.Run("dotnet", [probe], project.Root, 20, 0, "", default));
    Assert.Equal(42, TestProcess.Run("dotnet", [probe], project.Root, 20, 128, "", default));
  }

  [Fact]
  public async Task SharedTaskForwardsFilterDefaultsAndFreshWorkers()
  {
    using var project = new TestProject();
    foreach (var name in new[] { "test.cs", "_support/TestProcess.cs" }) {
      using var stream = typeof(TestProcessTests).Assembly.GetManifestResourceStream("Shared/dotnet/" + name)!;
      using var reader = new StreamReader(stream);
      project.Write(".tasks/_/dotnet/" + name, reader.ReadToEnd());
    }
    var output = Path.Combine(project.Root, "received.json");
    var probe = await project.BuildProcessProbeAsync("File.WriteAllText(" + JsonSerializer.Serialize(output)
      + ", System.Text.Json.JsonSerializer.Serialize(args));");
    // A host-native apphost lets the real shared entry point invoke a fake CLI.
    var executable = Path.ChangeExtension(probe, OperatingSystem.IsWindows() ? ".exe" : null);
    project.Write("Sample.slnx", "<Solution />");
    project.Write(".dotasks.yaml", "settings:\n  solution: Sample.slnx\ntargets:\n  _/dotnet/test:\n    defaults:\n"
      + "      timeout-seconds: 20\n      no-build: true\n      dotnet: " + JsonSerializer.Serialize(executable) + "\n");
    var result = await project.RunAsync("test", "--filter", "FullyQualifiedName~A|FullyQualifiedName~B", "-c", "Debug");
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.Equal(new[] { "test", Path.Combine(project.Root, "Sample.slnx"), "-c", "Debug", "--disable-build-servers",
      "--no-build", "--filter", "FullyQualifiedName~A|FullyQualifiedName~B" },
      JsonSerializer.Deserialize<string[]>(File.ReadAllText(output)));
  }

  private static Process? GetProcess( int pid )
  {
    try { return Process.GetProcessById(pid); } catch (ArgumentException) { return null; }
  }
}

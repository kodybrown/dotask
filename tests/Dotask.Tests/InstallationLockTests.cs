using System.Diagnostics;
using DoTask.Cli.Execution;
using DoTask.Cli.Metadata;

namespace DoTask.Tests;

public sealed class InstallationLockTests
{
  [Fact]
  public async Task LockHandoffAcrossProcessesPreservesExclusionAndRemovesTheFile()
  {
    using var project = new TestProject();
    var directory = Path.Combine(project.Root, "installation root");
    var lockFile = Path.Combine(directory, ".dotask-install.lock");
    // Exercise the actual library lock from an independent process, without
    // adding a public API solely for the test.
    var source = project.Write(".tasks/lock-probe.cs", """
      using System.Reflection;
      using DoTask;
      var acquire = typeof(BuildContext).Assembly.GetType("DoTask.InstallationFiles")!
        .GetMethod("Lock", BindingFlags.Static | BindingFlags.NonPublic)!;
      IDisposable lease;
      try { lease = (IDisposable)acquire.Invoke(null, new object[] { args[0] })!; }
      catch (TargetInvocationException ex) when (ex.InnerException is TaskException) {
        Console.WriteLine("busy");
        return 23;
      }
      using (lease) {
        Console.WriteLine("acquired");
        Console.Out.Flush();
        Console.ReadLine();
      }
      return 0;
      """);
    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(90));
    var compiled = await new TargetCompiler().CompileAsync(MetadataReader.Read(source), timeout.Token);
    Assert.True(compiled.Success, compiled.Diagnostics);

    Process Start()
    {
      var start = new ProcessStartInfo(DotnetHost.Find())
      {
        UseShellExecute = false,
        RedirectStandardInput = true,
        RedirectStandardOutput = true,
        RedirectStandardError = true
      };
      start.ArgumentList.Add(compiled.AssemblyPath!);
      start.ArgumentList.Add(directory);
      return Process.Start(start)!;
    }

    using (var parentLease = InstallationFiles.Lock(directory))
    using (var blocked = Start())
    {
      try
      {
        Assert.Equal("busy", await blocked.StandardOutput.ReadLineAsync(timeout.Token));
        await blocked.WaitForExitAsync(timeout.Token);
        Assert.Equal(23, blocked.ExitCode);
        Assert.True(File.Exists(lockFile));
      }
      finally { if (!blocked.HasExited) { blocked.Kill(true); await blocked.WaitForExitAsync(); } }
    }
    Assert.False(File.Exists(lockFile));

    for (var attempt = 0; attempt < 3; attempt++)
    {
      using var owner = Start();
      try
      {
        Assert.Equal("acquired", await owner.StandardOutput.ReadLineAsync(timeout.Token));
        Assert.Throws<TaskException>(() => InstallationFiles.Lock(directory));
        Assert.True(File.Exists(lockFile));
        if (attempt == 2)
        {
          owner.Kill(entireProcessTree: true);
        }
        else
        {
          await owner.StandardInput.WriteLineAsync("release");
        }

        await owner.WaitForExitAsync(timeout.Token);
        if (attempt != 2)
        {
          Assert.Equal(0, owner.ExitCode);
          Assert.False(File.Exists(lockFile));
        }
        // Abrupt termination on Unix may leave an unlocked pathname. It must
        // neither block reacquisition nor survive the next graceful release.
        using (var next = InstallationFiles.Lock(directory))
        {
          Assert.True(File.Exists(lockFile));
        }

        Assert.False(File.Exists(lockFile));
      }
      finally { if (!owner.HasExited) { owner.Kill(true); await owner.WaitForExitAsync(); } }
    }
  }

  [Fact]
  public async Task HandledCancellationReleasesAndRemovesTheLock()
  {
    using var project = new TestProject();
    var directory = Path.Combine(project.Root, "installation root");
    var lockFile = Path.Combine(directory, ".dotask-install.lock");
    using var cancellation = new CancellationTokenSource();
    await Assert.ThrowsAnyAsync<OperationCanceledException>(async () =>
    {
      using var lease = InstallationFiles.Lock(directory);
      Assert.True(File.Exists(lockFile));
      cancellation.Cancel();
      await Task.Delay(Timeout.Infinite, cancellation.Token);
    });
    Assert.False(File.Exists(lockFile));
  }
}

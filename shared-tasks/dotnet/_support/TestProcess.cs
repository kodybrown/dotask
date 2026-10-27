using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using Microsoft.Win32.SafeHandles;
using DoTask;

namespace DoTask.Shared.DotNet;

/// <summary>Bounds one test process tree, including the workers it creates.</summary>
internal static class TestProcess
{
  internal static int Run( string executable, IReadOnlyList<string> arguments, string directory,
    int timeoutSeconds, int memoryLimitMb, string lockName, CancellationToken cancellationToken )
  {
    if (timeoutSeconds is < 1 or > 86400 || memoryLimitMb is < 0 or > 1048576)
      throw new TaskException("Timeout must be 1..86400 seconds; memory-limit-mb must be 0..1048576 (0 disables the memory cap).");
    if (string.IsNullOrEmpty(lockName))
      lockName = "dotask-tests-" + Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(Path.GetFullPath(directory))));
    if (!System.Text.RegularExpressions.Regex.IsMatch(lockName, @"\A[A-Za-z0-9_.-]{1,100}\z"))
      throw new TaskException("lock-name must contain 1..100 letters, digits, dots, underscores or hyphens.");
    // Named mutex ownership is thread-affine. Keep acquisition, process monitoring
    // and release synchronous on this thread, including cancellation cleanup.
    using var mutex = new Mutex(false, OperatingSystem.IsWindows() ? @"Local\" + lockName : lockName);
    var acquired = false;
    try {
      try { acquired = mutex.WaitOne(0); } catch (AbandonedMutexException) { acquired = true; }
      if (!acquired)
        throw new TaskException($"Another test run is active ({lockName}). Wait for it to finish.");
      cancellationToken.ThrowIfCancellationRequested();
      using var job = OperatingSystem.IsWindows() ? CreateJob(memoryLimitMb) : null;
      var start = new ProcessStartInfo(executable) { UseShellExecute = false, WorkingDirectory = directory };
      string? unit = null;
      if (memoryLimitMb > 0 && OperatingSystem.IsLinux() && !HasCgroupLimit(memoryLimitMb)) {
        // Fail visibly if the host cannot enforce a configured cap. systemd owns
        // the complete cgroup, including children whose original parent exits.
        unit = "dotask-tests-" + Guid.NewGuid().ToString("N") + ".scope";
        start.FileName = "systemd-run";
        foreach (var argument in new[] { "--user", "--scope", "--quiet", "--collect", "--unit=" + unit,
          "--property=MemoryMax=" + memoryLimitMb + "M", "--property=MemorySwapMax=0", "--", executable })
          start.ArgumentList.Add(argument);
      } else if (memoryLimitMb > 0 && !OperatingSystem.IsWindows() && !OperatingSystem.IsLinux()) {
        throw new TaskException("An aggregate test memory cap is supported on Windows (Job Object) and Linux (systemd user scope). Set memory-limit-mb to 0 on this host.");
      }
      foreach (var argument in arguments)
        start.ArgumentList.Add(argument);
      using var process = new Process { StartInfo = start };
      var started = false;
      try {
        started = process.Start();
        if (!started)
          throw new TaskException("Could not start the test process.");
        if (job is not null && !AssignProcessToJobObject(job, process.Handle))
          throw new Win32Exception(Marshal.GetLastWin32Error(), "Cannot attach tests to the process-tree Job Object.");
        Console.WriteLine($"Test guard: one run ({lockName}), timeout {timeoutSeconds}s, memory cap {(memoryLimitMb == 0 ? "disabled" : memoryLimitMb + " MB")}.");
        var timer = Stopwatch.StartNew();
        while (!process.WaitForExit(100)) {
          cancellationToken.ThrowIfCancellationRequested();
          if (timer.Elapsed.TotalSeconds >= timeoutSeconds)
            throw new TaskException($"Tests exceeded the {timeoutSeconds} second timeout; the complete process tree was terminated.");
        }
        cancellationToken.ThrowIfCancellationRequested();
        return process.ExitCode;
      } finally {
        if (unit is not null) {
          var stopInfo = new ProcessStartInfo("systemctl");
          stopInfo.UseShellExecute = false;
          foreach (var argument in new[] { "--user", "kill", "--kill-whom=all", unit })
            stopInfo.ArgumentList.Add(argument);
          using var stop = new Process();
          stop.StartInfo = stopInfo;
          if (started) {
            stop.Start();
            if (!stop.WaitForExit(5000))
              stop.Kill(true);
          }
        }
        if (started && !process.HasExited) {
          process.Kill(true);
          process.WaitForExit();
        }
        // Disposing the Windows Job Object also stops any surviving descendants.
      }
    } finally {
      if (acquired)
        mutex.ReleaseMutex();
    }
  }

  private static bool HasCgroupLimit( int memoryLimitMb )
  {
    // CI/container scopes may already enforce a tighter aggregate cap. Do not
    // require a second systemd user manager in that case.
    var membership = File.ReadLines("/proc/self/cgroup").FirstOrDefault(line => line.StartsWith("0::", StringComparison.Ordinal));
    if (membership is null)
      return false;
    var file = "/sys/fs/cgroup" + membership[3..].TrimEnd('/') + "/memory.max";
    return File.Exists(file) && ulong.TryParse(File.ReadAllText(file).Trim(), out var limit)
      && limit <= (ulong)memoryLimitMb * 1024 * 1024;
  }
  private static SafeFileHandle CreateJob( int memoryLimitMb )
  {
    var job = CreateJobObject(IntPtr.Zero, null);
    if (job.IsInvalid)
      throw new Win32Exception(Marshal.GetLastWin32Error());
    var limits = new ExtendedLimitInformation();
    limits.Basic.LimitFlags = 0x00002000; // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
    if (memoryLimitMb > 0) {
      limits.Basic.LimitFlags |= 0x00000200; // JOB_OBJECT_LIMIT_JOB_MEMORY
      limits.JobMemoryLimit = (UIntPtr)((ulong)memoryLimitMb * 1024 * 1024);
    }
    var size = Marshal.SizeOf<ExtendedLimitInformation>();
    var buffer = Marshal.AllocHGlobal(size);
    try {
      Marshal.StructureToPtr(limits, buffer, false);
      if (!SetInformationJobObject(job, 9, buffer, (uint)size))
        throw new Win32Exception(Marshal.GetLastWin32Error());
      return job;
    } catch {
      job.Dispose();
      throw;
    } finally {
      Marshal.FreeHGlobal(buffer);
    }
  }

  [StructLayout(LayoutKind.Sequential)]
  private struct BasicLimitInformation
  {
    public long PerProcessUserTimeLimit, PerJobUserTimeLimit;
    public uint LimitFlags;
    public UIntPtr MinimumWorkingSetSize, MaximumWorkingSetSize;
    public uint ActiveProcessLimit;
    public UIntPtr Affinity;
    public uint PriorityClass, SchedulingClass;
  }
  [StructLayout(LayoutKind.Sequential)]
  private struct IoCounters
  {
    public ulong ReadOperationCount, WriteOperationCount, OtherOperationCount;
    public ulong ReadTransferCount, WriteTransferCount, OtherTransferCount;
  }
  [StructLayout(LayoutKind.Sequential)]
  private struct ExtendedLimitInformation
  {
    public BasicLimitInformation Basic;
    public IoCounters Io;
    public UIntPtr ProcessMemoryLimit, JobMemoryLimit, PeakProcessMemoryUsed, PeakJobMemoryUsed;
  }
  [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  private static extern SafeFileHandle CreateJobObject( IntPtr attributes, string? name );
  [DllImport("kernel32.dll", SetLastError = true)]
  private static extern bool SetInformationJobObject( SafeFileHandle job, uint informationClass, IntPtr information, uint length );
  [DllImport("kernel32.dll", SetLastError = true)]
  private static extern bool AssignProcessToJobObject( SafeFileHandle job, IntPtr process );
}

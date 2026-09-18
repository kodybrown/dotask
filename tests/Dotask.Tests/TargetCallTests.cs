namespace DoTask.Tests;

public sealed class TargetCallTests
{
  [Fact]
  public async Task ExistenceUsesLiveFullNamesAndAliasesWithoutValidatingOrExecutingTargets()
  {
    using var project = new TestProject();
    var context = project.Context();
    Assert.False(await context.TargetExistsAsync("run"));
    var file = project.Target("dotnet run", "File.WriteAllText(\"ran\", \"bad\"); DoesNotCompile();",
      "/// <requires setting=\"missing\" /><option name=\"input\" required=\"true\" />");
    Assert.True(await context.TargetExistsAsync("RUN"));
    Assert.True(await context.TargetExistsAsync("DOTNET-RUN"));
    project.Target("broken", metadata: "/// <summary>Invalid XML");
    Assert.True(await context.TargetExistsAsync("broken"));
    Assert.False(File.Exists(Path.Combine(project.Root, "ran")));
    File.Delete(file);
    Assert.False(await context.TargetExistsAsync("run"));
    Assert.False(await context.TargetExistsAsync("dotnet-run"));
    Assert.Empty(Directory.EnumerateFiles(project.Root, "*.json*"));
  }

  [Fact]
  public async Task AmbiguityIsNotAbsenceAndExplicitNamesTakePrecedence()
  {
    using var project = new TestProject();
    var context = project.Context();
    project.Target("dotnet format");
    project.Target("rust format");
    await Assert.ThrowsAsync<ProcessFailedException>(() => context.TargetExistsAsync("format"));
    var ambiguous = await context.ExecTargetIfExistsAsync("format");
    Assert.Equal(TargetExecutionStatus.Failed, ambiguous.Status);
    Assert.Null(ambiguous.ExitCode);
    Assert.Contains("Ambiguous target", ambiguous.Error);
    project.Target("format");
    Assert.True(await context.TargetExistsAsync("format"));
    project.Target("dotnet-format");
    await Assert.ThrowsAsync<ProcessFailedException>(() => context.TargetExistsAsync("dotnet-format"));
    var duplicate = await context.ExecTargetIfExistsAsync("dotnet-format");
    Assert.Equal(TargetExecutionStatus.Failed, duplicate.Status);
    Assert.Contains("Ambiguous full target name", duplicate.Error);
  }

  [Fact]
  public async Task OptionalExecutionDistinguishesAbsenceSuccessAndNonzeroExitCodes()
  {
    using var project = new TestProject();
    var context = project.Context();
    var missing = await context.ExecTargetIfExistsAsync("missing");
    Assert.Equal(TargetExecutionStatus.NotFound, missing.Status);
    Assert.Null(missing.ExitCode);
    Assert.Null(missing.Error);
    project.Target("dotnet run", """
      var project = BuildContext.Current;
      project.Files.WriteText("result", project.TargetName + ":" + project.Parameters.Get<string>("message"));
      """, "/// <option name=\"message\" required=\"true\" />");
    var success = await context.ExecTargetIfExistsAsync("RUN", new { Message = "hello" });
    Assert.Equal(TargetExecutionStatus.Succeeded, success.Status);
    Assert.Equal(0, success.ExitCode);
    Assert.Null(success.Error);
    Assert.Equal("dotnet-run:hello", File.ReadAllText(Path.Combine(project.Root, "result")));
    project.Target("fail", "Environment.Exit(BuildContext.Current.Parameters.Get<int>(\"code\"));",
      "/// <option name=\"code\" type=\"int\" />");
    foreach (var code in new[] { 1, 7 })
    {
      var failure = await context.ExecTargetIfExistsAsync("fail", new { Code = code });
      Assert.Equal(TargetExecutionStatus.Failed, failure.Status);
      Assert.Equal(code, failure.ExitCode);
      Assert.Contains($"exit code {code}", failure.Error);
    }
    Assert.Empty(Directory.EnumerateFiles(project.Root, "*.json*"));
  }

  [Fact]
  public async Task ExistingButInvalidTargetsReturnFailureWithDiagnostics()
  {
    using var project = new TestProject();
    var context = project.Context();
    project.Target("metadata", metadata: "/// <summary>Invalid XML");
    project.Target("compile", "DoesNotCompile();");
    project.Target("required", metadata: "/// <option name=\"input\" required=\"true\" />");
    project.Target("requirements", metadata: "/// <requires setting=\"missing\" />");
    foreach (var (name, diagnostic) in new[]
    {
      ("metadata", "Metadata error"), ("compile", "CS0103"),
      ("required", "requires --input"), ("requirements", "requires setting 'missing'")
    })
    {
      Assert.True(await context.TargetExistsAsync(name));
      var result = await context.ExecTargetIfExistsAsync(name);
      Assert.Equal(TargetExecutionStatus.Failed, result.Status);
      Assert.Null(result.ExitCode);
      Assert.Contains(diagnostic, result.Error);
    }
  }

  [Fact]
  public async Task OptionalCallsUseTheSelectedDirectoryConfigAndCanonicalCycleDetection()
  {
    using var project = new TestProject();
    project.Target("dotnet child", "throw new Exception(\"Wrong directory\");");
    project.Write(".abc/config.yaml", "settings: { message: shared }\ntargets: { dotnet-child: { defaults: { name: default } } }");
    project.Write(".abc/dotnet child.cs", """
      using DoTask;
      /// <option name="name" />
      public static class Target
      {
        public static void Main()
        {
          var project = BuildContext.Current;
          project.Files.WriteText("result", project.Config.Get<string>("message") + ":" + project.Parameters.Get<string>("name"));
        }
      }
      """);
    project.Write(".abc/dotnet outer.cs", """
      using DoTask;
      public static class Target
      {
        public static async Task Main()
        {
          var project = BuildContext.Current;
          Console.WriteLine("exists=" + await project.TargetExistsAsync("CHILD"));
          var child = await project.ExecTargetIfExistsAsync("CHILD");
          Console.WriteLine(child.Status);
          var cycle = await project.ExecTargetIfExistsAsync("dotnet-outer");
          Console.WriteLine(cycle.Status + ":" + cycle.Error);
        }
      }
      """);
    var result = await project.RunAsync("--use-dir", ".abc", "outer");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Contains("exists=True", result.StandardOutput);
    Assert.Contains("Succeeded", result.StandardOutput);
    Assert.Contains("Failed:Target cycle: dotnet-outer -> dotnet-outer", result.StandardOutput);
    Assert.Equal("shared:default", File.ReadAllText(Path.Combine(project.Root, "result")));
  }

  [Fact]
  public async Task CancellationAndInvalidInputsAreNotReportedAsMissing()
  {
    using var project = new TestProject();
    var context = project.Context();
    await Assert.ThrowsAsync<ArgumentException>(() => context.TargetExistsAsync(" "));
    await Assert.ThrowsAsync<ArgumentException>(() => context.ExecTargetIfExistsAsync(" "));
    using var cancelled = new CancellationTokenSource();
    cancelled.Cancel();
    await Assert.ThrowsAnyAsync<OperationCanceledException>(() => context.TargetExistsAsync("missing", cancelled.Token));
    await Assert.ThrowsAnyAsync<OperationCanceledException>(() => context.ExecTargetIfExistsAsync("missing", cancellationToken: cancelled.Token));
    Assert.Empty(Directory.EnumerateFiles(project.Root, "*.json*"));
    Directory.Delete(project.Tasks);
    await Assert.ThrowsAsync<ProcessFailedException>(() => context.TargetExistsAsync("missing"));
    await Assert.ThrowsAsync<ProcessFailedException>(() => context.ExecTargetIfExistsAsync("missing"));
  }

  [Fact]
  public async Task OptionalExecutionCancellationStopsTheChildAndCleansRequestFiles()
  {
    using var project = new TestProject();
    project.Target("slow", """
      File.WriteAllText("started", "yes");
      await Task.Delay(TimeSpan.FromSeconds(30));
      File.WriteAllText("finished", "bad");
      """, async: true);
    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(60));
    using var cancel = CancellationTokenSource.CreateLinkedTokenSource(timeout.Token);
    var invocation = project.Context().ExecTargetIfExistsAsync("slow", cancellationToken: cancel.Token);
    try
    {
      while (!File.Exists(Path.Combine(project.Root, "started")))
      {
        if (invocation.IsCompleted)
        {
          Assert.Fail($"Target stopped before starting: {await invocation}");
        }
        await Task.Delay(25, timeout.Token);
      }
    }
    finally
    {
      cancel.Cancel();
      await Assert.ThrowsAnyAsync<OperationCanceledException>(() => invocation);
    }
    Assert.False(File.Exists(Path.Combine(project.Root, "finished")));
    Assert.Empty(Directory.EnumerateFiles(project.Root, "*.json*"));
    Assert.Equal([project.Tasks], Directory.GetDirectories(project.Root));
  }

  [Fact]
  public async Task VerifyRunsAvailableChecksInOrderForwardsOptionsAndStopsOnFailure()
  {
    using var project = new TestProject();
    await CopyVerifyAsync(project);
    project.Target("dotask-official/dotnet/check", "File.AppendAllText(\"order\", \"check;\");");
    project.Target("dotask-official/dotnet/test", "File.AppendAllText(\"order\", BuildContext.Current.Parameters.Get<string>(\"configuration\") + \";\");",
      "/// <option name=\"configuration\" choices=\"Debug,Release\" />");
    project.Target("dotask-official/dotnet/format", "File.AppendAllText(\"order\", BuildContext.Current.Parameters.Get<bool>(\"verify\").ToString());",
      "/// <option name=\"verify\" type=\"bool\" />");
    var result = await project.RunAsync("verify", "-c", "Debug");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Equal("check;Debug;True", File.ReadAllText(Path.Combine(project.Root, "order")));
    File.Delete(Path.Combine(project.Root, "order"));
    project.Target("dotask-official/dotnet/test", "Environment.Exit(7);", "/// <option name=\"configuration\" />");
    result = await project.RunAsync("verify");
    Assert.Equal(7, result.ExitCode);
    Assert.Equal("check;", File.ReadAllText(Path.Combine(project.Root, "order")));
  }

  [Fact]
  public async Task VerifySkipsMissingTargetsButFailsWhenNothingCanBeVerified()
  {
    using var project = new TestProject();
    await CopyVerifyAsync(project);
    var empty = await project.RunAsync("verify");
    Assert.Equal(1, empty.ExitCode);
    Assert.Contains("No verification targets found", empty.StandardError);
    project.Target("dotask-official/dotnet/format", "Console.WriteLine(BuildContext.Current.Parameters.Get<bool>(\"verify\"));",
      "/// <option name=\"verify\" type=\"bool\" />");
    var one = await project.RunAsync("dotnet-verify");
    Assert.True(one.ExitCode == 0, one.StandardError);
    Assert.Contains("Skipping dotask-official/dotnet/check: target not found.", one.StandardOutput);
    Assert.Contains("Skipping dotask-official/dotnet/test: target not found.", one.StandardOutput);
    Assert.Contains("True", one.StandardOutput);
  }

  private static async Task CopyVerifyAsync(TestProject project)
  {
    using var stream = typeof(TargetCallTests).Assembly.GetManifestResourceStream("DoTask.Tests.Targets.DotnetVerify.cs")!;
    using var reader = new StreamReader(stream);
    project.Write(".tasks/dotask-official/dotnet/verify.cs", await reader.ReadToEndAsync());
  }
}

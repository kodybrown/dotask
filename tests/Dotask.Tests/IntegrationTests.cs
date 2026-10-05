using System.Text.Json;
using DoTask.Cli;
using DoTask.Cli.Execution;
using DoTask.Cli.Metadata;

namespace DoTask.Tests;

public sealed class IntegrationTests
{
  [Fact]
  public async Task ReusableDotnetCheckOnlyQueriesToolsWithoutLoadingTheProjectOrCallingOtherTargets()
  {
    using var project = new TestProject();
    using var stream = typeof(IntegrationTests).Assembly.GetManifestResourceStream("Shared/dotnet/check.cs")!;
    using var reader = new StreamReader(stream);
    project.Write(".tasks/dotnet check.cs", await reader.ReadToEndAsync());
    project.Write("Broken.csproj", "This project must never be evaluated by check.");
    project.Write(".tasks/config.yaml", "settings: { solution: Broken.csproj }");
    project.Target("dotnet test", "File.WriteAllText(\"tests-ran\", \"bad\");",
      "// dotask: 1\n// options:\n//   - {\"name\": \"configuration\"}\n// end-dotask\n");
    project.Target("dotnet format", "File.WriteAllText(\"format-ran\", \"bad\");",
      "// dotask: 1\n// options:\n//   - {\"name\": \"verify\", \"type\": \"bool\"}\n// end-dotask\n");
    var result = await project.RunAsync("check");
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.Equal("", result.StandardError);
    Assert.Contains(".NET SDK:", result.StandardOutput);
    Assert.Contains("MSBuild:", result.StandardOutput);
    Assert.Contains(".NET formatter:", result.StandardOutput);
    Assert.False(File.Exists(Path.Combine(project.Root, "tests-ran")));
    Assert.False(File.Exists(Path.Combine(project.Root, "format-ran")));
    Assert.False(Directory.Exists(Path.Combine(project.Root, "obj")));
  }

  [Fact]
  public async Task HelpShowsSourceDescriptionsAndCompilationErrorsAreReportedOnExecution()
  {
    using var project = new TestProject();
    project.Target("broken", "DoesNotExist();", "// dotask: 1\n// description: \"A target with a compiler error.\"\n// end-dotask\n");
    project.Target("good", "Console.WriteLine(\"TARGET-RAN\");", "// dotask: 1\n// description: \"A good target.\"\n// end-dotask\n");
    var help = await project.RunAsync();
    Assert.Equal(0, help.ExitCode);
    Assert.Contains("A target with a compiler error.", help.StandardOutput);
    Assert.DoesNotContain("CS0103", help.StandardOutput);
    Assert.Contains("A good target.", help.StandardOutput);
    Assert.DoesNotContain("TARGET-RAN", help.StandardOutput);
    var detailedHelp = await project.RunAsync("help", "broken");
    Assert.Equal(0, detailedHelp.ExitCode);
    Assert.Contains("A target with a compiler error.", detailedHelp.StandardOutput);
    Assert.DoesNotContain("CS0103", detailedHelp.StandardOutput);
    var run = await project.RunAsync("GOOD");
    Assert.Equal(0, run.ExitCode);
    Assert.Contains("TARGET-RAN", run.StandardOutput);
    var broken = await project.RunAsync("broken");
    Assert.NotEqual(0, broken.ExitCode);
    Assert.Contains("CS0103", broken.StandardError);
  }

  [Fact]
  public async Task NestedCallsShareConfigAndInvocationButHaveIndependentParametersAndSourcePaths()
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", "settings: { Message: 'Hello=World' }");
    project.Target("outer", """
      var project = BuildContext.Current;
      await project.ExecTargetAsync("INNER", new { OS = "linux", Message = "A=B 'quoted'" });
      await project.ExecTargetAsync("inner", new { OS = "windows", Message = "second" });
      """, "// dotask: 1\n// options:\n//   - {\"name\": \"outerOnly\", \"default\": \"not-forwarded\"}\n// end-dotask\n", async: true);
    project.Target("inner", """
      var project = BuildContext.Current;
      Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new {
        project.RootDirectory, project.InvocationDirectory, project.TargetFile, project.TargetDirectory,
        Host = project.OS.ToString(), OutputOS = project.Parameters.Get<string>("OS"),
        Message = project.Parameters.Get<string>("message"), Config = project.Config.Get<string>("message"),
        HasOuterOption = project.Parameters.Contains("outerOnly"), Cwd = Environment.CurrentDirectory
      }));
      """, """
      // dotask: 1
      // options:
      //   - {"name": "OS", "choices": ["windows", "linux", "macos"], "required": true}
      //   - {"name": "message", "required": true}
      // end-dotask
      """);
    var nested = Path.Combine(project.Root, "src", "nested");
    Directory.CreateDirectory(nested);
    var result = await project.RunFromAsync(nested, "outer");
    Assert.True(result.ExitCode == 0, result.StandardError);
    var lines = result.StandardOutput.Split('\n', StringSplitOptions.RemoveEmptyEntries);
    Assert.Equal(2, lines.Length);
    using var first = JsonDocument.Parse(lines[0]);
    var data = first.RootElement;
    Assert.Equal(project.Root, data.GetProperty("RootDirectory").GetString());
    Assert.Equal(nested, data.GetProperty("InvocationDirectory").GetString());
    Assert.Equal(project.Tasks, data.GetProperty("TargetDirectory").GetString());
    Assert.Equal(Path.Combine(project.Tasks, "inner.cs"), data.GetProperty("TargetFile").GetString());
    Assert.Equal(project.Root, data.GetProperty("Cwd").GetString());
    Assert.Equal("linux", data.GetProperty("OutputOS").GetString());
    Assert.Equal("A=B 'quoted'", data.GetProperty("Message").GetString());
    Assert.Equal("Hello=World", data.GetProperty("Config").GetString());
    Assert.False(data.GetProperty("HasOuterOption").GetBoolean());
    using var second = JsonDocument.Parse(lines[1]);
    Assert.Equal("windows", second.RootElement.GetProperty("OutputOS").GetString());
    Assert.Equal(data.GetProperty("Host").GetString(), second.RootElement.GetProperty("Host").GetString());
  }

  [Fact]
  public async Task GroupedTargetsCompileAndShareIdentityAndDefaultsAcrossAliasesAndNestedCalls()
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", "targets: { dotnet-run: { defaults: { configuration: Release } }, run: { defaults: { configuration: Ignored } } }");
    project.Target("dotnet run", """
      var project = BuildContext.Current;
      Console.WriteLine(project.TargetName + ":" + project.Parameters.Get<string>("configuration") + ":" + Path.GetFileName(project.TargetFile));
      """, "// dotask: 1\n// options:\n//   - {\"name\": \"configuration\", \"choices\": [\"Debug\", \"Release\"], \"default\": \"Debug\"}\n// end-dotask\n");
    var shortRun = await project.RunAsync("RUN");
    var fullRun = await project.RunAsync("DOTNET-RUN");
    Assert.True(shortRun.ExitCode == 0, shortRun.StandardError);
    Assert.True(fullRun.ExitCode == 0, fullRun.StandardError);
    Assert.Equal("dotnet-run:Release:dotnet run.cs" + Environment.NewLine, shortRun.StandardOutput);
    Assert.Equal(shortRun.StandardOutput, fullRun.StandardOutput);
    project.Target("dotnet check", """
      await BuildContext.Current.ExecTargetAsync("run");
      await BuildContext.Current.ExecTargetAsync("dotnet-run", new { Configuration = "Debug" });
      """, async: true);
    var nested = await project.RunAsync("check");
    Assert.True(nested.ExitCode == 0, nested.StandardError);
    Assert.Contains("dotnet-run:Release:dotnet run.cs", nested.StandardOutput);
    Assert.Contains("dotnet-run:Debug:dotnet run.cs", nested.StandardOutput);
    project.Target("dotnet cycle", "await BuildContext.Current.ExecTargetAsync(\"dotnet-cycle\");", async: true);
    var cycle = await project.RunAsync("cycle");
    Assert.Equal(1, cycle.ExitCode);
    Assert.Contains("dotnet-cycle -> dotnet-cycle", cycle.StandardError);
  }

  [Fact]
  public async Task CyclesAndFailedProcessesStopTargetsWithUsefulErrors()
  {
    using var project = new TestProject();
    project.Target("a", "await BuildContext.Current.ExecTargetAsync(\"b\");", async: true);
    project.Target("b", "await BuildContext.Current.ExecTargetAsync(\"A\");", async: true);
    var cycle = await project.RunAsync("a");
    Assert.Equal(1, cycle.ExitCode);
    Assert.Contains("a -> b -> a", cycle.StandardError);
    project.Target("fail", "Environment.Exit(7);");
    project.Target("parent", """
      await BuildContext.Current.ExecTargetAsync("fail");
      Console.WriteLine("SHOULD-NOT-RUN");
      """, async: true);
    var failure = await project.RunAsync("parent");
    Assert.Equal(7, failure.ExitCode);
    Assert.DoesNotContain("SHOULD-NOT-RUN", failure.StandardOutput);
    Assert.Contains("exit code 7", failure.StandardError);
  }

  [Theory]
  [InlineData(false)]
  [InlineData(true)]
  public async Task TargetCompilationIsolatesConsumerMsbuildButSupportsIncludedFiles( bool native )
  {
    using var project = new TestProject { NativeRunner = native };
    project.Write("Directory.Build.props", "<Project><PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>");
    project.Write("Directory.Build.targets", "<Project><Target Name=\"Poison\" BeforeTargets=\"Build\"><Error Text=\"CONSUMER-BUILD-RAN\" /></Target></Project>");
    project.Write(".tasks/shared/Words.cs", "public static class Words { public static string Message => \"included\"; }");
    project.Write(".tasks/include.cs", """
      // dotask: 1
      // description: "Use a shared file."
      // end-dotask
      #:include shared/Words.cs
      using DoTask;

      public static class Target
      {
        public static void Main() => Console.WriteLine(Words.Message + ":" + BuildContext.Current.TargetName);
      }
      """);
    var result = await project.RunAsync("include");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Contains("included:include", result.StandardOutput);
    project.Write(".tasks/shared/Words.cs", "public static class Words { public static string Message => \"changed\"; }");
    var changed = await project.RunAsync("include");
    Assert.True(changed.ExitCode == 0, changed.StandardError);
    Assert.Contains("changed:include", changed.StandardOutput);
    Assert.False(Directory.Exists(Path.Combine(project.Tasks, "obj")));
  }

  [Fact]
  public async Task ExplicitDirectoryOverrideIsHonoredByExecutionHelpAndCompletion()
  {
    using var project = new TestProject();
    var file = project.Target("custom", "Console.WriteLine(BuildContext.Current.TargetName);", "// dotask: 1\n// description: \"Custom directory.\"\n// end-dotask\n");
    var custom = Path.Combine(project.Root, ".abc");
    Directory.CreateDirectory(custom);
    File.Move(file, Path.Combine(custom, "custom.cs"));
    var execution = await project.RunAsync("--use-dir", ".abc", "custom");
    Assert.True(execution.ExitCode == 0, execution.StandardError);
    Assert.Contains("custom", execution.StandardOutput);
    var help = await project.RunAsync("help", "custom", "--use-dir=.abc");
    Assert.Contains("Custom directory.", help.StandardOutput);
    var completion = await project.RunAsync("__complete", "--line", "dotask --use-dir .abc c");
    Assert.Contains("custom\ttarget", completion.StandardOutput);
    var missing = await project.RunAsync("--use-dir", ".missing", "custom");
    Assert.Equal(1, missing.ExitCode);
    Assert.Contains("does not exist", missing.StandardError);
  }

  [Fact]
  public async Task CompletionDoesNotCompileRestoreOrExecuteTargets()
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", "invalid: [");
    project.Write(".tasks/run.cs", """
      // dotask: 1
      // description: "A completion-only test."
      // options:
      //   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"]}
      // end-dotask
      #:package This.Package.Must.Never.Be.Restored@1.0.0

      public static class Target
      {
        public static void Main() { ThisDoesNotCompile(); }
      }
      """);
    var result = await project.RunAsync("__complete", "--line", "dotask run -c r");
    Assert.Equal(0, result.ExitCode);
    Assert.Contains("Release\tvalue", result.StandardOutput);
    Assert.Equal("", result.StandardError);
  }

  [Fact]
  public async Task RequirementsAndArgumentErrorsPreventTargetSideEffects()
  {
    using var project = new TestProject();
    project.Target("guarded", "File.WriteAllText(\"marker\", \"bad\");", """
      // dotask: 1
      // options:
      //   - {"name": "name", "required": true}
      // requires:
      //   - {"kind": "setting", "value": "missing"}
      // end-dotask
      """);
    var argument = await project.RunAsync("guarded");
    Assert.Contains("requires --name", argument.StandardError);
    var requirement = await project.RunAsync("guarded", "name=test");
    Assert.Contains("requires setting 'missing'", requirement.StandardError);
    Assert.False(File.Exists(Path.Combine(project.Root, "marker")));
  }

  [Theory]
  [InlineData(false)]
  [InlineData(true)]
  public async Task NativeFileBasedPackageReferencesAreSupported( bool native )
  {
    using var project = new TestProject { NativeRunner = native };
    project.Write(".tasks/package.cs", """
      #:package YamlDotNet@16.3.0
      Console.WriteLine(typeof(YamlDotNet.RepresentationModel.YamlStream).Name);
      """);
    var result = await project.RunAsync("package");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Contains("YamlStream", result.StandardOutput);
  }

  [Theory]
  [InlineData(false)]
  [InlineData(true)]
  public async Task NativeProjectReferencesKeepTheirOwnBuildProperties( bool native )
  {
    using var project = new TestProject { NativeRunner = native };
    project.WriteBuildProperties("shared/Directory.Build.props",
      "<TargetFramework>net10.0</TargetFramework><DefineConstants>LIBRARY_FLAG</DefineConstants>");
    project.Write("shared/Library.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\" />");
    project.Write("shared/Library.cs", """
      public static class Shared
      {
      #if LIBRARY_FLAG
        public const string Value = "library-built";
      #endif
      }
      """);
    project.Write(".tasks/reference.cs", """
      #:project ../shared/Library.csproj
      Console.WriteLine(Shared.Value);
      """);
    var result = await project.RunAsync("reference");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Contains("library-built", result.StandardOutput);
  }

  [Fact]
  public async Task DirectCompilationAcceptsPortableSeparatorsAndRecordsOutput()
  {
    using var project = new TestProject();
    var file = project.Target("nested/run", "Console.WriteLine(typeof(BuildContext).Name);");
    // Keep this input unnormalized even though the fixture normally returns
    // native paths: direct compiler callers can supply portable separators.
    var target = MetadataReader.Read(file.Replace('\\', '/'));
    var result = await new TargetCompiler().CompileAsync(target, CancellationToken.None);
    Assert.True(result.Success, result.Diagnostics);
    Assert.True(File.Exists(result.AssemblyPath));
  }

  [Fact]
  public async Task ConcurrentCompilationUsesConsistentSnapshots()
  {
    using var project = new TestProject();
    var target = MetadataReader.Read(project.Target("run", "Console.WriteLine(\"concurrent\");"));
    var compiler = new TargetCompiler(cacheRoot: Path.Combine(project.Root, "cache"));
    var results = await Task.WhenAll(Enumerable.Range(0, 2).Select(i => compiler.CompileAsync(target, CancellationToken.None,
      Path.Combine(project.Root, "snapshot" + i))));
    Assert.All(results, result => Assert.True(result.Success, result.Diagnostics));
    Assert.NotEqual(results[0].AssemblyPath, results[1].AssemblyPath);
    Assert.Equal(File.ReadAllBytes(results[0].AssemblyPath!), File.ReadAllBytes(results[1].AssemblyPath!));
  }
}

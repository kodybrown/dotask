using DoTask.Cli;
using DoTask.Cli.Completion;
using DoTask.Cli.Configuration;
using DoTask.Cli.Execution;
using DoTask.Cli.Metadata;

namespace DoTask.Tests;

public sealed class HelpAndShellTests
{
  [Theory]
  [InlineData("")]
  [InlineData("help")]
  [InlineData("help run")]
  [InlineData("run --help")]
  [InlineData("run -h")]
  public async Task HelpReadsMetadataAndDefaultsWithoutSdkRestoreCompilationOrExecution( string command )
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", "targets: { run: { defaults: { configuration: Release } } }");
    project.Write(".tasks/run.cs", """
      #:package This.Package.Must.Never.Be.Restored@1.0.0
      /// <summary>Describe this target without building it.</summary>
      /// <option name="configuration" alias="c" choices="Debug,Release" default="Debug">Build mode.</option>
      /// <option name="input" required="true" />
      /// <requires setting="missing" />
      public static class Target
      {
        public static void Main()
        {
          File.WriteAllText("target-ran", "bad");
          ThisDoesNotCompile();
        }
      }
      """);
    var unusableHost = project.Write("not-a-dotnet-host", "The SDK must not be invoked for help.");
    var temporary = Path.Combine(project.Root, "temp");
    Directory.CreateDirectory(temporary);
    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(30));
    var result = await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = DotnetHost.Find(),
      Arguments = [typeof(CliApplication).Assembly.Location, .. command.Split(' ', StringSplitOptions.RemoveEmptyEntries)],
      WorkingDirectory = project.Root,
      Environment = new Dictionary<string, string?> {
        ["DOTNET_HOST_PATH"] = unusableHost,
        ["TMPDIR"] = temporary,
        ["TMP"] = temporary,
        ["TEMP"] = temporary
      },
      CaptureOutput = true,
      ThrowOnError = false
    }, timeout.Token);
    Assert.Equal(0, result.ExitCode);
    Assert.Equal("", result.StandardError);
    Assert.Contains("Describe this target without building it.", result.StandardOutput);
    Assert.Contains("--configuration, -c", result.StandardOutput);
    Assert.Contains("default: Release", result.StandardOutput);
    Assert.Contains("--input", result.StandardOutput);
    Assert.False(File.Exists(Path.Combine(project.Root, "target-ran")));
    Assert.False(Directory.Exists(Path.Combine(temporary, "_dotnet", "dotask")));
  }

  [Theory]
  [InlineData("--help")]
  [InlineData("-h")]
  [InlineData("help --help")]
  public async Task GlobalHelpDoesNotReadOrLocateTheProject( string command )
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", "invalid: [");
    project.Target("project-only-target", metadata: "/// <summary>Project-only description.</summary>");
    var expected = new StringWriter();
    HelpWriter.Usage(new HelpText(expected));
    foreach (var args in new[]
    {
      command.Split(' '),
      command.Split(' ').Concat(["--use-dir", "missing-task-directory"]).ToArray()
    }) {
      var output = new StringWriter();
      var error = new StringWriter();
      var code = await CliApplication.RunAsync(args, project.Root, output, error);
      Assert.Equal(0, code);
      Assert.Equal("", error.ToString());
      Assert.Equal(expected.ToString(), output.ToString());
      Assert.DoesNotContain(project.Root, output.ToString());
      Assert.DoesNotContain("project-only", output.ToString());
    }
    var withoutProject = new StringWriter();
    Assert.Equal(0, await CliApplication.RunAsync(command.Split(' '), Path.GetPathRoot(project.Root)!, withoutProject, new StringWriter()));
    Assert.Equal(expected.ToString(), withoutProject.ToString());
  }

  [Theory]
  [InlineData("")]
  [InlineData("help")]
  public async Task ProjectSummaryShowsIdentitySettingsAndTargetOptionsWithoutCliUsage( string command )
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", """
      name: Example project
      description: Project description.
      settings:
        app:
          retries: 3
          enabled: true
          names: [one, two]
          empty: {}
          unset: null
          message: "First line.\nSecond line."
      targets:
        run:
          defaults:
            configuration: Release
      """);
    project.Target("run", metadata: """
      /// <summary>Run the project.</summary>
      /// <option name="configuration" alias="c" choices="Debug,Release" default="Debug">Build mode.</option>
      """);
    var result = await project.RunAsync(command.Split(' ', StringSplitOptions.RemoveEmptyEntries));
    Assert.Equal(0, result.ExitCode);
    Assert.Equal("", result.StandardError);
    Assert.StartsWith("Example project" + Environment.NewLine + "Project description.", result.StandardOutput);
    Assert.Contains("Settings:", result.StandardOutput);
    Assert.Contains("  tasks: ./.tasks", result.StandardOutput);
    Assert.Contains("  app:", result.StandardOutput);
    Assert.Contains("    retries: 3", result.StandardOutput);
    Assert.Contains("    enabled: true", result.StandardOutput);
    Assert.Contains("    names: [\"one\",\"two\"]", result.StandardOutput);
    Assert.Contains("    empty: {}", result.StandardOutput);
    Assert.Contains("    unset: null", result.StandardOutput);
    Assert.Contains("    message: 'First line.\\nSecond line.'", result.StandardOutput);
    Assert.True(result.StandardOutput.IndexOf("    unset:", StringComparison.Ordinal)
      < result.StandardOutput.IndexOf("  tasks:", StringComparison.Ordinal));
    Assert.Contains("\nTargets:", result.StandardOutput);
    Assert.DoesNotContain("Targets in ", result.StandardOutput);
    Assert.DoesNotContain(project.Tasks, result.StandardOutput);
    Assert.Contains("Run the project.", result.StandardOutput);
    Assert.Contains("Target options:", result.StandardOutput);
    Assert.Contains("--configuration, -c <Debug|Release>  Build mode. (default: Release)", result.StandardOutput);
    Assert.DoesNotContain("Targets: run", result.StandardOutput);
    Assert.Contains("See `dotask help <target>` for detailed information on each target.", result.StandardOutput);
    Assert.DoesNotContain("Usage: dotask", result.StandardOutput);
    Assert.DoesNotContain("Global options:", result.StandardOutput);
    Assert.DoesNotContain("dotask - portable C# tasks", result.StandardOutput);
  }

  [Fact]
  public async Task ProjectSummaryUsesDirectoryNameWithoutConfigAndHonorsDirectoryOverride()
  {
    using var project = new TestProject();
    var fallback = await project.RunAsync();
    Assert.Equal(0, fallback.ExitCode);
    Assert.StartsWith(Path.GetFileName(project.Root) + Environment.NewLine, fallback.StandardOutput);
    Assert.Contains("  tasks: ./.tasks", fallback.StandardOutput);
    Assert.Contains("(no C# targets)", fallback.StandardOutput);
    project.Write(".abc/config.yaml", "name: Custom project\ndescription: Selected task directory.");
    var selected = await project.RunAsync("--use-dir", ".abc");
    Assert.Equal(0, selected.ExitCode);
    Assert.StartsWith("Custom project" + Environment.NewLine + "Selected task directory.", selected.StandardOutput);
    Assert.Contains("  tasks: ./.abc", selected.StandardOutput);
    Assert.Contains("\nTargets:", selected.StandardOutput);
  }

  [Theory]
  [InlineData("")]
  [InlineData("help")]
  public async Task MissingProjectReportsAnErrorWithoutCliUsage( string command )
  {
    using var project = new TestProject();
    Directory.Delete(project.Tasks);
    var result = await project.RunAsync(command.Split(' ', StringSplitOptions.RemoveEmptyEntries));
    Assert.Equal(1, result.ExitCode);
    Assert.Equal("", result.StandardOutput);
    Assert.Contains("No .dotasks.yaml or .tasks directory found.", result.StandardError);
    Assert.DoesNotContain("Usage:", result.StandardError);
  }

  [Fact]
  public async Task HelpReportsMetadataAndDefaultErrorsWithoutHidingOtherTargets()
  {
    using var project = new TestProject();
    project.Target("badmetadata", metadata: "/// <option name=\"help\" />");
    project.Target("baddefault", metadata: "/// <option name=\"configuration\" choices=\"Debug,Release\" />");
    project.Target("good", metadata: "/// <summary>A valid description.</summary>");
    project.Write(".tasks/config.yaml", "targets: { baddefault: { defaults: { configuration: Unknown } } }");
    var listing = await project.RunAsync("help");
    Assert.Equal(0, listing.ExitCode);
    Assert.Contains("Metadata error:", listing.StandardOutput);
    Assert.Contains("Invalid value for --configuration", listing.StandardOutput);
    Assert.Contains("A valid description.", listing.StandardOutput);
    var detailed = await project.RunAsync("help", "baddefault");
    Assert.Equal(0, detailed.ExitCode);
    Assert.Contains("Invalid value for --configuration", detailed.StandardOutput);
  }

  [Fact]
  public void CombinedHelpMergesDescriptionsAndDefaultsForCompatibleOptions()
  {
    using var project = new TestProject();
    var build = MetadataReader.Read(project.Target("build", metadata: "/// <option name=\"configuration\" alias=\"c\" choices=\"Debug,Release\" default=\"Debug\">Mode.</option>"));
    var test = MetadataReader.Read(project.Target("test", metadata: "/// <option name=\"CONFIGURATION\" alias=\"C\" choices=\"Debug,Release\" default=\"Debug\">Mode.</option>"));
    var publish = MetadataReader.Read(project.Target("publish", metadata: "/// <option name=\"configuration\" alias=\"c\" choices=\"Debug,Release\" default=\"Debug\">Mode.</option>"));
    var clean = MetadataReader.Read(project.Target("clean", metadata: "/// <option name=\"configuration\" alias=\"c\" choices=\"release,debug\" default=\"debug\">Mode to clean.</option>"));
    var custom = MetadataReader.Read(project.Target("custom", metadata: "/// <option name=\"configuration\" alias=\"c\" choices=\"Debug,Release\" />"));
    project.Write(".tasks/config.yaml", "targets: { publish: { defaults: { configuration: Release } } }");
    var output = new StringWriter();
    var config = ProjectConfiguration.Load(project.Tasks);
    HelpWriter.CombinedOptions(new HelpText(output), [build, test, publish, clean, custom], config);
    Assert.Contains("--configuration, -c <Debug|Release>  Mode.", output.ToString());
    Assert.DoesNotContain("Targets:", output.ToString());
    Assert.DoesNotContain("Defaults:", output.ToString());
    Assert.DoesNotContain("(default:", output.ToString());
    Assert.Single(output.ToString().Split('\n'), line => line.Contains("--configuration"));
    var detailed = new StringWriter();
    HelpWriter.Target(new HelpText(detailed), clean, config, null, clean.Name, project.Root);
    Assert.Contains("Mode to clean. (default: debug)", detailed.ToString());
    Assert.DoesNotContain("Targets:", detailed.ToString());
  }

  [Fact]
  public void CombinedHelpKeepsIncompatibleOptionContractsSeparate()
  {
    using var project = new TestProject();
    var variants = new[] { "", "alias=\"m\"", "type=\"int\"", "required=\"true\"", "choices=\"one,two\"", "completion=\"file\"" };
    var targets = variants.Select(( attributes, i ) => MetadataReader.Read(project.Target($"task{i}",
      metadata: $"/// <option name=\"mode\" {attributes}>Mode.</option>"))).ToArray();
    var output = new StringWriter();
    HelpWriter.CombinedOptions(new HelpText(output), targets, ProjectConfiguration.Load(project.Tasks));
    var text = output.ToString().Replace("\r\n", "\n");
    Assert.Equal(variants.Length, text.Split('\n').Count(line => line.Contains("--mode")));
    Assert.DoesNotContain("Targets:", text);
    Assert.Contains("--mode, -m <string>", text);
    Assert.Contains("--mode <int>", text);
    Assert.Contains("--mode <one|two>", text);
    Assert.Contains("(required)", text);
    Assert.DoesNotContain("\n\n", text);
  }

  [Fact]
  public async Task HelpListsEachGroupedTargetOnceWithOnlyUsableAliasesAndFullNameDefaults()
  {
    using var project = new TestProject();
    project.Target("dotnet run", metadata: """
      /// <summary>Run the app.</summary>
      /// <option name="configuration" alias="c" choices="Debug,Release" default="Debug" />
      """);
    project.Write(".tasks/config.yaml", "targets: { dotnet-run: { defaults: { configuration: Release } } }");
    var listing = await project.RunAsync();
    Assert.Equal(0, listing.ExitCode);
    var targetLine = Assert.Single(listing.StandardOutput.Split('\n'), line => line.Contains("Run the app."));
    Assert.StartsWith("  run ", targetLine);
    Assert.DoesNotContain("dotnet-run", targetLine);
    Assert.Contains("default: Release", listing.StandardOutput);
    var shortHelp = await project.RunAsync("help", "RUN");
    var fullHelp = await project.RunAsync("DOTNET-RUN", "--help");
    Assert.Equal(0, shortHelp.ExitCode);
    Assert.Equal(0, fullHelp.ExitCode);
    Assert.StartsWith("Target:" + Environment.NewLine + "  run ", shortHelp.StandardOutput);
    Assert.Contains(Environment.NewLine + "                  Source: './.tasks/dotnet run.cs'", shortHelp.StandardOutput);
    Assert.DoesNotContain("run (dotnet-run)", shortHelp.StandardOutput);
    Assert.Equal(shortHelp.StandardOutput, fullHelp.StandardOutput);
    project.Target("rust run", metadata: "/// <summary>Run Rust.</summary>");
    var ambiguous = await project.RunAsync("help", "run");
    Assert.Equal(1, ambiguous.ExitCode);
    Assert.Contains("dotask dotnet-run", ambiguous.StandardError);
    Assert.Contains("dotask rust-run", ambiguous.StandardError);
    var both = await project.RunAsync();
    Assert.Equal(0, both.ExitCode);
    Assert.DoesNotContain("run (", both.StandardOutput);
    Assert.Contains("Run the app.", both.StandardOutput);
    Assert.Contains("Run Rust.", both.StandardOutput);
  }

  [Theory]
  [InlineData("bash", "complete -F")]
  [InlineData("zsh", "compdef")]
  [InlineData("fish", "complete -c dotask")]
  [InlineData("powershell", "Register-ArgumentCompleter")]
  public void ShellScriptsAreEmbeddedAndAvailableWithoutProject( string shell, string marker )
  {
    var output = new StringWriter();
    CompletionCommand.PrintScript(shell, output);
    Assert.Contains(marker, output.ToString());
    Assert.Contains("__complete", output.ToString());
  }

  [Fact]
  public async Task BashAdapterHandlesSeparatedAndEqualsValues()
  {
    if (OperatingSystem.IsWindows() || !File.Exists("/bin/bash")) {
      return;
    }
    using var project = new TestProject();
    project.Target("run", metadata: "/// <option name=\"configuration\" alias=\"c\" choices=\"Debug,Release\" />");
    var script = new StringWriter();
    CompletionCommand.PrintScript("bash", script);
    project.Write("completion.bash", script.ToString());
    var result = await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = "/bin/bash",
      Arguments = ["--noprofile", "--norc", "-c", """
        dotask() { "$DOTASK_TEST_HOST" "$DOTASK_TEST_CLI" "$@"; }
        source ./completion.bash
        COMP_LINE='dotask run -c r'
        COMP_POINT=${#COMP_LINE}
        COMP_WORDS=(dotask run -c r)
        COMP_CWORD=3
        _dotask_complete
        [[ ${COMPREPLY[0]} == Release ]] || exit 10
        COMP_LINE='dotask run configuration=r'
        COMP_POINT=${#COMP_LINE}
        COMP_WORDS=(dotask run configuration = r)
        COMP_CWORD=4
        _dotask_complete
        [[ ${COMPREPLY[0]} == Release ]] || exit 11
        """],
      Environment = new Dictionary<string, string?> {
        ["DOTASK_TEST_HOST"] = DotnetHost.Find(),
        ["DOTASK_TEST_CLI"] = typeof(CliApplication).Assembly.Location
      },
      WorkingDirectory = project.Root,
      CaptureOutput = true,
      ThrowOnError = false
    }, CancellationToken.None);
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
  }

  [Fact]
  public void CursorAwareCompletionUsesOnlyTextBeforeCursorIncludingUnicode()
  {
    using var project = new TestProject();
    project.Target("run", metadata: "/// <option name=\"name\" /><option name=\"configuration\" alias=\"c\" choices=\"Debug,Release\" />");
    const string prefix = "dotask run name=日本語 -c r";
    var output = new StringWriter();
    CompletionCommand.Query(["--shell", "bash", "--line", prefix + " unrelated", "--position", System.Text.Encoding.UTF8.GetByteCount(prefix).ToString()], project.Root, output);
    Assert.StartsWith("Release\tvalue", output.ToString());
  }
}

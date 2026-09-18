using DoTask.Cli.Completion;

namespace DoTask.Tests;

public sealed class CompletionTests
{
  [Theory]
  [InlineData("dotask r", "run")]
  [InlineData("dotask help r", "run")]
  [InlineData("dotask RUN --c", "--configuration")]
  [InlineData("dotask run -c r", "Release")]
  [InlineData("dotask run Configuration=r", "Configuration=Release")]
  [InlineData("dotask run --configuration=r", "--configuration=Release")]
  [InlineData("dotask run --verbose ", "false")]
  [InlineData("dotask completion p", "powershell")]
  public void SuggestionsUseTargetMetadata( string line, string expected )
  {
    using var project = new TestProject();
    project.Target("run", "ThisDeliberatelyDoesNotCompile();", """
      /// <summary>Run app.</summary>
      /// <option name="configuration" alias="c" choices="Debug,Release" />
      /// <option name="verbose" type="bool" />
      """);
    Assert.Contains(CompletionEngine.Complete(line, project.Root), c => c.Value == expected);
    Assert.False(Directory.Exists(Path.Combine(project.Tasks, "obj")));
  }

  [Fact]
  public void UsedOptionsAreNotSuggestedAgain()
  {
    using var project = new TestProject();
    project.Target("run", metadata: "/// <option name=\"configuration\" alias=\"c\" choices=\"Debug,Release\" />");
    var suggestions = CompletionEngine.Complete("dotask run -c Debug --", project.Root);
    Assert.DoesNotContain(suggestions, c => c.Value == "--configuration");
  }

  [Theory]
  [InlineData("dotask r", "run")]
  [InlineData("dotask dotnet-r", "dotnet-run")]
  [InlineData("dotask help r", "run")]
  [InlineData("dotask help dotnet-r", "dotnet-run")]
  [InlineData("dotask RUN -c r", "Release")]
  [InlineData("dotask DOTNET-RUN -c r", "Release")]
  public void FullAndShortNamesUseTheSameCompletionMetadata( string line, string expected )
  {
    using var project = new TestProject();
    project.Target("dotnet run", "DoesNotCompile();", "/// <option name=\"configuration\" alias=\"c\" choices=\"Debug,Release\" />");
    project.Write(".tasks/config.yaml", "invalid: [");
    Assert.Contains(CompletionEngine.Complete(line, project.Root), c => c.Value == expected);
  }

  [Fact]
  public void CompletionOmitsAmbiguousAliasesAndHonorsExplicitEntryPoints()
  {
    using var project = new TestProject();
    project.Target("dotnet format", metadata: "/// <option name=\"verify\" type=\"bool\" />");
    project.Target("rust format", metadata: "/// <option name=\"check\" type=\"bool\" />");
    var names = CompletionEngine.Complete("dotask ", project.Root);
    Assert.DoesNotContain(names, c => c.Value == "format");
    Assert.Contains(names, c => c.Value == "dotnet-format");
    Assert.Contains(names, c => c.Value == "rust-format");
    Assert.DoesNotContain(CompletionEngine.Complete("dotask format --", project.Root), c => c.Value is "--verify" or "--check");
    Assert.Contains(CompletionEngine.Complete("dotask dotnet-format --", project.Root), c => c.Value == "--verify");
    project.Target("format", metadata: "/// <option name=\"all\" type=\"bool\" />");
    Assert.Single(CompletionEngine.Complete("dotask f", project.Root), c => c.Value == "format");
    Assert.Contains(CompletionEngine.Complete("dotask format --", project.Root), c => c.Value == "--all");
    Assert.DoesNotContain(CompletionEngine.Complete("dotask format --", project.Root), c => c.Value is "--verify" or "--check");
  }

  [Fact]
  public void ExplicitDirectoryAndPathsWithSpacesWorkWithoutReadingYaml()
  {
    using var project = new TestProject();
    var custom = Path.Combine(project.Root, "custom tasks");
    Directory.CreateDirectory(custom);
    File.Copy(project.Target("custom", metadata: "/// <option name=\"output\" type=\"path\" completion=\"directory\" />"), Path.Combine(custom, "custom.cs"));
    project.Write("custom tasks/config.yaml", "this is intentionally invalid YAML [");
    Directory.CreateDirectory(Path.Combine(project.Root, "output files"));
    Assert.Contains(CompletionEngine.Complete("dotask --use-dir 'custom tasks' custom --output out", project.Root),
      c => c.Value == "output files/" && c.Kind == "directory");
    Assert.Contains(CompletionEngine.Complete("dotask --use-dir=cu", project.Root), c => c.Value == "--use-dir=custom tasks/");
    Assert.Contains(CompletionEngine.Complete("dotask --use-dir cu", project.Root), c => c.Value == "custom tasks/");
  }

  [Theory]
  [InlineData("dotask --use-dir 'a b' run ", "bash", "a b")]
  [InlineData("dotask --use-dir a\\ b run ", "bash", "a b")]
  [InlineData("dotask --use-dir 'a''b' run ", "powershell", "a'b")]
  [InlineData("dotask --use-dir a` b run ", "powershell", "a b")]
  public void TokenizationPreservesShellArgumentBoundaries( string line, string shell, string directory )
  {
    var words = ShellWords.Parse(line, shell);
    Assert.Equal(directory, words[2]);
    Assert.Equal("", words[^1]);
  }
}

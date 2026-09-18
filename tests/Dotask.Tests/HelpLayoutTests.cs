using System.Text;
using DoTask.Cli;

namespace DoTask.Tests;

public sealed class HelpLayoutTests
{
  [Theory]
  [InlineData(60)]
  [InlineData(96)]
  [InlineData(120)]
  public void WrappingPreservesWordsAndAlignsContinuationLines(int width)
  {
    const string prefix = "  --configuration, -c <Debug|Release>  ";
    var description = string.Join(" ", Enumerable.Repeat("Build configuration for the selected project.", 5));
    var output = new StringWriter();
    new HelpText(output, width).WriteRow(prefix, description);
    var lines = output.ToString().Split(Environment.NewLine, StringSplitOptions.RemoveEmptyEntries);
    Assert.True(lines.Length > 1);
    Assert.All(lines, line => Assert.True(line.Length <= width, line));
    Assert.StartsWith(prefix, lines[0]);
    Assert.All(lines.Skip(1), line => Assert.StartsWith(new string(' ', prefix.Length), line));
    Assert.Equal(description, string.Join(" ", lines.Select(line => line[prefix.Length..])));
  }

  [Theory]
  [InlineData(null)]
  [InlineData(0)]
  [InlineData(59)]
  public void UnknownOrNarrowWidthsDoNotWrap(int? width)
  {
    var text = "  --input <path>  " + new string('x', 200);
    var output = new StringWriter();
    new HelpText(output, width).WriteLine(text, 18);
    Assert.Equal(text + Environment.NewLine, output.ToString());
  }

  [Fact]
  public void LongLabelsAndTokensAreWrappedWithoutLosingTextOrSplittingSurrogates()
  {
    var label = "--" + new string('x', 90);
    var value = new string('y', 57) + "😀" + new string('z', 150);
    var output = new StringWriter();
    var help = new HelpText(output, 60);
    help.WriteRow(label + "  ", value);
    var lines = output.ToString().Split(Environment.NewLine, StringSplitOptions.RemoveEmptyEntries);
    Assert.All(lines, line => Assert.True(line.Length <= 60, line));
    Assert.Equal(label + value, string.Concat(lines.Select(line => line.Trim())));
    _ = new UTF8Encoding(false, true).GetBytes(output.ToString());
    output.GetStringBuilder().Clear();
    help.WriteRow(new string(' ', 59), "😀");
    Assert.Equal("  😀" + Environment.NewLine, output.ToString());
  }

  [Theory]
  [InlineData(60)]
  [InlineData(96)]
  public async Task ProjectTargetAndCliHelpHonorTheRequestedWidth(int width)
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", """
      name: Help wrapping fixture
      description: A project description long enough to exercise wrapping while keeping every word in the output at any supported width.
      settings:
        project: src/App/App.csproj
        solution: My Application.slnx
      """);
    project.Target("dotnet build", "ThisDoesNotCompile();", """
      /// <summary>Build all of the projects in the configured solution using the selected SDK and build configuration.</summary>
      /// <option name="configuration" alias="c" choices="Debug,Release" default="Debug">Build configuration for all projects selected by the solution and the current platform.</option>
      /// <requires setting="solution" />
      /// <example>dotask build --configuration Release</example>
      """);
    foreach (var command in new[] { "", "help", "build --help", "--help" })
    {
      var output = new StringWriter();
      var error = new StringWriter();
      var code = await CliApplication.RunAsync(command.Split(' ', StringSplitOptions.RemoveEmptyEntries),
        project.Root, output, error, consoleWidth: width);
      Assert.Equal(0, code);
      Assert.Equal("", error.ToString());
      Assert.All(output.ToString().Split(Environment.NewLine), line => Assert.True(line.Length <= width, line));
    }
  }

  [Fact]
  public async Task SummaryShowsConditionalStringQuotesTasksLastAndOnlySharedDefaults()
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", """
      settings:
        project: src/App/App.csproj
        solution: My Application.slnx
        empty: ""
        label: "Developer's project"
        enabled: true
        names: [one, two]
      targets:
        test:
          defaults:
            configuration: Release
      """);
    const string options = """
      /// <option name="configuration" alias="c" choices="Debug,Release" default="Debug">Build configuration.</option>
      /// <option name="verify" type="bool" default="false">Verify formatting.</option>
      """;
    project.Target("build", metadata: options);
    project.Target("test", metadata: options);
    var output = new StringWriter();
    Assert.Equal(0, await CliApplication.RunAsync([], project.Root, output, new StringWriter(), consoleWidth: 96));
    var text = output.ToString().Replace("\r\n", "\n");
    Assert.Contains("  project: src/App/App.csproj\n", text);
    Assert.Contains("  solution: 'My Application.slnx'\n", text);
    Assert.Contains("  label: 'Developer''s project'\n", text);
    Assert.Contains("  empty: ''\n", text);
    Assert.Contains("  enabled: true\n", text);
    Assert.Contains("  names: [\"one\",\"two\"]\n", text);
    Assert.Contains("  tasks: ./.tasks\n\nTargets:\n", text);
    var combined = text.Split("Target options:\n")[1];
    Assert.DoesNotContain("Targets:", combined);
    Assert.DoesNotContain("Defaults:", combined);
    Assert.DoesNotContain("default: Debug", combined);
    Assert.DoesNotContain("default: Release", combined);
    Assert.Contains("Verify formatting. (default: false)", combined);
    Assert.EndsWith("See `dotask help <target>` for detailed information on each target.\n", text);
    output.GetStringBuilder().Clear();
    Assert.Equal(0, await CliApplication.RunAsync(["test", "--help"], project.Root, output, new StringWriter(), consoleWidth: 96));
    Assert.Contains("Build configuration. (default: Release)", output.ToString());
  }

  [Fact]
  public async Task CompletionDataBypassesHelpWrapping()
  {
    using var project = new TestProject();
    var name = "task" + new string('x', 90);
    project.Target(name);
    var output = new StringWriter();
    Assert.Equal(0, await CliApplication.RunAsync(
      ["__complete", "--shell", "bash", "--line", "dotask ", "--position", "7"],
      project.Root, output, new StringWriter(), consoleWidth: 60));
    Assert.Contains(name + "\t", output.ToString());
  }
}

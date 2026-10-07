using System.Text;

namespace DoTask.Tests;

public sealed class HelpLayoutTests
{
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
      // dotask: 1
      // options:
      //   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"], "default": "Debug", "description": "Build configuration."}
      //   - {"name": "verify", "type": "bool", "default": "false", "description": "Verify formatting."}
      // end-dotask
      """;
    project.Target("build", metadata: options);
    project.Target("test", metadata: options);
    var output = new StringWriter();
    Assert.Equal(0, await NativeCli.RunAsync(["--verbose"], project.Root, output, new StringWriter()));
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
    Assert.Equal(0, await NativeCli.RunAsync(["test", "--help"], project.Root, output, new StringWriter()));
    Assert.Contains("Build configuration. (default: Release)", output.ToString());
  }

  [Fact]
  public async Task CompletionDataBypassesHelpWrapping()
  {
    using var project = new TestProject();
    var name = "task" + new string('x', 90);
    project.Target(name);
    var output = new StringWriter();
    Assert.Equal(0, await NativeCli.RunAsync(
      ["__complete", "--shell", "bash", "--line", "dotask ", "--position", "7"],
      project.Root, output, new StringWriter()));
    Assert.Contains(name + "\t", output.ToString());
  }
}

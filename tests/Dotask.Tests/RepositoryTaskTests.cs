using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using DoTask.Cli.Metadata;

namespace DoTask.Tests;

public sealed class RepositoryTaskTests
{
  [Fact]
  public async Task CatalogPreservesMetadataHashesSupportAndStableOrderingWithoutExecutingTasks()
  {
    using var project = CatalogProject();
    var source = """
      /// <summary>Café &amp; tools
      /// with <c>XML</c>.</summary>
      /// <requires task="a/help" />
      /// <requires file="z/_support/helper.cs" />
      public static class Target { public static void Main() { throw new Exception(); } }
      """.ReplaceLineEndings("\r\n");
    project.Write("shared/z/run.cs", source);
    project.Write("shared/a/help.cs", "// no metadata\n");
    project.Write("shared/z/_ignored.cs", "not a task");
    project.Write("shared/z/_support/helper.cs", "support");
    var bytes = await GenerateCatalogAsync(project);
    Assert.Equal(bytes, await GenerateCatalogAsync(project));
    Assert.DoesNotContain((byte)'\r', bytes);
    Assert.Equal((byte)'\n', bytes[^1]);
    using var json = JsonDocument.Parse(bytes);
    var tasks = json.RootElement.GetProperty("tasks");
    Assert.Equal(2, tasks.GetArrayLength());
    Assert.Equal("a/help", tasks[0].GetProperty("id").GetString());
    Assert.Equal("(no description)", tasks[0].GetProperty("description").GetString());
    var task = tasks[1];
    Assert.Equal("Café & tools with XML.", task.GetProperty("description").GetString());
    Assert.Equal("a/help", task.GetProperty("requires")[0].GetString());
    Assert.Equal(2, task.GetProperty("files").GetArrayLength());
    Assert.Equal(Convert.ToHexStringLower(SHA256.HashData(Encoding.UTF8.GetBytes(source))),
      task.GetProperty("files")[1].GetProperty("sha256").GetString());
  }

  [Theory]
  [InlineData("../outside.cs")]
  [InlineData("/outside.cs")]
  [InlineData("group/../../outside.cs")]
  [InlineData("group\\outside.cs")]
  [InlineData("C:/outside.cs")]
  public async Task CatalogRejectsNonportableOrEscapingSupportPaths( string path )
  {
    using var project = CatalogProject();
    project.Write("shared/group/run.cs", $$"""
      /// <requires file="{{path}}" />
      public static class Target { public static void Main() { } }
      """);
    var result = await project.RunAsync("catalog", "--root", "shared");
    Assert.Equal(1, result.ExitCode);
    Assert.Contains("portable relative path", result.StandardError);
    Assert.False(File.Exists(Path.Combine(project.Root, "shared/catalog.json")));
  }

  [Theory]
  [InlineData("<requires unknown=\"x\" />", "Metadata error:")]
  [InlineData("<requires file=\"\" />", "Metadata error:")]
  [InlineData("<requires file=\"group/a.txt\" task=\"group/run\" />", "Metadata error:")]
  [InlineData("<requires task=\"group/missing\" />", "requires missing task:")]
  [InlineData("<requires task=\"group/missing\">", "Metadata error:")]
  public async Task CatalogRejectsInvalidMetadataAndMissingDependencies( string metadata, string error )
  {
    using var project = CatalogProject();
    project.Write("shared/group/run.cs", $"/// {metadata}\npublic static class Target {{ public static void Main() {{ }} }}");
    var result = await project.RunAsync("catalog", "--root", "shared");
    Assert.Equal(1, result.ExitCode);
    Assert.Contains(error, result.StandardError);
    Assert.False(File.Exists(Path.Combine(project.Root, "shared/catalog.json")));
  }

  [Fact]
  public async Task CatalogRejectsLegacySidecarsWithMigrationInstructions()
  {
    using var project = CatalogProject();
    project.Write("shared/group/run.cs", "// task");
    var manifest = project.Write("shared/group/run.task.json", "{\"requires\":[\"group/check\"]}");
    var result = await project.RunAsync("catalog", "--root", "shared");
    Assert.Equal(1, result.ExitCode);
    Assert.Contains("XML <requires task=", result.StandardError);
    Assert.Contains(manifest, result.StandardError);
    Assert.True(File.Exists(manifest));
  }

  [Fact]
  public async Task CatalogUsesEntryPointDocumentationInsteadOfUnrelatedComments()
  {
    using var project = CatalogProject();
    project.Write("shared/group/run.cs", """
      /// <summary>Ignored</summary>
      /// <requires task="group/missing" />
      public class Helper { }
      public static class Target
      {
        /**
         * <summary>Selected entry point</summary>
         * <requires task="group/check" />
         */
        public static void Main() { InvalidCSharp(); }
      }
      """);
    project.Write("shared/group/check.cs", "// task");
    using var json = JsonDocument.Parse(await GenerateCatalogAsync(project));
    var task = json.RootElement.GetProperty("tasks")[1];
    Assert.Equal("Selected entry point", task.GetProperty("description").GetString());
    Assert.Equal("group/check", task.GetProperty("requires")[0].GetString());
  }

  [Fact]
  public async Task CatalogVerifyDetectsDriftWithoutOverwritingIt()
  {
    using var project = CatalogProject();
    project.Write("shared-tasks/group/run.cs", "/// <summary>Original</summary>\npublic static class Target { public static void Main() { } }");
    var generated = await project.RunAsync("catalog");
    Assert.True(generated.ExitCode == 0, generated.StandardError);
    var destination = Path.Combine(project.Root, "shared-tasks/catalog.json");
    var baseline = File.ReadAllBytes(destination);
    Assert.Equal(0, (await project.RunAsync("catalog", "--verify")).ExitCode);
    project.Write("shared-tasks/group/run.cs", "/// <summary>Changed</summary>\npublic static class Target { public static void Main() { } }");
    var stale = await project.RunAsync("catalog", "--verify");
    Assert.Equal(1, stale.ExitCode);
    Assert.Contains("catalog.json is stale", stale.StandardError);
    Assert.Equal(baseline, File.ReadAllBytes(destination));
  }

  [Fact]
  public async Task RepositoryVerifyRequiresEveryStageForwardsOptionsAndStopsOnFailure()
  {
    using var project = new TestProject();
    Copy(project, "verify.cs");
    project.Target("check", "File.AppendAllText(\"order\", \"check;\");");
    project.Target("dotnet/test", "File.AppendAllText(\"order\", BuildContext.Current.Parameters.Get<string>(\"configuration\") + \";\");",
      "/// <option name=\"configuration\" choices=\"Debug,Release\" />");
    project.Target("dotnet/format", "File.AppendAllText(\"order\", BuildContext.Current.Parameters.Get<bool>(\"verify\") + \";\");",
      "/// <option name=\"verify\" type=\"bool\" />");
    project.Target("verify-docs", "File.AppendAllText(\"order\", \"docs;\");");
    project.Target("catalog", "File.AppendAllText(\"order\", BuildContext.Current.Parameters.Get<bool>(\"verify\").ToString());",
      "/// <option name=\"verify\" type=\"bool\" />");
    project.Target("shim", "File.AppendAllText(\"order\", \";shim:\" + BuildContext.Current.Parameters.Get<bool>(\"verify\"));",
      "/// <option name=\"verify\" type=\"bool\" />");
    var result = await project.RunAsync("verify", "-c", "Debug");
    Assert.True(result.ExitCode == 0, result.StandardError);
    var order = Path.Combine(project.Root, "order");
    Assert.Equal("check;Debug;True;docs;True;shim:True", File.ReadAllText(order));
    File.Delete(order);
    File.Delete(Path.Combine(project.Tasks, "verify-docs.cs"));
    result = await project.RunAsync("verify");
    Assert.Equal(1, result.ExitCode);
    Assert.Equal("check;Release;True;", File.ReadAllText(order));
    File.Delete(order);
    project.Target("dotnet/format", "Environment.Exit(23);", "/// <option name=\"verify\" type=\"bool\" />");
    Assert.Equal(23, (await project.RunAsync("verify")).ExitCode);
    Assert.Equal("check;Release;", File.ReadAllText(order));
  }

  [Fact]
  public async Task DocumentationCheckFailsForMissingRequiredFile()
  {
    using var project = new TestProject();
    Copy(project, "verify-docs.cs");
    var result = await project.RunAsync("verify-docs");
    Assert.Equal(1, result.ExitCode);
    Assert.Contains("Required document is missing: README.md", result.StandardError);
  }

  [Theory]
  [InlineData(0)]
  [InlineData(29)]
  public async Task DocumentationCheckDelegatesGitWhitespaceAndPreservesFailures( int exitCode )
  {
    using var project = DocumentationProject();
    project.Target("git/check", $$"""
      var project = BuildContext.Current;
      project.Files.WriteText("git-check-called", project.Parameters.Get<bool>("whitespace").ToString());
      Environment.Exit({{exitCode}});
      """, "/// <option name=\"whitespace\" type=\"bool\" default=\"false\" />");
    var metadata = MetadataReader.Read(Path.Combine(project.Tasks, "verify-docs.cs"), project.Tasks);
    Assert.Contains(new Requirement("task", "git/check"), metadata.Requirements);
    var result = await project.RunAsync("verify-docs");
    Assert.Equal(exitCode, result.ExitCode);
    Assert.Equal("True", File.ReadAllText(Path.Combine(project.Root, "git-check-called")));
    Assert.Equal(exitCode == 0, result.StandardOutput.Contains("Required documentation exists", StringComparison.Ordinal));
  }

  [Fact]
  public async Task DocumentationCheckFailsWhenGitTaskIsMissing()
  {
    using var project = DocumentationProject();
    var result = await project.RunAsync("verify-docs");
    Assert.Equal(1, result.ExitCode);
    Assert.Contains("Unknown target 'git/check'", result.StandardError);
    Assert.DoesNotContain("Required documentation exists", result.StandardOutput);
  }

  [Fact]
  public async Task GitCheckOptionChecksBothDiffsWithoutChangingFilesAndDefaultWorksOutsideRepository()
  {
    using var project = new TestProject();
    using (var stream = typeof(RepositoryTaskTests).Assembly.GetManifestResourceStream("Shared/git/check.cs")!)
    using (var reader = new StreamReader(stream)) {
      project.Write(".tasks/git/check.cs", reader.ReadToEnd());
    }
    var version = await project.RunAsync("git/check");
    Assert.True(version.ExitCode == 0, version.StandardError);
    Assert.Contains("OK: git version", version.StandardOutput);
    Assert.NotEqual(0, (await project.RunAsync("git/check", "--whitespace")).ExitCode);
    await GitAsync("init");
    await GitAsync("config", "core.autocrlf", "false");
    await GitAsync("config", "core.whitespace", "blank-at-eol");
    var file = project.Write("tracked.txt", "clean\n");
    await GitAsync("add", "tracked.txt");
    Assert.Equal(0, (await project.RunAsync("git/check", "--whitespace")).ExitCode);

    File.WriteAllText(file, "unstaged error \n");
    var unstaged = await project.RunAsync("git/check", "--whitespace");
    Assert.NotEqual(0, unstaged.ExitCode);
    Assert.Contains("trailing whitespace", unstaged.StandardOutput + unstaged.StandardError);
    Assert.Equal("unstaged error \n", File.ReadAllText(file));
    // The default remains a tool check even with a dirty working tree.
    Assert.Equal(0, (await project.RunAsync("git/check")).ExitCode);

    await GitAsync("add", "tracked.txt");
    File.WriteAllText(file, "working tree fixed\n");
    var staged = await project.RunAsync("git/check", "--whitespace");
    Assert.NotEqual(0, staged.ExitCode);
    Assert.Contains("trailing whitespace", staged.StandardOutput + staged.StandardError);
    Assert.Equal("working tree fixed\n", File.ReadAllText(file));
    var index = await GitAsync("show", ":tracked.txt");
    Assert.Equal("unstaged error \n", index.StandardOutput);

    async Task<ProcessResult> GitAsync( params string[] arguments ) => await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = "git",
      Arguments = arguments,
      WorkingDirectory = project.Root,
      CaptureOutput = true,
      Environment = new Dictionary<string, string?> { ["GIT_CONFIG_GLOBAL"] = Path.Combine(project.Root, "no-global-gitconfig"), ["GIT_CONFIG_NOSYSTEM"] = "1" }
    }, CancellationToken.None);
  }

  private static TestProject CatalogProject()
  {
    var project = new TestProject();
    Copy(project, "catalog.cs");
    Copy(project, "MetadataReader.cs", "src/Dotask.Cli/Metadata/MetadataReader.cs");
    Copy(project, "TargetDefinition.cs", "src/Dotask.Cli/Metadata/TargetDefinition.cs");
    return project;
  }

  private static async Task<byte[]> GenerateCatalogAsync( TestProject project )
  {
    var result = await project.RunAsync("catalog", "--root", "shared");
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    return File.ReadAllBytes(Path.Combine(project.Root, "shared/catalog.json"));
  }

  private static TestProject DocumentationProject()
  {
    var project = new TestProject();
    Copy(project, "verify-docs.cs");
    foreach (var path in new[] {
      "README.md", "LICENSE.md", "AGENTS.md", "docs/README.md", "docs/USAGE.md",
      "docs/SHARED-TASKS.md", "docs/INSTALLATION.md", "docs/TARGETS.md", "docs/AI-ASSISTANTS.md",
      "docs/DESIGN.md", "docs/VERIFICATION.md", "docs/CHANGELOG.md", "examples/basic/README.md"
    }) {
      project.Write(path, "fixture documentation\n");
    }
    return project;
  }

  private static void Copy( TestProject project, string resource, string? destination = null )
  {
    using var stream = typeof(RepositoryTaskTests).Assembly.GetManifestResourceStream("RepositoryTasks/" + resource)!;
    using var reader = new StreamReader(stream);
    project.Write(destination ?? (".tasks/" + resource), reader.ReadToEnd());
  }
}

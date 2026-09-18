using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using DoTask.RepositoryTasks;

namespace DoTask.Tests;

public sealed class RepositoryTaskTests
{
  [Fact]
  public void CatalogPreservesMetadataHashesSupportAndStableOrderingWithoutExecutingTasks()
  {
    using var project = new TestProject();
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
    var bytes = TaskCatalog.Generate(Path.Combine(project.Root, "shared"));
    Assert.Equal(bytes, TaskCatalog.Generate(Path.Combine(project.Root, "shared")));
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
  public void CatalogRejectsNonportableOrEscapingSupportPaths(string path)
  {
    using var project = new TestProject();
    project.Write("shared/group/run.cs", $$"""
      /// <requires file="{{path}}" />
      public static class Target { public static void Main() { } }
      """);
    Assert.Throws<TaskException>(() => TaskCatalog.Generate(Path.Combine(project.Root, "shared")));
  }

  [Theory]
  [InlineData("<requires unknown=\"x\" />")]
  [InlineData("<requires file=\"\" />")]
  [InlineData("<requires file=\"group/a.txt\" task=\"group/run\" />")]
  [InlineData("<requires task=\"group/missing\" />")]
  [InlineData("<requires task=\"group/missing\">")]
  public void CatalogRejectsInvalidMetadataAndMissingDependencies(string metadata)
  {
    using var project = new TestProject();
    project.Write("shared/group/run.cs", $"/// {metadata}\npublic static class Target {{ public static void Main() {{ }} }}");
    Assert.Throws<TaskException>(() => TaskCatalog.Generate(Path.Combine(project.Root, "shared")));
  }

  [Fact]
  public void CatalogRejectsLegacySidecarsWithMigrationInstructions()
  {
    using var project = new TestProject();
    project.Write("shared/group/run.cs", "// task");
    var manifest = project.Write("shared/group/run.task.json", "{\"requires\":[\"group/check\"]}");
    var error = Assert.Throws<TaskException>(() => TaskCatalog.Generate(Path.Combine(project.Root, "shared")));
    Assert.Contains("XML <requires task=", error.Message);
    Assert.Contains(manifest, error.Message);
    Assert.True(File.Exists(manifest));
  }

  [Fact]
  public void CatalogUsesEntryPointDocumentationInsteadOfUnrelatedComments()
  {
    using var project = new TestProject();
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
    using var json = JsonDocument.Parse(TaskCatalog.Generate(Path.Combine(project.Root, "shared")));
    var task = json.RootElement.GetProperty("tasks")[1];
    Assert.Equal("Selected entry point", task.GetProperty("description").GetString());
    Assert.Equal("group/check", task.GetProperty("requires")[0].GetString());
  }

  [Fact]
  public async Task CatalogVerifyDetectsDriftWithoutOverwritingIt()
  {
    using var project = new TestProject();
    Copy(project, "catalog.cs");
    Copy(project, "TaskCatalog.cs", ".tasks/_support/TaskCatalog.cs");
    Copy(project, "MetadataReader.cs", "src/Dotask.Cli/Metadata/MetadataReader.cs");
    Copy(project, "TargetDefinition.cs", "src/Dotask.Cli/Metadata/TargetDefinition.cs");
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

  private static void Copy(TestProject project, string resource, string? destination = null)
  {
    using var stream = typeof(RepositoryTaskTests).Assembly.GetManifestResourceStream("RepositoryTasks/" + resource)!;
    using var reader = new StreamReader(stream);
    project.Write(destination ?? ".tasks/" + resource, reader.ReadToEnd());
  }
}

using System.Text.Json;

namespace DoTask.Tests;

public sealed class LanguageVariantTests
{
  private const string Header = "// dotask: 1\n// description: Variant.\n// options: [{name: message, default: header}, {name: recurse, type: bool}]\n// end-dotask\n";

  [Fact]
  public async Task QualifiedCallsDefaultsCompletionNestedCallsAndCyclesDistinguishImplementations()
  {
    using var project = new TestProject();
    project.Write(".dotasks.yaml", "targets: {tools/check: {defaults: {message: common}}, tools/check.cs: {defaults: {MESSAGE: csharp}}, tools/check.rs: {defaults: {message: rust}}}");
    project.Target("tools/check", """
      var p = BuildContext.Current;
      Console.WriteLine(p.TargetName + ":" + p.Parameters.Get<string>("message"));
      if (p.Parameters.Get<bool>("recurse")) await p.ExecTargetAsync("tools/check.rs", new { recurse = true });
      """, Header, async: true);
    project.Write(".tasks/tools/check.rs", Header + """
      use dotask_sdk::{BuildContext, Result};
      fn main() { dotask_sdk::run(task); }
      fn task(p: &BuildContext) -> Result<()> {
        println!("{}:{}", p.task_name(), p.string("message")?);
        Ok(())
      }
      """);
    Assert.Equal(1, (await project.RunAsync("check")).ExitCode);
    Assert.Contains("csharp", (await project.RunAsync("help", "tools/check.cs")).StandardOutput);
    Assert.Contains("rust", (await project.RunAsync("help", "check.rs")).StandardOutput);
    Assert.Contains(NativeCompletion.Complete("dotask tools/check.", project.Root), c => c.Value == "tools/check.cs");
    Assert.Contains(NativeCompletion.Complete("dotask tools/check.", project.Root), c => c.Value == "tools/check.rs");
    Assert.DoesNotContain(NativeCompletion.Complete("dotask ", project.Root), c => c.Value == "check");
    var csharp = await project.RunAsync("CHECK.CS");
    Assert.True(csharp.ExitCode == 0, csharp.StandardError);
    Assert.Contains("tools/check.cs:csharp", csharp.StandardOutput);
    var rust = await project.RunAsync("tools/check.rs");
    Assert.True(rust.ExitCode == 0, rust.StandardError);
    Assert.Contains("tools/check.rs:rust", rust.StandardOutput);
    // Calling the other implementation is not a cycle. Returning to this one is.
    var nested = await project.RunAsync("check.cs", "--recurse");
    Assert.True(nested.ExitCode == 0, nested.StandardError);
    Assert.Contains("tools/check.rs:rust", nested.StandardOutput);
    project.Write(".tasks/tools/check.rs", Header + """
      use dotask_sdk::{json, BuildContext, Result};
      fn main() { dotask_sdk::run(task); }
      fn task(p: &BuildContext) -> Result<()> { p.exec_target("tools/check.cs", json!({})) }
      """);
    var cycle = await project.RunAsync("check.cs", "--recurse");
    Assert.NotEqual(0, cycle.ExitCode);
    Assert.Contains("tools/check.cs -> tools/check.rs -> tools/check.cs", cycle.StandardError);
    project.Write(".tasks/group.task", "steps: [{run: tools/check.cs}, {run: tools/check.rs}]");
    Assert.Contains("tools/check.cs", (await project.RunAsync("help", "group")).StandardOutput);
  }

  [Fact]
  public async Task SingleImplementationAcceptsQualifiedNamesAndDefaultsWithoutChangingItsCanonicalName()
  {
    using var project = new TestProject();
    project.Target("tools/check", "Console.WriteLine(BuildContext.Current.TargetName + \":\" + BuildContext.Current.Parameters.Get<string>(\"message\"));", Header);
    project.Write(".dotasks.yaml", "targets: {tools/check: {defaults: {message: base}}, tools/check.cs: {defaults: {message: specific}}}");
    var result = await project.RunAsync("check.cs");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Contains("tools/check:specific", result.StandardOutput);
    Assert.Contains(NativeCompletion.Complete("dotask check.", project.Root), c => c.Value == "check.cs");
  }

  [Fact]
  public async Task GroupLanguageSelectionNoOpSyncAndIndependentRemovalPreserveExistingOwnership()
  {
    using var project = new TestProject();
    var options = Options(project);
    project.Write("private/tools/check.cs", Header + "Console.WriteLine(\"csharp\");");
    Assert.Equal(0, (await RustArtifacts.Run(project.Root, ["--add", "private-tasks/tools"], options)).ExitCode);
    var lockFile = Path.Combine(project.Root, ".dotasks-lock.yaml");
    File.AppendAllText(lockFile, "# keep this note\n");
    var before = File.ReadAllBytes(lockFile);
    project.Write("private/tools/check.rs", Header + "fn main() { println!(\"rust\"); }");
    var sync = await RustArtifacts.Run(project.Root, ["--sync", "--lang", "csharp"], options);
    Assert.True(sync.ExitCode == 0, sync.StandardError);
    Assert.Equal(before, File.ReadAllBytes(lockFile));
    Assert.False(File.Exists(Path.Combine(project.Tasks, "private-tasks/tools/check.rs")));
    var added = await RustArtifacts.Run(project.Root, ["--add", "private-tasks/tools", "--lang=rust"], options);
    Assert.True(added.ExitCode == 0, added.StandardError);
    var tracking = TaskLock.Read(File.ReadAllBytes(lockFile), ".tasks");
    Assert.Contains("private-tasks/tools/check", tracking.Tasks.Keys);
    Assert.Contains("private-tasks/tools/check.rs", tracking.Tasks.Keys);
    Assert.Equal(2, tracking.Tasks.Count);
    Assert.Equal(0, (await RustArtifacts.Run(project.Root, ["--sync"], options)).ExitCode);
    Assert.Contains(NativeCompletion.Complete("dotask --remove --lang rust private-tasks/tools/", project.Root, sharedTaskOptions: options), c => c.Value == "private-tasks/tools/check.rs");
    Assert.DoesNotContain(NativeCompletion.Complete("dotask --remove --lang rust private-tasks/tools/", project.Root, sharedTaskOptions: options), c => c.Value == "private-tasks/tools/check");
    Assert.Equal(0, (await RustArtifacts.Run(project.Root, ["--remove", "private-tasks/tools", "--lang", "rust"], options)).ExitCode);
    Assert.True(File.Exists(Path.Combine(project.Tasks, "private-tasks/tools/check.cs")));
    Assert.False(File.Exists(Path.Combine(project.Tasks, "private-tasks/tools/check.rs")));
  }

  [Fact]
  public async Task UntrackedSiblingLocalEditsAndMalformedOwnershipNeverGetAdoptedOrOverwritten()
  {
    using var project = new TestProject();
    var options = Options(project);
    project.Write("private/tools/check.cs", Header);
    project.Write("private/tools/check.rs", Header);
    project.Write(".tasks/private-tasks/tools/check.rs", Header);
    var refused = await RustArtifacts.Run(project.Root, ["--add", "private-tasks/tools"], options);
    Assert.Equal(1, refused.ExitCode);
    Assert.False(File.Exists(Path.Combine(project.Tasks, "private-tasks/tools/check.cs")));
    Assert.False(File.Exists(Path.Combine(project.Root, ".dotasks-lock.yaml")));
    Assert.Equal(0, (await RustArtifacts.Run(project.Root, ["--add", "private-tasks/tools/check.cs"], options)).ExitCode);
    File.AppendAllText(Path.Combine(project.Tasks, "private-tasks/tools/check.cs"), "// local edit\n");
    var bytes = File.ReadAllBytes(Path.Combine(project.Root, ".dotasks-lock.yaml"));
    Assert.Equal(1, (await RustArtifacts.Run(project.Root, ["--remove", "private-tasks/tools/check.cs"], options)).ExitCode);
    Assert.Equal(bytes, File.ReadAllBytes(Path.Combine(project.Root, ".dotasks-lock.yaml")));
    var lockText = File.ReadAllText(Path.Combine(project.Root, ".dotasks-lock.yaml"));
    Assert.Contains("  private-tasks/tools/check.cs:", lockText);
    project.Write(".dotasks-lock.yaml", lockText.Replace("\n  private-tasks/tools/check.cs:", "\n  private-tasks/tools/check.rs:", StringComparison.Ordinal));
    Assert.Equal(1, (await RustArtifacts.Run(project.Root, ["--sync"], options)).ExitCode);
  }

  [Theory]
  [InlineData("python")]
  [InlineData("")]
  public async Task InvalidLanguageFailsBeforeProjectMutation( string language )
  {
    using var project = new TestProject();
    var result = await RustArtifacts.Run(project.Root, ["--add", "git", "--lang=" + language], Options(project));
    Assert.Equal(1, result.ExitCode);
    Assert.Contains("csharp, rust or go", result.StandardError);
    Assert.Empty(Directory.GetFileSystemEntries(project.Tasks));
    Assert.Contains(NativeCompletion.Complete("dotask --add git --lang r", project.Root), c => c.Value == "rust");
  }

  private static SharedTaskOptions Options( TestProject project ) => new(Path.Combine(project.Root, "cache"), Path.Combine(project.Root, "private"), Path.Combine(project.Root, "online"));

  [Fact]
  public async Task LanguageFilteredSyncUpdatesOnlyRecordedVariantsAndDependenciesUseExplicitFiles()
  {
    using var project = new TestProject();
    var options = Options(project);
    project.Write("private/tools/helper.cs", Header);
    project.Write("private/tools/helper.rs", Header);
    project.Write("private/tools/run.rs", "// dotask: 1\n// requires: [{kind: task, value: tools/helper.cs}]\n// end-dotask\n");
    var added = await RustArtifacts.Run(project.Root, ["--add", "private-tasks/tools/run.rs", "--lang", "rust"], options);
    Assert.True(added.ExitCode == 0, added.StandardError);
    Assert.True(File.Exists(Path.Combine(project.Tasks, "private-tasks/tools/helper.cs")));
    Assert.False(File.Exists(Path.Combine(project.Tasks, "private-tasks/tools/helper.rs")));
    Assert.Equal(1, (await RustArtifacts.Run(project.Root, ["--remove", "private-tasks/tools/helper.cs"], options)).ExitCode);
    var csharp = File.ReadAllBytes(Path.Combine(project.Tasks, "private-tasks/tools/helper.cs"));
    File.AppendAllText(Path.Combine(options.PrivateDirectory, "tools/helper.cs"), "// new csharp\n");
    File.AppendAllText(Path.Combine(options.PrivateDirectory, "tools/run.rs"), "// new rust\n");
    Assert.Equal(0, (await RustArtifacts.Run(project.Root, ["--sync", "--lang", "csharp"], options)).ExitCode);
    Assert.NotEqual(csharp, File.ReadAllBytes(Path.Combine(project.Tasks, "private-tasks/tools/helper.cs")));
    Assert.DoesNotContain("new rust", File.ReadAllText(Path.Combine(project.Tasks, "private-tasks/tools/run.rs")));
    project.Write("private/tools/extra.rs", Header);
    Assert.Equal(0, (await RustArtifacts.Run(project.Root, ["--sync"], options)).ExitCode);
    Assert.False(File.Exists(Path.Combine(project.Tasks, "private-tasks/tools/extra.rs")));
    project.Write("private/tools/run.rs", "// dotask: 1\n// requires: [{kind: task, value: tools/helper}]\n// end-dotask\n");
    var baseline = File.ReadAllBytes(Path.Combine(project.Root, ".dotasks-lock.yaml"));
    Assert.Equal(1, (await RustArtifacts.Run(project.Root, ["--sync"], options)).ExitCode);
    Assert.Equal(baseline, File.ReadAllBytes(Path.Combine(project.Root, ".dotasks-lock.yaml")));
  }

  [Fact]
  public async Task CatalogGeneratesDistinctVariantIdsAndRejectsAmbiguousRequirements()
  {
    using var project = new TestProject();
    using var stream = typeof(LanguageVariantTests).Assembly.GetManifestResourceStream("RepositoryTasks/catalog.rs")!;
    using var reader = new StreamReader(stream);
    project.Write(".tasks/catalog.rs", await reader.ReadToEndAsync());
    project.Write("shared/tools/check.cs", Header);
    project.Write("shared/tools/check.rs", Header);
    project.Write("shared/tools/run.rs", "// dotask: 1\n// requires: [{kind: task, value: tools/check.cs}]\n// end-dotask\n");
    var result = await project.RunAsync("catalog", "--root", "shared");
    Assert.True(result.ExitCode == 0, result.StandardError);
    using var json = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(project.Root, "shared/catalog.json")));
    var ids = json.RootElement.GetProperty("tasks").EnumerateArray().Select(t => t.GetProperty("id").GetString()).ToArray();
    Assert.Contains("tools/check.cs", ids);
    Assert.Contains("tools/check.rs", ids);
    Assert.Contains("tools/run", ids);
    project.Write("shared/tools/run.rs", "// dotask: 1\n// requires: [{kind: task, value: tools/check}]\n// end-dotask\n");
    var bytes = File.ReadAllBytes(Path.Combine(project.Root, "shared/catalog.json"));
    var invalid = await project.RunAsync("catalog", "--root", "shared");
    Assert.Equal(1, invalid.ExitCode);
    Assert.Contains("requires ambiguous task", invalid.StandardError);
    Assert.Equal(bytes, File.ReadAllBytes(Path.Combine(project.Root, "shared/catalog.json")));
  }

  [Fact]
  public async Task ASourceChangingLanguageCannotReuseTheExistingImplementationsOwnershipKey()
  {
    using var project = new TestProject();
    var options = Options(project);
    var original = project.Write("private/tools/check.cs", Header);
    Assert.Equal(0, (await RustArtifacts.Run(project.Root, ["--add", "private-tasks/tools/check"], options)).ExitCode);
    var installed = File.ReadAllBytes(Path.Combine(project.Tasks, "private-tasks/tools/check.cs"));
    File.Delete(original);
    project.Write("private/tools/check.rs", Header + "// rust replacement\n");
    Assert.Equal(0, (await RustArtifacts.Run(project.Root, ["--add", "private-tasks/tools/check.rs"], options)).ExitCode);
    var tracking = TaskLock.Read(File.ReadAllBytes(Path.Combine(project.Root, ".dotasks-lock.yaml")), ".tasks");
    Assert.Contains("private-tasks/tools/check", tracking.Tasks.Keys);
    Assert.Contains("private-tasks/tools/check.rs", tracking.Tasks.Keys);
    Assert.Equal(installed, File.ReadAllBytes(Path.Combine(project.Tasks, "private-tasks/tools/check.cs")));
    Assert.Equal(0, (await RustArtifacts.Run(project.Root, ["--sync", "--lang", "rust"], options)).ExitCode);
    Assert.Equal(installed, File.ReadAllBytes(Path.Combine(project.Tasks, "private-tasks/tools/check.cs")));
  }
}

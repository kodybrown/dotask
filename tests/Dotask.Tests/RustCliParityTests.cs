using System.Text.Json;
using DoTask.Cli;
using DoTask.Cli.SharedTasks;

namespace DoTask.Tests;

public sealed class RustCliParityTests
{
  private static string Normalize( string value ) => value.Replace("\r\n", "\n", StringComparison.Ordinal);

  private static async Task<ProcessResult> Reference( TestProject project, params string[] args )
  {
    using var output = new StringWriter();
    using var error = new StringWriter();
    var code = await CliApplication.RunAsync(args, project.Root, output, error);
    return new(code, output.ToString(), error.ToString());
  }

  [Theory]
  [InlineData("")]
  [InlineData("help")]
  [InlineData("--verbose")]
  [InlineData("help run")]
  [InlineData("run --help")]
  [InlineData("help group")]
  public async Task NativeHelpMatchesReferenceWithSettingsDefaultsAndAllMetadataSections( string command )
  {
    using var project = Project();
    project.Write(".dotasks.yaml", """
      name: Example
      description: Shared project
      settings: { solution: example.slnx, nested: { flag: true }, phrase: 'two words' }
      targets: { tools/run: { defaults: { configuration: Release } } }
      """);
    var args = command.Split(' ', StringSplitOptions.RemoveEmptyEntries);
    var reference = await Reference(project, args);
    var native = await RustArtifacts.Run(project.Root, args);
    Assert.Equal(reference.ExitCode, native.ExitCode);
    Assert.Equal(Normalize(reference.StandardOutput), Normalize(native.StandardOutput));
    Assert.Equal(Normalize(reference.StandardError), Normalize(native.StandardError));
    Assert.False(File.Exists(Path.Combine(project.Root, "executed")));
  }

  [Theory]
  [InlineData("dotask ", "bash")]
  [InlineData("dotask --verbose ru", "bash")]
  [InlineData("dotask help r", "zsh")]
  [InlineData("dotask run --", "fish")]
  [InlineData("dotask run -c R", "powershell")]
  [InlineData("dotask run --configuration=R", "bash")]
  [InlineData("dotask run --flag ", "bash")]
  [InlineData("dotask run --flag --", "bash")]
  [InlineData("dotask run --input 'with s", "powershell")]
  [InlineData("dotask --use-dir .tasks run --", "bash")]
  [InlineData("dotask --init --", "fish")]
  [InlineData("echo ignored | dotask run --", "powershell")]
  public async Task NativeCompletionMatchesReferenceWithoutLoadingProjectConfiguration( string line, string shell )
  {
    using var project = Project();
    project.Write(".dotasks.yaml", "invalid: [");
    project.Write("with spaces.txt", "example");
    var args = new[] { "__complete", "--line", line, "--shell", shell };
    var reference = await Reference(project, args);
    var native = await RustArtifacts.Run(project.Root, args);
    Assert.Equal(0, native.ExitCode);
    Assert.Equal(Normalize(reference.StandardOutput), Normalize(native.StandardOutput));
    Assert.Empty(native.StandardError);
    Assert.False(File.Exists(Path.Combine(project.Root, "executed")));
  }

  [Theory]
  [InlineData("settings: { item: &item { first: 1 }, alias: *item }")]
  [InlineData("settings: { quoted: 'true', number: 0x10, quotedNumber: !!str 12 }")]
  [InlineData("SETTINGS: { Flag: true }\nNAME: Native")]
  [InlineData("targets: { tools/run: { defaults: { configuration: release, flag: true } } }")]
  [InlineData("targets: { tools/run: { defaults: { label: été } } }")]
  public async Task NativeYamlMatchesReferenceScalarAndAliasSemantics( string config )
  {
    using var project = Project();
    project.Write(".dotasks.yaml", config);
    var reference = await Reference(project, "--verbose");
    var native = await RustArtifacts.Run(project.Root, ["--verbose"]);
    Assert.True(native.ExitCode == 0, native.StandardError);
    Assert.Equal(Normalize(reference.StandardOutput), Normalize(native.StandardOutput));
  }

  [Fact]
  public async Task NativeAndReferenceShareRevisionsAndPreserveNoOpTrackingBytes()
  {
    using var project = new TestProject();
    var source = project.Write("online/tools/check.cs", "// a source snapshot\n");
    var task = new SharedTask("tools/check", "tools/check.cs", "Quote \" café 日本語 <>&' + `", "csharp",
      [new("tools/check.cs", SharedTaskFiles.HashFile(source)!)], []);
    project.Write("online/catalog.json", JsonSerializer.Serialize(new SharedCatalog(1, [task]), SharedTaskJson.Options));
    var options = new SharedTaskOptions(Path.Combine(project.Root, "cache"), Path.Combine(project.Root, "private"), Path.Combine(project.Root, "online"));
    using var output = new StringWriter();
    using var error = new StringWriter();
    Assert.Equal(0, await CliApplication.RunAsync(["--add", "tools/check"], project.Root, output, error, sharedTaskOptions: options));
    var lockPath = Path.Combine(project.Root, ".dotasks-lock.yaml");
    File.AppendAllText(lockPath, "# retained user note\n");
    var original = File.ReadAllBytes(lockPath);
    var sync = await RustArtifacts.Run(project.Root, ["--sync"], options);
    Assert.True(sync.ExitCode == 0, sync.StandardError + sync.StandardOutput);
    var revision = TaskLock.Read(File.ReadAllBytes(lockPath), ".tasks").Tasks["_/tools/check"].Revision;
    Assert.Equal(JsonSerializer.Serialize(task, SharedTaskJson.Options),
      File.ReadAllText(Path.Combine(options.CacheDirectory, "_/.revisions", revision + ".json")));
    Assert.Contains("Unchanged:", sync.StandardOutput);
    Assert.Equal(original, File.ReadAllBytes(lockPath));
    var remove = await RustArtifacts.Run(project.Root, ["--remove", "tools/check"], options);
    Assert.True(remove.ExitCode == 0, remove.StandardError + remove.StandardOutput);
    Assert.Empty(TaskLock.Read(File.ReadAllBytes(lockPath), ".tasks").Tasks);
  }

  [Fact]
  public async Task NativeRecoveryUnderstandsReferenceJournalAndPreservesLaterEdits()
  {
    using var project = new TestProject();
    var target = project.Write(".tasks/tools/check.cs", "later edit");
    project.Write(".tasks/.dotask/owner", "dotask shared-task transaction state v1\n");
    project.Write(".tasks/.dotask/transaction/0.original", "original");
    var journal = new ProjectTaskTransaction.Journal(1,
      [new(".tasks/tools/check.cs", SharedTaskFiles.Hash("original"), SharedTaskFiles.Hash("incoming"), "0.original")]);
    var journalPath = project.Write(".tasks/.dotask/transaction/journal.json", JsonSerializer.Serialize(journal, SharedTaskJson.Options));
    var options = new SharedTaskOptions(Path.Combine(project.Root, "cache"), Path.Combine(project.Root, "private"));
    foreach (var args in new[] { new[] { "--sync", "--dry-run" }, new[] { "--sync" } }) {
      var refused = await RustArtifacts.Run(project.Root, args, options);
      Assert.Equal(1, refused.ExitCode);
      Assert.Equal("later edit", File.ReadAllText(target));
      Assert.True(File.Exists(journalPath));
    }
    File.WriteAllText(target, "incoming");
    var recovered = await RustArtifacts.Run(project.Root, ["--sync"], options);
    Assert.True(recovered.ExitCode == 0, recovered.StandardError + recovered.StandardOutput);
    Assert.Equal("original", File.ReadAllText(target));
    Assert.False(Directory.Exists(Path.Combine(project.Tasks, ".dotask")));
  }

  private static TestProject Project()
  {
    var project = new TestProject();
    project.Target("tools/run", "File.WriteAllText(\"executed\", \"wrong\");", """
      // dotask: 1
      // description: "Run a useful task."
      // remarks: "Metadata only."
      // options:
      //   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"], "default": "Debug", "description": "Build configuration."}
      //   - {"name": "input", "type": "path", "completion": "file", "description": "Input file."}
      //   - {"name": "flag", "type": "bool", "description": "Boolean flag."}
      //   - {"name": "label", "choices": ["Été", "Hiver"], "default": "Été", "description": "Label."}
      // requires:
      //   - {"kind": "tool", "value": "missing-tool-must-not-be-called"}
      // capabilities: ["process"]
      // examples: ["dotask run -c Release"]
      // end-dotask
      """);
    project.Write(".tasks/group.task", "description: Group description\nsteps:\n  - run: tools/run\n    optional: true\n    with: { flag: true }\n");
    return project;
  }
}

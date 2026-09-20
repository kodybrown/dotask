using System.Net;
using System.Text;
using System.Text.Json;
using DoTask.Cli;
using DoTask.Cli.Completion;
using DoTask.Cli.Configuration;
using DoTask.Cli.Discovery;
using DoTask.Cli.Execution;
using DoTask.Cli.SharedTasks;

namespace DoTask.Tests;

public sealed class SharedTaskTests
{
  [Fact]
  public async Task ListingAlignsDescriptionsAndComparesActualCacheWithoutChangingProject()
  {
    using var fixture = new Fixture();
    fixture.Online("tools/one");
    fixture.Online("tools/longer");
    fixture.WriteCatalog();
    Assert.Equal(0, (await fixture.Run("--add", "tools/one")).Code);
    var before = File.ReadAllBytes(fixture.LockPath);
    var listing = await fixture.Run("--list");
    Assert.Equal(0, listing.Code);
    Assert.Contains("Installed       Matches cache", listing.Output);
    Assert.Contains("Not in project", listing.Output);
    var rows = listing.Output.Split('\n').Where(line => line.Contains("Example task")).ToArray();
    Assert.Equal(2, rows.Length);
    Assert.Equal(rows[0].IndexOf("Example task", StringComparison.Ordinal), rows[1].IndexOf("Example task", StringComparison.Ordinal));
    // Refreshing the online catalog must not change the comparison with downloaded files.
    fixture.CatalogTasks.Clear();
    fixture.Online("tools/one", "Console.WriteLine(123);");
    fixture.WriteCatalog();
    Assert.Contains("Matches cache", (await fixture.Run("--list")).Output);
    File.AppendAllText(fixture.Installed("_/tools/one"), "// local edit");
    Assert.Contains("Differs", (await fixture.Run("--list")).Output);
    File.Delete(Path.Combine(fixture.Options.CacheDirectory, "_/tools/one.cs"));
    Assert.Contains("Unavailable", (await fixture.Run("--list")).Output);
    File.Delete(fixture.Installed("_/tools/one"));
    Assert.Contains("Missing", (await fixture.Run("--list")).Output);
    Assert.Equal(before, File.ReadAllBytes(fixture.LockPath));
    Assert.False(Directory.Exists(Path.Combine(fixture.Project.Tasks, ".dotask")));
  }

  [Fact]
  public async Task ListingDetectsPrivateSupportChangesAndUntrackedCopies()
  {
    using var fixture = new Fixture();
    fixture.Private("tools/one", metadata: "/// <requires file=\"tools/data.txt\" />");
    var support = Path.Combine(fixture.Options.PrivateDirectory, "tools/data.txt");
    File.WriteAllText(support, "original");
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/one")).Code);
    Assert.Contains("Matches original", (await fixture.Run("--list", "private-tasks/*")).Output);
    File.WriteAllText(support, "changed");
    Assert.Contains("Differs", (await fixture.Run("--list", "private-tasks/*")).Output);
    File.Delete(fixture.LockPath);
    Assert.Contains("Untracked", (await fixture.Run("--list", "private-tasks/*")).Output);
    Directory.Delete(fixture.Project.Tasks, true);
    File.Delete(Path.Combine(fixture.Project.Root, ".dotasks.yaml"));
    Assert.Contains("Not in project", (await fixture.Run("--list", "private-tasks/*")).Output);
    Assert.False(Directory.Exists(fixture.Project.Tasks));
  }

  [Theory]
  [InlineData("dotnet/build")]
  [InlineData("_/dotnet/build")]
  public async Task OfficialSourceUsesReservedDirectoryAndQualifiedCompletion( string selection )
  {
    using var fixture = new Fixture();
    fixture.Online("dotnet/build");
    fixture.WriteCatalog();
    var added = await fixture.Run("--add", selection);
    Assert.True(added.Code == 0, added.Error + added.Output);
    Assert.True(File.Exists(fixture.Installed("_/dotnet/build")));
    Assert.Equal("_", Assert.Single(fixture.Lock().Tasks).Value.Source);
    Assert.Contains(CompletionEngine.Complete("dotask _/dotnet/b", fixture.Project.Root), c => c.Value == "_/dotnet/build");
    var help = await fixture.Run("--help");
    Assert.Contains("Official tasks (_)", help.Output);
  }

  [Fact]
  public async Task SharedManagementRemovesIdleStateAndDryRunPreservesIt()
  {
    using var fixture = new Fixture();
    fixture.Private("tools/one");
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/one")).Code);
    var state = Path.Combine(fixture.Project.Tasks, ".dotask");
    Assert.False(Directory.Exists(state));
    fixture.Project.Write(".tasks/.dotask/owner", "dotask shared-task transaction state v1\n");
    fixture.Project.Write(".tasks/.dotask/.gitignore", "*\n");
    Assert.Equal(0, (await fixture.Run("--sync", "--dry-run")).Code);
    Assert.True(Directory.Exists(state));
    Assert.Equal(0, (await fixture.Run("--sync")).Code);
    Assert.False(Directory.Exists(state));
    Assert.Equal(0, (await fixture.Run("--remove", "private-tasks/tools/one")).Code);
    Assert.False(Directory.Exists(state));
  }

  [Theory]
  [InlineData("notes.txt", "keep")]
  [InlineData("transaction/notes.txt", "keep")]
  [InlineData(".gitignore", "custom")]
  public void IdleCleanupPreservesUnrecognizedFiles( string path, string content )
  {
    using var project = new TestProject();
    project.Write(".tasks/.dotask/owner", "dotask shared-task transaction state v1\n");
    var file = project.Write(".tasks/.dotask/" + path, content);
    new ProjectTaskTransaction(TaskDirectory.Locate(project.Root)).Apply([], CancellationToken.None);
    Assert.Equal(content, File.ReadAllText(file));
    Assert.True(File.Exists(Path.Combine(project.Tasks, ".dotask/owner")));
  }

  [Fact]
  public void IdleCleanupRemovesRecognizedOrphanStaging()
  {
    using var project = new TestProject();
    project.Write(".tasks/.dotask/owner", "dotask shared-task transaction state v1\n");
    project.Write(".tasks/.dotask/transaction/0.original", "backup");
    project.Write(".tasks/.dotask/transaction/1.new", "staged");
    new ProjectTaskTransaction(TaskDirectory.Locate(project.Root)).Apply([], CancellationToken.None);
    Assert.False(Directory.Exists(Path.Combine(project.Tasks, ".dotask")));
  }

  [Fact]
  public async Task OfficialCatalogMatchesSourcesAndEveryPublishedTaskCompiles()
  {
    using var project = new TestProject();
    var assembly = typeof(SharedTaskTests).Assembly;
    foreach (var name in assembly.GetManifestResourceNames().Where(n => n.StartsWith("Shared/", StringComparison.Ordinal))) {
      using var stream = assembly.GetManifestResourceStream(name)!;
      using var reader = new StreamReader(stream);
      project.Write(".tasks/_/" + name[7..].Replace('\\', '/'), await reader.ReadToEndAsync());
    }
    var root = Path.Combine(project.Tasks, "_");
    var catalog = JsonSerializer.Deserialize<SharedCatalog>(File.ReadAllBytes(Path.Combine(root, "catalog.json")), SharedTaskJson.Options)!;
    SharedTaskStore.ValidateCatalog(catalog);
    foreach (var task in catalog.Tasks) {
      foreach (var file in task.Files) {
        Assert.Equal(file.Sha256, SharedTaskFiles.HashFile(Path.Combine(root, file.Path)));
      }
    }
    var targets = new TargetCatalog(project.Tasks);
    Assert.Equal(catalog.Tasks.Length, targets.Targets.Count);
    var compiler = new TargetCompiler();
    foreach (var task in targets.Targets) {
      var result = await compiler.CompileAsync(task, CancellationToken.None);
      Assert.True(result.Success, task.Name + ": " + result.Diagnostics);
    }
  }

  [Fact]
  public async Task PrivateSupportFilesAreTrackedAndSharedOwnershipPreventsPrematureRemoval()
  {
    using var fixture = new Fixture();
    fixture.Private("tools/one", metadata: "/// <requires file=\"tools/_support/Helper.cs\" />");
    fixture.Private("tools/two", metadata: "/// <requires file=\"tools/_support/Helper.cs\" />");
    var support = Path.Combine(fixture.Options.PrivateDirectory, "tools/_support/Helper.cs");
    Directory.CreateDirectory(Path.GetDirectoryName(support)!);
    File.WriteAllText(support, "public static class Helper { }");
    Assert.Equal(0, (await fixture.Run("--add", "PRIVATE-TASKS/tools/{one,two}")).Code);
    Assert.Equal(2, new TargetCatalog(fixture.Project.Tasks).Targets.Count);
    var installed = Path.Combine(fixture.Project.Tasks, "private-tasks/tools/_support/Helper.cs");
    Assert.True(File.Exists(installed));
    Assert.Equal(0, (await fixture.Run("--remove", "private-tasks/tools/one")).Code);
    Assert.True(File.Exists(installed));
    File.AppendAllText(installed, "// local");
    Assert.Equal(1, (await fixture.Run("--remove", "private-tasks/tools/two")).Code);
    Assert.True(File.Exists(fixture.Installed("private-tasks/tools/two")));
    File.WriteAllBytes(installed, File.ReadAllBytes(support));
    Assert.Equal(0, (await fixture.Run("--remove", "private-tasks/tools/two")).Code);
    Assert.False(File.Exists(installed));
    Assert.True(File.Exists(support));
  }

  [Fact]
  public async Task XmlTaskDependenciesAreCopiedAndShownInHelpButNeverAutomaticallyExecuted()
  {
    using var fixture = new Fixture();
    fixture.Private("tools/run", metadata: "/// <requires task=\"tools/help\" />");
    fixture.Private("tools/help", "throw new TaskException(\"Do not automatically execute me\");");
    var added = await fixture.Run("--add", "private-tasks/tools/run");
    Assert.True(added.Code == 0, added.Error + added.Output);
    Assert.True(File.Exists(fixture.Installed("private-tasks/tools/help")));
    Assert.Equal(2, fixture.Lock().Tasks.Count);
    var help = await fixture.Run("run", "--help");
    Assert.Equal(0, help.Code);
    Assert.Contains("task: tools/help", help.Output);
    var run = await fixture.Project.RunAsync("run");
    Assert.True(run.ExitCode == 0, run.StandardError);
    Assert.Contains("hello", run.StandardOutput);
    Assert.DoesNotContain("Do not automatically", run.StandardError);
  }

  [Theory]
  [InlineData("/// <requires task=\"tools/missing\" />")]
  [InlineData("/// <requires file=\"../outside.txt\" />")]
  [InlineData("/// <requires file=\"tools/missing.txt\" />")]
  [InlineData("/// <requires file=\"tools/a.txt\" task=\"tools/check\" />")]
  public async Task InvalidPrivateXmlDependenciesFailBeforeProjectMutation( string metadata )
  {
    using var fixture = new Fixture();
    var original = fixture.Private("tools/run", metadata: metadata);
    var bytes = File.ReadAllBytes(original);
    Assert.Equal(1, (await fixture.Run("--add", "private-tasks/tools/run")).Code);
    Assert.Empty(Directory.GetFileSystemEntries(fixture.Project.Tasks));
    Assert.False(File.Exists(fixture.LockPath));
    Assert.Equal(bytes, File.ReadAllBytes(original));
  }

  [Fact]
  public async Task LegacyPrivateManifestFailsWithMigrationInstructionsWithoutChangingOriginals()
  {
    using var fixture = new Fixture();
    var original = fixture.Private("tools/run");
    var manifest = Path.ChangeExtension(original, ".task.json");
    const string contents = "{\"requires\":[\"tools/check\"]}";
    File.WriteAllText(manifest, contents);
    var result = await fixture.Run("--add", "private-tasks/tools/run");
    Assert.Equal(1, result.Code);
    Assert.Contains("XML <requires task=", result.Error);
    Assert.Contains(manifest, result.Error);
    Assert.Equal(contents, File.ReadAllText(manifest));
    Assert.Empty(Directory.GetFileSystemEntries(fixture.Project.Tasks));
    Assert.False(File.Exists(fixture.LockPath));
  }

  [Fact]
  public async Task ConcurrentAddsSerializeAndPreserveBothTrackingEntries()
  {
    using var fixture = new Fixture();
    fixture.Private("tools/one");
    fixture.Private("tools/two");
    var results = await Task.WhenAll(fixture.Run("--add", "private-tasks/tools/one"), fixture.Run("--add", "private-tasks/tools/two"));
    Assert.All(results, result => Assert.True(result.Code == 0, result.Error + result.Output));
    Assert.Equal(2, fixture.Lock().Tasks.Count);
  }

  [Fact]
  public async Task CustomDirectoryUsesRootConfigAndLockCannotManageAnotherDirectory()
  {
    using var fixture = new Fixture();
    fixture.Project.Write(".dotasks.yaml", "name: anchored\nsettings: {}\n");
    fixture.Private("tools/one");
    var result = await fixture.Run("--use-dir", "build/tasks", "--add", "private-tasks/tools/one");
    Assert.True(result.Code == 0, result.Error);
    var directory = TaskDirectory.Locate(fixture.Project.Root, "build/tasks");
    Assert.Equal(fixture.Project.Root, directory.RootDirectory);
    Assert.True(File.Exists(Path.Combine(fixture.Project.Root, "build/tasks/private-tasks/tools/one.cs")));
    Assert.Equal(1, (await fixture.Run("--remove", "private-tasks/tools/one")).Code);
    Assert.Equal(0, (await fixture.Run("--use-dir", "build/tasks", "--remove", "private-tasks/tools/one")).Code);
  }

  [Fact]
  public async Task UnrecognizedInternalDirectoryIsNeverRemovedOrAdopted()
  {
    using var fixture = new Fixture();
    fixture.Private("tools/one");
    var handwritten = fixture.Project.Write(".tasks/.dotask/transaction/notes.txt", "keep this");
    Assert.Equal(1, (await fixture.Run("--add", "private-tasks/tools/one")).Code);
    Assert.Equal("keep this", File.ReadAllText(handwritten));
    Assert.False(File.Exists(fixture.Installed("private-tasks/tools/one")));
  }

  [Fact]
  public async Task UnsupportedCatalogRuntimeAndDependencyCyclesFailWithoutMutations()
  {
    using var fixture = new Fixture();
    fixture.Online("tools/one", dependencies: ["tools/two"]);
    fixture.Online("tools/two", dependencies: ["tools/one"]);
    fixture.WriteCatalog();
    Assert.Equal(1, (await fixture.Run("--add", "tools/one")).Code);
    Assert.False(File.Exists(fixture.LockPath));
    fixture.CatalogTasks[0] = fixture.CatalogTasks[0] with { Runtime = "rust" };
    fixture.WriteCatalog();
    Assert.Equal(1, (await fixture.Run("--save", "tools/one")).Code);
    Assert.False(File.Exists(fixture.LockPath));
  }

  [Fact]
  public async Task ManagementCompletionUsesOnlyLocalMetadataAndInstalledTracking()
  {
    using var fixture = new Fixture();
    fixture.Private("tools/hello");
    var suggestions = CompletionEngine.Complete("dotask --add private-tasks/tools/h", fixture.Project.Root,
      sharedTaskOptions: fixture.Options);
    Assert.Contains(suggestions, c => c.Value == "private-tasks/tools/hello");
    Assert.False(Directory.Exists(fixture.Options.CacheDirectory));
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/hello")).Code);
    Directory.Delete(fixture.Options.PrivateDirectory, true);
    Directory.Delete(fixture.Options.CacheDirectory, true);
    suggestions = CompletionEngine.Complete("dotask --remove private-tasks/tools/h", fixture.Project.Root,
      sharedTaskOptions: fixture.Options);
    Assert.Contains(suggestions, c => c.Value == "private-tasks/tools/hello");
    Assert.False(Directory.Exists(fixture.Options.CacheDirectory));
  }

  [Fact]
  public void RootConfigAnchorsAnEmptyProjectAndNestedTaskNamesStayUnambiguous()
  {
    using var project = new TestProject();
    Directory.Delete(project.Tasks);
    project.Write(".dotasks.yaml", "name: Root project\nsettings: { message: hello }");
    var child = Path.Combine(project.Root, "src");
    Directory.CreateDirectory(child);
    var directory = TaskDirectory.Locate(child);
    Assert.Equal(project.Root, directory.RootDirectory);
    Assert.Empty(new TargetCatalog(directory.DirectoryPath).Targets);
    Assert.Equal("Root project", ProjectConfiguration.Load(directory.DirectoryPath).Name);
    project.Target("_/dotnet/build", metadata: "/// <summary>Build.</summary>");
    var catalog = new TargetCatalog(project.Tasks);
    Assert.Equal("_/dotnet/build", catalog.Get("build").Name);
    Assert.Same(catalog.Get("build"), catalog.Get("dotnet/build"));
    Assert.Same(catalog.Get("build"), catalog.Get("DOTNET-BUILD"));
    project.Target("second-source/dotnet/build");
    catalog = new TargetCatalog(project.Tasks);
    Assert.Throws<TaskException>(() => catalog.Get("build"));
    Assert.Throws<TaskException>(() => catalog.Get("dotnet/build"));
    project.Target("build");
    project.Write(".tasks/_/dotnet/_support/Helper.cs", "invalid C# helper");
    catalog = new TargetCatalog(project.Tasks);
    Assert.Equal("build", catalog.Get("build").Name);
    Assert.Equal(3, catalog.Targets.Count);
    Assert.Contains(CompletionEngine.Complete("dotask second-source/dotnet/b", project.Root), c => c.Value == "second-source/dotnet/build");
  }

  [Fact]
  public void RootAndLegacyConfigCannotSilentlyMerge()
  {
    using var project = new TestProject();
    project.Write(".dotasks.yaml", "settings: {}");
    project.Write(".tasks/config.yaml", "settings: {}");
    Assert.Contains("Both", Assert.Throws<TaskException>(() => ProjectConfiguration.Load(project.Tasks)).Message);
  }

  [Fact]
  public async Task AddCopiesPrivateTasksAndPreservesHandwrittenConfiguration()
  {
    using var fixture = new Fixture();
    var original = fixture.Private("my-tasks/sortini");
    var yaml = "# handwritten comment\nname: 'My project'\nsettings: { value: 42 }\n";
    fixture.Project.Write(".dotasks.yaml", yaml);
    var result = await fixture.Run("--add", "private-tasks/my-tasks/sortini");
    Assert.Equal(0, result.Code);
    Assert.Equal(File.ReadAllBytes(original), File.ReadAllBytes(fixture.Installed("private-tasks/my-tasks/sortini")));
    Assert.Equal(yaml, File.ReadAllText(Path.Combine(fixture.Project.Root, ".dotasks.yaml")));
    var tracking = fixture.Lock();
    Assert.Equal(SharedTaskFiles.HashFile(original), tracking.Tasks["private-tasks/my-tasks/sortini"].Files["private-tasks/my-tasks/sortini.cs"]);
    Assert.DoesNotContain(fixture.Root, File.ReadAllText(fixture.LockPath));
    var before = File.ReadAllBytes(fixture.LockPath);
    Assert.Equal(0, (await fixture.Run("--sync")).Code);
    Assert.Equal(before, File.ReadAllBytes(fixture.LockPath));
    Assert.True(File.Exists(original));
  }

  [Theory]
  [InlineData(false)]
  [InlineData(true)]
  public async Task AddNeverOverwritesUntrackedFilesEvenWhenBytesMatch( bool identical )
  {
    using var fixture = new Fixture();
    var original = fixture.Private("tools/hello");
    var existing = identical ? File.ReadAllText(original) : "// handwritten task";
    fixture.Project.Write(".tasks/private-tasks/tools/hello.cs", existing);
    var result = await fixture.Run("--add", "private-tasks/tools/hello");
    Assert.Equal(1, result.Code);
    Assert.Contains("untracked", result.Output);
    Assert.Equal(existing, File.ReadAllText(fixture.Installed("private-tasks/tools/hello")));
    Assert.False(File.Exists(fixture.LockPath));
  }

  [Fact]
  public async Task AddSyncAndRemoveProtectLocalEditsAndDeletion()
  {
    using var fixture = new Fixture();
    var source = fixture.Private("tools/hello");
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/hello")).Code);
    var originalLock = File.ReadAllBytes(fixture.LockPath);
    File.AppendAllText(fixture.Installed("private-tasks/tools/hello"), "\n// local edit\n");
    var edited = File.ReadAllText(fixture.Installed("private-tasks/tools/hello"));
    Assert.Equal(1, (await fixture.Run("--add", "private-tasks/tools/hello")).Code);
    Assert.Equal(1, (await fixture.Run("--remove", "private-tasks/tools/hello")).Code);
    Assert.Equal(0, (await fixture.Run("--sync")).Code);
    File.AppendAllText(source, "\n// upstream change\n");
    var conflict = await fixture.Run("--sync");
    Assert.Equal(1, conflict.Code);
    Assert.Contains("Locally modified", conflict.Output);
    Assert.Equal(edited, File.ReadAllText(fixture.Installed("private-tasks/tools/hello")));
    Assert.Equal(originalLock, File.ReadAllBytes(fixture.LockPath));
    Assert.NotEmpty(Directory.GetFiles(fixture.Options.CacheDirectory, "*.base", SearchOption.AllDirectories));
    File.Delete(fixture.Installed("private-tasks/tools/hello"));
    Assert.Equal(1, (await fixture.Run("--sync")).Code);
    Assert.False(File.Exists(fixture.Installed("private-tasks/tools/hello")));
    Assert.Equal(originalLock, File.ReadAllBytes(fixture.LockPath));
  }

  [Fact]
  public async Task ReviewedMergeUpdatesBaselineWithoutOverwritingLocalContent()
  {
    using var fixture = new Fixture();
    var original = fixture.Private("tools/hello");
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/hello")).Code);
    File.AppendAllText(original, "\n// upstream\n");
    File.AppendAllText(fixture.Installed("private-tasks/tools/hello"), "\n// merged local and upstream\n");
    var merged = File.ReadAllBytes(fixture.Installed("private-tasks/tools/hello"));
    Assert.Equal(1, (await fixture.Run("--sync", "private-tasks/tools/hello")).Code);
    var result = await fixture.Run("--sync", "private-tasks/tools/hello", "--accept-merge");
    Assert.Equal(0, result.Code);
    Assert.Equal(merged, File.ReadAllBytes(fixture.Installed("private-tasks/tools/hello")));
    Assert.Equal(SharedTaskFiles.HashFile(original), fixture.Lock().Tasks["private-tasks/tools/hello"].Files["private-tasks/tools/hello.cs"]);
    Assert.Equal(0, (await fixture.Run("--sync")).Code);
    Assert.Equal(1, (await fixture.Run("--remove", "private-tasks/tools/hello")).Code);
  }

  [Fact]
  public async Task AcceptMergeCannotAcknowledgeAnUnreviewedNewerRevision()
  {
    using var fixture = new Fixture();
    var source = fixture.Private("tools/hello");
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/hello")).Code);
    File.AppendAllText(fixture.Installed("private-tasks/tools/hello"), "\n// local\n");
    File.AppendAllText(source, "\n// upstream one\n");
    Assert.Equal(1, (await fixture.Run("--sync")).Code);
    var before = File.ReadAllBytes(fixture.LockPath);
    File.AppendAllText(source, "\n// upstream two, not yet reviewed\n");
    Assert.Equal(1, (await fixture.Run("--sync", "private-tasks/tools/hello", "--accept-merge")).Code);
    Assert.Equal(before, File.ReadAllBytes(fixture.LockPath));
  }

  [Fact]
  public async Task UntouchedFilesSyncAndCanBeRemovedAgainstTheirInstalledVersionOffline()
  {
    using var fixture = new Fixture();
    var original = fixture.Private("tools/hello");
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/hello")).Code);
    File.AppendAllText(original, "\n// new upstream\n");
    Assert.Equal(0, (await fixture.Run("--sync")).Code);
    Assert.Equal(File.ReadAllBytes(original), File.ReadAllBytes(fixture.Installed("private-tasks/tools/hello")));
    fixture.Project.Write(".tasks/private-tasks/tools/handwritten.cs", "// keep me");
    File.AppendAllText(original, "\n// upstream changed again\n");
    Directory.Delete(fixture.Options.CacheDirectory, true);
    Assert.Equal(0, (await fixture.Run("--remove", "private-tasks/tools/hello")).Code);
    Assert.False(File.Exists(fixture.Installed("private-tasks/tools/hello")));
    Assert.True(File.Exists(fixture.Installed("private-tasks/tools/handwritten")));
    Assert.True(File.Exists(original));
    Assert.Equal(1, (await fixture.Run("--remove", "private-tasks/tools/handwritten")).Code);
  }

  [Fact]
  public async Task DryRunDoesNotCreateProjectFilesAndWildcardsAreFixedSelections()
  {
    using var fixture = new Fixture();
    Directory.Delete(fixture.Project.Tasks);
    fixture.Private("tools/one");
    fixture.Private("tools/two");
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/*", "--dry-run")).Code);
    Assert.Empty(Directory.GetFileSystemEntries(fixture.Project.Root));
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/{one,two}")).Code);
    fixture.Private("tools/three");
    Assert.Equal(0, (await fixture.Run("--sync")).Code);
    Assert.False(File.Exists(fixture.Installed("private-tasks/tools/three")));
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/*")).Code);
    Assert.True(File.Exists(fixture.Installed("private-tasks/tools/three")));
  }

  [Fact]
  public async Task MissingOrInvalidTrackingNeverAdoptsOrDeletesProjectFiles()
  {
    using var fixture = new Fixture();
    fixture.Private("tools/hello");
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/hello")).Code);
    var task = File.ReadAllBytes(fixture.Installed("private-tasks/tools/hello"));
    File.WriteAllText(fixture.LockPath, "version: 1\nversion: 1\ntasks: {}\n");
    Assert.Equal(1, (await fixture.Run("--remove", "private-tasks/tools/hello")).Code);
    Assert.Equal(task, File.ReadAllBytes(fixture.Installed("private-tasks/tools/hello")));
    File.Delete(fixture.LockPath);
    Assert.Equal(1, (await fixture.Run("--remove", "private-tasks/tools/hello")).Code);
    Assert.Equal(1, (await fixture.Run("--add", "private-tasks/tools/hello")).Code);
  }

  [Fact]
  public async Task AConflictPreventsPartialChangesAcrossTheWholeBatch()
  {
    using var fixture = new Fixture();
    fixture.Private("tools/one");
    fixture.Private("tools/two");
    fixture.Project.Write(".tasks/private-tasks/tools/two.cs", "// keep");
    Assert.Equal(1, (await fixture.Run("--add", "private-tasks/tools/*")).Code);
    Assert.False(File.Exists(fixture.Installed("private-tasks/tools/one")));
    Assert.False(File.Exists(fixture.LockPath));
  }

  [Fact]
  public async Task OnlineDownloadIsSelectiveVerifiedAndReusableOffline()
  {
    using var fixture = new Fixture();
    fixture.Online("dotnet/one");
    fixture.Online("dotnet/two");
    fixture.WriteCatalog();
    Assert.Equal(0, (await fixture.Run("--save", "dotnet/one")).Code);
    Assert.True(File.Exists(Path.Combine(fixture.Options.CacheDirectory, "_/dotnet/one.cs")));
    Assert.False(File.Exists(Path.Combine(fixture.Options.CacheDirectory, "_/dotnet/two.cs")));
    Directory.Delete(fixture.OnlineDirectory, true);
    Assert.Equal(0, (await fixture.Run("--add", "dotnet/one")).Code);
    Assert.True(File.Exists(fixture.Installed("_/dotnet/one")));
    var originalLock = File.ReadAllBytes(fixture.LockPath);
    Assert.Equal(1, (await fixture.Run("--sync")).Code);
    Assert.Equal(originalLock, File.ReadAllBytes(fixture.LockPath));
  }

  [Fact]
  public async Task CorruptOnlineFilesAndUnsafeCatalogPathsNeverReachTheProject()
  {
    using var fixture = new Fixture();
    fixture.Online("dotnet/build");
    fixture.WriteCatalog();
    File.AppendAllText(Path.Combine(fixture.OnlineDirectory, "dotnet/build.cs"), "tampered");
    var result = await fixture.Run("--add", "dotnet/build");
    Assert.Equal(1, result.Code);
    Assert.Contains("SHA-256", result.Error);
    Assert.False(File.Exists(fixture.LockPath));
    var task = fixture.CatalogTasks[0];
    fixture.CatalogTasks[0] = task with { Files = [new("../outside.cs", task.Files[0].Sha256)] };
    fixture.WriteCatalog();
    Assert.Equal(1, (await fixture.Run("--save", "dotnet/build")).Code);
    Assert.False(File.Exists(Path.Combine(fixture.Root, "outside.cs")));
  }

  [Fact]
  public async Task RequiredTasksAndSharedSupportFilesAreInstalledAndRemovedSafely()
  {
    using var fixture = new Fixture();
    fixture.Online("dotnet/restore");
    fixture.Online("dotnet/build", dependencies: ["dotnet/restore"]);
    fixture.WriteCatalog();
    Assert.Equal(0, (await fixture.Run("--add", "dotnet/build")).Code);
    Assert.Equal(2, fixture.Lock().Tasks.Count);
    Assert.Equal(1, (await fixture.Run("--remove", "dotnet/restore")).Code);
    Assert.True(File.Exists(fixture.Installed("_/dotnet/restore")));
    Assert.Equal(0, (await fixture.Run("--remove", "dotnet/{build,restore}")).Code);
    Assert.Empty(fixture.Lock().Tasks);
  }

  [Fact]
  public async Task SymbolicLinkDestinationsAreNeverFollowed()
  {
    if (OperatingSystem.IsWindows()) {
      return;
    }
    using var fixture = new Fixture();
    fixture.Private("tools/hello");
    var outside = Path.Combine(fixture.Root, "outside");
    Directory.CreateDirectory(outside);
    Directory.CreateSymbolicLink(Path.Combine(fixture.Project.Tasks, "private-tasks"), outside);
    Assert.Equal(1, (await fixture.Run("--add", "private-tasks/tools/hello")).Code);
    Assert.Empty(Directory.GetFileSystemEntries(outside));
    Assert.False(File.Exists(fixture.LockPath));
  }

  [Fact]
  public void TransactionRollsBackFilesAndTrackingAfterFailure()
  {
    using var project = new TestProject();
    project.Write(".tasks/tools/one.cs", "original");
    project.Write(".dotasks-lock.yaml", "old lock");
    var transaction = new ProjectTaskTransaction(TaskDirectory.Locate(project.Root));
    TaskFileChange[] changes = [
      new(".tasks/tools/one.cs", SharedTaskFiles.Hash("original"), "new"u8.ToArray()),
      new(".tasks/tools/two.cs", null, "new file"u8.ToArray()),
      new(".dotasks-lock.yaml", SharedTaskFiles.Hash("old lock"), "new lock"u8.ToArray())];
    Assert.Throws<IOException>(() => transaction.Apply(changes, CancellationToken.None, index =>
    {
      if (index == 1) {
        throw new IOException("Simulated interrupted update");
      }
    }));
    Assert.Equal("original", File.ReadAllText(Path.Combine(project.Tasks, "tools/one.cs")));
    Assert.Equal("old lock", File.ReadAllText(Path.Combine(project.Root, ".dotasks-lock.yaml")));
    Assert.False(File.Exists(Path.Combine(project.Tasks, "tools/two.cs")));
    Assert.False(transaction.HasPending);
    Assert.False(Directory.Exists(Path.Combine(project.Tasks, ".dotask")));
  }

  [Fact]
  public void InterruptedTransactionRecoversButNeverOverwritesSubsequentUserEdits()
  {
    using var project = new TestProject();
    project.Write(".tasks/tools/one.cs", "new");
    project.Write(".tasks/.dotask/owner", "dotask shared-task transaction state v1\n");
    project.Write(".tasks/.dotask/transaction/0.original", "original");
    var journal = new ProjectTaskTransaction.Journal(1,
      [new(".tasks/tools/one.cs", SharedTaskFiles.Hash("original"), SharedTaskFiles.Hash("new"), "0.original")]);
    project.Write(".tasks/.dotask/transaction/journal.json", JsonSerializer.Serialize(journal, SharedTaskJson.Options));
    var transaction = new ProjectTaskTransaction(TaskDirectory.Locate(project.Root));
    project.Write(".tasks/tools/one.cs", "later edit");
    Assert.Throws<TaskException>(() => transaction.Recover());
    Assert.Equal("later edit", File.ReadAllText(Path.Combine(project.Tasks, "tools/one.cs")));
    Assert.True(transaction.HasPending);
    project.Write(".tasks/tools/one.cs", "new");
    transaction.Recover();
    Assert.Equal("original", File.ReadAllText(Path.Combine(project.Tasks, "tools/one.cs")));
    Assert.False(transaction.HasPending);
    Assert.False(Directory.Exists(Path.Combine(project.Tasks, ".dotask")));
  }

  [Fact]
  public async Task HttpCatalogAndFilesUseSelectiveRequestsAndVerifyHashes()
  {
    using var fixture = new Fixture();
    fixture.Online("tools/hello");
    fixture.Online("tools/unused");
    var requests = new List<string>();
    using var client = new HttpClient(new Handler(request =>
    {
      var path = request.RequestUri!.AbsolutePath.Split("/shared-tasks/")[1];
      requests.Add(path);
      var bytes = path == "catalog.json" ? JsonSerializer.SerializeToUtf8Bytes(new SharedCatalog(1, fixture.CatalogTasks.ToArray()), SharedTaskJson.Options)
        : File.ReadAllBytes(Path.Combine(fixture.OnlineDirectory, path));
      return new HttpResponseMessage(HttpStatusCode.OK) { Content = new ByteArrayContent(bytes) };
    }));
    var store = new SharedTaskStore(fixture.Options with { OnlineDirectory = null }, client);
    var catalog = await store.CatalogAsync("_", true, CancellationToken.None);
    await store.SaveAsync("_", catalog.Tasks[0], CancellationToken.None);
    Assert.Equal(["catalog.json", "tools/hello.cs"], requests);
  }

  [Fact]
  public async Task FreshCheckoutRunsQualifiedNestedTasksWithoutAnySourceOrCache()
  {
    using var fixture = new Fixture();
    fixture.Private("tools/child", "Console.WriteLine(BuildContext.Current.Config.Get<string>(\"message\") + \":\" + BuildContext.Current.TargetName);");
    fixture.Private("tools/parent", "await BuildContext.Current.ExecTargetAsync(\"private-tasks/tools/child\");", async: true);
    fixture.Project.Write(".dotasks.yaml", "settings: { message: offline }");
    Assert.Equal(0, (await fixture.Run("--add", "private-tasks/tools/{parent,child}")).Code);
    Directory.Delete(fixture.Options.PrivateDirectory, true);
    Directory.Delete(fixture.Options.CacheDirectory, true);
    using var clone = new TestProject();
    foreach (var file in Directory.GetFiles(fixture.Project.Root, "*", SearchOption.AllDirectories)
      .Where(f => !f.Contains(Path.DirectorySeparatorChar + ".dotask" + Path.DirectorySeparatorChar, StringComparison.Ordinal))) {
      var target = Path.Combine(clone.Root, Path.GetRelativePath(fixture.Project.Root, file));
      Directory.CreateDirectory(Path.GetDirectoryName(target)!);
      File.Copy(file, target);
    }
    var result = await clone.RunAsync("parent");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Contains("offline:private-tasks/tools/child", result.StandardOutput);
    Assert.False(Directory.Exists(fixture.Options.CacheDirectory));
    Assert.False(Directory.Exists(fixture.Options.PrivateDirectory));
  }

  private sealed class Handler( Func<HttpRequestMessage, HttpResponseMessage> response ) : HttpMessageHandler
  {
    protected override Task<HttpResponseMessage> SendAsync( HttpRequestMessage request, CancellationToken cancellationToken ) => Task.FromResult(response(request));
  }

  private sealed class Fixture : IDisposable
  {
    public TestProject Project { get; } = new();
    public string Root { get; } = Path.Combine(Path.GetTempPath(), "dotask-shared-tests", Guid.NewGuid().ToString("N"));
    public SharedTaskOptions Options { get; }
    public string OnlineDirectory => Path.Combine(Root, "online");
    public string LockPath => Path.Combine(Project.Root, ".dotasks-lock.yaml");
    public List<SharedTask> CatalogTasks { get; } = [];

    public Fixture() => Options = new(Path.Combine(Root, "cache"), Path.Combine(Root, "private"), OnlineDirectory);
    public string Installed( string id ) => Path.Combine(Project.Tasks, id + ".cs");
    public TaskLock Lock() => TaskLock.Read(File.ReadAllBytes(LockPath), ".tasks");
    public string Private( string id, string body = "Console.WriteLine(\"hello\");", bool async = false, string metadata = "" )
      => WriteSource(Options.PrivateDirectory, id, body, async, metadata);
    public void Online( string id, string body = "Console.WriteLine(\"hello\");", string[]? dependencies = null )
    {
      var path = WriteSource(OnlineDirectory, id, body, false);
      CatalogTasks.Add(new(id, id + ".cs", "Example task", "csharp", [new(id + ".cs", SharedTaskFiles.HashFile(path)!)], dependencies ?? []));
    }
    public void WriteCatalog() => File.WriteAllBytes(Path.Combine(OnlineDirectory, "catalog.json"),
      JsonSerializer.SerializeToUtf8Bytes(new SharedCatalog(1, CatalogTasks.ToArray()), SharedTaskJson.Options));
    private static string WriteSource( string root, string id, string body, bool async, string metadata = "" )
    {
      var path = Path.Combine(root, id + ".cs");
      Directory.CreateDirectory(Path.GetDirectoryName(path)!);
      File.WriteAllText(path, $$"""
        using DoTask;
        /// <summary>Example task.</summary>
        {{metadata}}
        public static class Target
        {
          public static {{(async ? "async Task" : "void")}} Main()
          {
            {{body}}
          }
        }
        """);
      return path;
    }
    public async Task<(int Code, string Output, string Error)> Run( params string[] args )
    {
      using var output = new StringWriter();
      using var error = new StringWriter();
      var code = await CliApplication.RunAsync(args, Project.Root, output, error, sharedTaskOptions: Options);
      return (code, output.ToString(), error.ToString());
    }
    public void Dispose()
    {
      Project.Dispose();
      if (Directory.Exists(Root)) {
        Directory.Delete(Root, true);
      }
    }
  }
}

using System.Text;
using System.Text.Json;

namespace DoTask.Tests;

public sealed class InstallationTests
{
  [Fact]
  public void CommandDirectoryPrecedenceIsExplicitThenBinThenPlatformDefault()
  {
    Assert.Equal("explicit", UserInstaller.ResolveBinDirectory("explicit", "personal"));
    Assert.Equal("personal", UserInstaller.ResolveBinDirectory(null, "personal"));
    Assert.Equal(UserInstaller.ResolveBinDirectory(null, null), UserInstaller.ResolveBinDirectory(null, "  "));
    Assert.EndsWith("bin", UserInstaller.ResolveBinDirectory(null, null));
  }

  [Fact]
  public async Task FreshRepeatAndSameVersionUpdatesKeepImmutableBuildsAndSwitchTheCommand()
  {
    using var fixture = new Fixture();
    var first = await UserInstaller.InstallAsync(fixture.Definition);
    Assert.False(first.Reused);
    Assert.Equal("payload one", File.ReadAllText(Path.Combine(first.InstallDirectory, fixture.Executable)));
    Assert.Contains("1.2.3-", first.InstallDirectory);
    Assert.Equal(64, first.Fingerprint.Length);
    AssertActive(first, fixture.Executable);
    var repeat = await UserInstaller.InstallAsync(fixture.Definition);
    Assert.True(repeat.Reused);
    Assert.Equal(first.InstallDirectory, repeat.InstallDirectory);
    fixture.WritePayload("payload two");
    var updated = await UserInstaller.InstallAsync(fixture.Definition);
    Assert.NotEqual(first.Fingerprint, updated.Fingerprint);
    Assert.True(Directory.Exists(first.InstallDirectory));
    Assert.Equal("payload one", File.ReadAllText(Path.Combine(first.InstallDirectory, fixture.Executable)));
    AssertActive(updated, fixture.Executable);
    Assert.False(File.Exists(Path.Combine(fixture.App, ".dotask-pending.json")));
    Assert.Empty(Directory.GetDirectories(fixture.App, ".staging-*"));
    AssertNoLocks(fixture);
  }

  [Fact]
  public async Task FingerprintIncludesResourcesAndExecutablePermissions()
  {
    using var fixture = new Fixture();
    fixture.Project.Write("published/data/settings.json", "one");
    var first = await UserInstaller.InstallAsync(fixture.Definition);
    fixture.Project.Write("published/data/settings.json", "two");
    var second = await UserInstaller.InstallAsync(fixture.Definition);
    Assert.NotEqual(first.Fingerprint, second.Fingerprint);
    if (!OperatingSystem.IsWindows())
    {
      File.SetUnixFileMode(Path.Combine(fixture.Source, fixture.Executable), UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
      var third = await UserInstaller.InstallAsync(fixture.Definition);
      Assert.NotEqual(second.Fingerprint, third.Fingerprint);
    }
  }

  [Fact]
  public async Task ExistingCommandPreventsAllActivationAndNeverAdoptsItsContents()
  {
    using var fixture = new Fixture();
    Directory.CreateDirectory(fixture.Bin);
    var occupied = Path.Combine(fixture.Bin, fixture.Executable);
    File.WriteAllText(occupied, "someone else's command");
    var definition = fixture.Definition with { Commands = [new("free", fixture.Executable), new("sample", fixture.Executable)] };
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(definition));
    Assert.Equal("someone else's command", File.ReadAllText(occupied));
    Assert.False(InstallationFiles.Exists(Path.Combine(fixture.Bin, OperatingSystem.IsWindows() ? "free.exe" : "free")));
    Assert.False(Directory.Exists(fixture.App));
    AssertNoLocks(fixture);
  }

  [Fact]
  public async Task UnownedApplicationDirectoryIsNeverAdopted()
  {
    using var fixture = new Fixture();
    Directory.CreateDirectory(fixture.App);
    var original = Path.Combine(fixture.App, "notes");
    File.WriteAllText(original, "mine");
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition));
    Assert.Equal("mine", File.ReadAllText(original));
    Assert.False(File.Exists(Path.Combine(fixture.App, ".dotask-install.json")));
  }

  [Fact]
  public async Task ModifiedInstalledPayloadIsPreservedAndCannotBeReused()
  {
    using var fixture = new Fixture();
    var first = await UserInstaller.InstallAsync(fixture.Definition);
    var file = Path.Combine(first.InstallDirectory, fixture.Executable);
    File.WriteAllText(file, "locally edited");
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition));
    Assert.Equal("locally edited", File.ReadAllText(file));
    AssertActive(first, fixture.Executable);
  }

  [Fact]
  public async Task ModifiedCommandAndMetadataLinksAreProtected()
  {
    using var fixture = new Fixture();
    var first = await UserInstaller.InstallAsync(fixture.Definition);
    var path = Path.Combine(fixture.Bin, OperatingSystem.IsWindows() ? "sample.shim" : "sample");
    File.Delete(path);
    File.WriteAllText(path, "mine");
    fixture.WritePayload("next build");
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition));
    Assert.Equal("mine", File.ReadAllText(path));
    Assert.Single(Directory.GetDirectories(fixture.App));
  }

  [Fact]
  public async Task UnixSourceLinksAndDanglingCommandLinksAreRejected()
  {
    if (OperatingSystem.IsWindows())
    {
      return;
    }

    using var fixture = new Fixture();
    Directory.CreateDirectory(fixture.Bin);
    var command = Path.Combine(fixture.Bin, "sample");
    File.CreateSymbolicLink(command, "/missing/unowned");
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition));
    Assert.Equal("/missing/unowned", new FileInfo(command).LinkTarget);
    File.Delete(command);
    var outside = fixture.Project.Write("outside", "unrelated");
    File.CreateSymbolicLink(Path.Combine(fixture.Source, "linked"), outside);
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition));
    Assert.Equal("unrelated", File.ReadAllText(outside));
  }

  [Fact]
  public async Task IntentionalBinDirectorySymlinkIsResolved()
  {
    if (OperatingSystem.IsWindows())
    {
      return;
    }

    using var fixture = new Fixture();
    Directory.CreateDirectory(fixture.Bin);
    var alias = Path.Combine(fixture.Project.Root, "bin alias");
    Directory.CreateSymbolicLink(alias, fixture.Bin);
    var result = await UserInstaller.InstallAsync(fixture.Definition with { BinDirectory = alias });
    Assert.Equal(InstallationFiles.PhysicalDirectory(fixture.Bin), result.BinDirectory);
    Assert.True(File.Exists(Path.Combine(alias, "sample")));
  }

  [Theory]
  [InlineData("../escape")]
  [InlineData("CON")]
  [InlineData("name.")]
  [InlineData("bad:name")]
  public async Task InvalidIdentitiesFailBeforeCreatingDestinations(string name)
  {
    using var fixture = new Fixture();
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition with { AppId = name }));
    Assert.False(Directory.Exists(fixture.Bin));
    Assert.False(Directory.Exists(fixture.Definition.InstallRoot));
  }

  [Theory]
  [InlineData("../sample")]
  [InlineData("/sample")]
  [InlineData("dir\\sample")]
  public async Task ExecutableCannotEscapePublishedFiles(string executable)
  {
    using var fixture = new Fixture();
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition with { Commands = [new("sample", executable)] }));
    Assert.False(Directory.Exists(fixture.Bin));
  }

  [Fact]
  public async Task CancellationDoesNotChangeActiveInstallation()
  {
    using var fixture = new Fixture();
    var first = await UserInstaller.InstallAsync(fixture.Definition);
    fixture.WritePayload("next build");
    using var cancelled = new CancellationTokenSource();
    cancelled.Cancel();
    await Assert.ThrowsAnyAsync<OperationCanceledException>(() => UserInstaller.InstallAsync(fixture.Definition, cancelled.Token));
    AssertActive(first, fixture.Executable);
  }

  [Fact]
  public async Task InterruptedActivationRecoversBeforeReinstalling()
  {
    using var fixture = new Fixture();
    var installed = await UserInstaller.InstallAsync(fixture.Definition);
    var receiptPath = Path.Combine(fixture.App, ".dotask-install.json");
    var before = InstallationFiles.Read<InstallReceipt>(receiptPath);
    var desired = InstallationCommands.Desired(Path.Combine(fixture.App, "interrupted-build"), fixture.Definition.Commands);
    var after = before with { ActiveDirectory = "interrupted-build", Commands = desired };
    var changes = InstallationCommands.Changes(before, after);
    var journal = Path.Combine(fixture.App, ".dotask-pending.json");
    InstallationFiles.Write(journal, new InstallJournal(before, after, changes));
    CommandFile.Apply(Path.Combine(fixture.Bin, changes[0].Name), changes[0].After);
    await UserInstaller.InstallAsync(fixture.Definition);
    AssertActive(installed, fixture.Executable);
    Assert.False(File.Exists(journal));

    InstallationFiles.Write(journal, new InstallJournal(before, after, changes));
    var edited = Path.Combine(fixture.Bin, changes[0].Name);
    File.Delete(edited);
    File.WriteAllText(edited, "manual edit during recovery");
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition));
    Assert.Equal("manual edit during recovery", File.ReadAllText(edited));
    Assert.True(File.Exists(journal));
  }

  [Fact]
  public async Task RemovedAliasesAreDeletedOnlyWhenTheyStillMatchOwnership()
  {
    using var fixture = new Fixture();
    var multiple = fixture.Definition with { Commands = [new("sample", fixture.Executable), new("alias", fixture.Executable)] };
    await UserInstaller.InstallAsync(multiple);
    var alias = Path.Combine(fixture.Bin, OperatingSystem.IsWindows() ? "alias.exe" : "alias");
    Assert.True(File.Exists(alias));
    await UserInstaller.InstallAsync(fixture.Definition);
    Assert.False(InstallationFiles.Exists(alias));
  }

  [Fact]
  public async Task InstallationCannotOverlapPublishedOutputOrMoveItsCommandDirectory()
  {
    using var fixture = new Fixture();
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition with { InstallRoot = fixture.Source }));
    await UserInstaller.InstallAsync(fixture.Definition);
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition with { BinDirectory = Path.Combine(fixture.Project.Root, "other bin") }));
  }

  [Fact]
  public async Task CommandCasingChangeFailsWithoutBreakingTheExistingCommand()
  {
    using var fixture = new Fixture();
    var installed = await UserInstaller.InstallAsync(fixture.Definition);
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition with { Commands = [new("Sample", fixture.Executable)] }));
    AssertActive(installed, fixture.Executable);
  }

  [Fact]
  public async Task CompetingInstallerAndMissingEntrypointsFailWithoutActivation()
  {
    using var fixture = new Fixture();
    using (var lease = InstallationFiles.Lock(fixture.Bin))
    {
      await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition));
      Assert.False(File.Exists(Path.Combine(fixture.Definition.InstallRoot!, ".dotask-install.lock")));
      Assert.True(File.Exists(Path.Combine(fixture.Bin, ".dotask-install.lock")));
      Assert.Throws<TaskException>(() => InstallationFiles.Lock(fixture.Bin));
    }
    AssertNoLocks(fixture);
    Assert.False(Directory.Exists(fixture.App));
    File.Delete(Path.Combine(fixture.Source, fixture.Executable));
    fixture.Project.Write("published/data", "a file but not an executable");
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition));
    Assert.False(Directory.Exists(fixture.App));
  }

  [Fact]
  public async Task PreviouslyRetainedLockFilesAreRemovedByTheNextInstallation()
  {
    using var fixture = new Fixture();
    Directory.CreateDirectory(fixture.Definition.InstallRoot!);
    Directory.CreateDirectory(fixture.Bin);
    File.WriteAllText(Path.Combine(fixture.Definition.InstallRoot!, ".dotask-install.lock"), "");
    File.WriteAllText(Path.Combine(fixture.Bin, ".dotask-install.lock"), "");
    await UserInstaller.InstallAsync(fixture.Definition);
    AssertNoLocks(fixture);
  }

  [Fact]
  public async Task NonExecutableUnixOutputIsRejected()
  {
    if (OperatingSystem.IsWindows())
    {
      return;
    }

    using var fixture = new Fixture();
    File.SetUnixFileMode(Path.Combine(fixture.Source, fixture.Executable), UnixFileMode.UserRead | UnixFileMode.UserWrite);
    await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition));
    Assert.False(Directory.Exists(fixture.App));
  }

  [Fact]
  public async Task WindowsShellWrappersAndSidecarsCannotBeSilentlyReplaced()
  {
    if (!OperatingSystem.IsWindows())
    {
      return;
    }

    using var fixture = new Fixture();
    Directory.CreateDirectory(fixture.Bin);
    foreach (var extension in new[] { ".cmd", ".shim" })
    {
      var file = Path.Combine(fixture.Bin, "sample" + extension);
      File.WriteAllText(file, "mine");
      await Assert.ThrowsAsync<TaskException>(() => UserInstaller.InstallAsync(fixture.Definition));
      Assert.Equal("mine", File.ReadAllText(file));
      File.Delete(file);
    }
  }

  private static void AssertNoLocks(Fixture fixture)
  {
    Assert.False(File.Exists(Path.Combine(fixture.Definition.InstallRoot!, ".dotask-install.lock")));
    Assert.False(File.Exists(Path.Combine(fixture.Bin, ".dotask-install.lock")));
  }

  private static void AssertActive(InstallationResult result, string executable)
  {
    var expected = Path.Combine(result.InstallDirectory, executable);
    if (OperatingSystem.IsWindows())
    {
      Assert.Equal($"path = \"{expected}\"\n", File.ReadAllText(Path.Combine(result.BinDirectory, "sample.shim")));
    }
    else
    {
      Assert.Equal(expected, new FileInfo(Path.Combine(result.BinDirectory, "sample")).LinkTarget);
    }
  }

  private sealed class Fixture : IDisposable
  {
    public TestProject Project { get; } = new();
    public string Source => Path.Combine(Project.Root, "published");
    public string Bin => Path.Combine(Project.Root, "command dir");
    public string App => Path.Combine(Project.Root, "programs", "sample");
    public string Executable => OperatingSystem.IsWindows() ? "sample.exe" : "sample";
    public InstallationDefinition Definition => new()
    {
      AppId = "sample",
      Version = "1.2.3",
      SourceDirectory = Source,
      BinDirectory = Bin,
      InstallRoot = Path.Combine(Project.Root, "programs"),
      Commands = [new("sample", Executable)]
    };
    public Fixture() => WritePayload("payload one");
    public void WritePayload(string value)
    {
      var file = Project.Write("published/" + Executable, value);
      if (!OperatingSystem.IsWindows())
      {
        File.SetUnixFileMode(file,
        UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute | UnixFileMode.GroupRead | UnixFileMode.GroupExecute);
      }
    }
    public void Dispose() => Project.Dispose();
  }
}

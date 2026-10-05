using DoTask.Cli.Discovery;
using DoTask.Cli.Metadata;

namespace DoTask.Tests;

public sealed class DiscoveryAndMetadataTests
{
  [Fact]
  public void OfficialSourceIsDiscoveredButUnderscoreHelpersRemainExcluded()
  {
    using var project = new TestProject();
    project.Target("_/dotnet/build");
    foreach (var name in new[] { "_support/helper", "_/dotnet/_support/helper", "_/dotnet/_/helper", "tools/_/helper", "_hidden/build", "_" }) {
      project.Target(name);
    }
    var catalog = new TargetCatalog(project.Tasks);
    var target = Assert.Single(catalog.Targets);
    Assert.Null(target.Error);
    Assert.Equal("_/dotnet/build", target.Name);
    Assert.Same(target, catalog.Get("dotnet/build"));
    Assert.Same(target, catalog.Get("build"));
  }

  [Fact]
  public void DiscoveryOnlyTreatsSupportedCodeExtensionsAsTargets()
  {
    using var project = new TestProject();
    project.Target("run");
    foreach (var extension in new[] { "txt", "md", "target", "targets", "yaml", "yml", "json", "rs", "py", "go" }) {
      project.Write("notes." + extension, "not a task");
      project.Write(".tasks/misc/bootstrap." + extension, "// dotask: 1\n// description: \"This must not be discovered.\"\n// end-dotask\n");
    }
    var target = Assert.Single(new TargetCatalog(project.Tasks).Targets);
    Assert.Equal("run", target.Name);
    Assert.Null(target.Error);
  }

  [Fact]
  public void DiscoveryUsesNearestAncestorAndPreservesInvocation()
  {
    using var project = new TestProject();
    var child = Path.Combine(project.Root, "src", "child");
    Directory.CreateDirectory(child);
    var found = TaskDirectory.Locate(child);
    Assert.Equal(project.Tasks, found.DirectoryPath);
    Assert.Equal(project.Root, found.RootDirectory);
    Assert.Equal(child, found.InvocationDirectory);
    Directory.CreateDirectory(Path.Combine(project.Root, "src", ".tasks"));
    Assert.Equal(Path.Combine(project.Root, "src"), TaskDirectory.Locate(child).RootDirectory);
  }

  [Fact]
  public void ExplicitDirectoryUsesInvocationAndNeverFallsBack()
  {
    using var project = new TestProject();
    var selected = Path.Combine(project.Root, "src", ".abc");
    Directory.CreateDirectory(selected);
    Assert.Equal(selected, TaskDirectory.Locate(Path.GetDirectoryName(selected)!, ".abc").DirectoryPath);
    Assert.Equal(selected, TaskDirectory.Locate(project.Root, selected).DirectoryPath);
    Assert.Throws<TaskException>(() => TaskDirectory.Locate(project.Root, ".missing"));
  }

  [Fact]
  public void MetadataUsesFileHeaderAndPreservesText()
  {
    using var project = new TestProject();
    var file = project.Target("run", metadata: """
      // dotask: 1
      // description: "Run & inspect the app."
      // remarks: "More detail."
      // options:
      //   - {"name": "configuration", "alias": "c", "default": "Debug", "choices": ["Debug", "Release"], "description": "The build mode."}
      // requires:
      //   - {"kind": "tool", "value": "dotnet"}
      //   - {"kind": "setting", "value": "application"}
      //   - {"kind": "task", "value": "dotnet/restore"}
      //   - {"kind": "file", "value": "dotnet/_support/Helper.cs"}
      // capabilities: ["network"]
      // examples: ["dotask run -c Release"]
      // end-dotask
      """);
    var target = MetadataReader.Read(file);
    Assert.Null(target.Error);
    Assert.Equal("run", target.Name);
    Assert.Equal("Run & inspect the app.", target.Description);
    var option = Assert.Single(target.Options);
    Assert.Equal("c", option.Alias);
    Assert.Equal(["Debug", "Release"], option.Choices);
    Assert.Equal(4, target.Requirements.Count);
    Assert.Contains(new Requirement("task", "dotnet/restore"), target.Requirements);
    Assert.Contains(new Requirement("file", "dotnet/_support/Helper.cs"), target.Requirements);
    Assert.Equal("network", Assert.Single(target.Capabilities));
  }

  [Theory]
  [InlineData("// dotask: 1\n// options:\n//   - {\"name\": \"configuration\", \"alias\": \"cc\"}\n// end-dotask\n", "single-letter")]
  [InlineData("// dotask: 1\n// options:\n//   - {\"name\": \"help\"}\n// end-dotask\n", "reserved")]
  [InlineData("// dotask: 1\n// options:\n//   - {\"name\": \"a\", \"alias\": \"c\"}\n//   - {\"name\": \"C\"}\n// end-dotask\n", "Duplicate")]
  [InlineData("// dotask: 1\n// options:\n//   - {\"name\": \"x\", \"type\": \"date\"}\n// end-dotask\n", "unsupported type")]
  [InlineData("// dotask: 1\n// [invalid\n// end-dotask\n", "Metadata error")]
  [InlineData("// dotask: 1\n// requires:\n//   - {\"kind\": \"invalid\", \"value\": \"invalid\"}\n// end-dotask\n", "one tool")]
  [InlineData("// dotask: 1\n// requires:\n//   - {\"kind\": \"task\", \"value\": \"\"}\n// end-dotask\n", "one tool")]
  [InlineData("// dotask: 1\n// requires: [{kind: task, value: x, extra: y}]\n// end-dotask", "Unknown")]
  public void InvalidMetadataProducesAnIndividualTargetError( string metadata, string expected )
  {
    using var project = new TestProject();
    var target = MetadataReader.Read(project.Target("broken", metadata: metadata));
    Assert.Contains(expected, target.Error);
    project.Target("good");
    Assert.Equal("good", new TargetCatalog(project.Tasks).Get("GOOD").Name);
  }

  [Fact]
  public void CaseOnlyTargetCollisionsAreErrorsWhenFilesystemAllowsThem()
  {
    using var project = new TestProject();
    project.Target("run");
    project.Target("RUN");
    var catalog = new TargetCatalog(project.Tasks);
    if (catalog.Targets.Count == 2) {
      Assert.All(catalog.Targets, t => Assert.Contains("Ambiguous", t.Error));
      Assert.Throws<TaskException>(() => catalog.Get("run"));
    }
  }

  [Theory]
  [InlineData("run", "run", null)]
  [InlineData("format-check", "format-check", null)]
  [InlineData("dotnet-run", "dotnet-run", null)]
  [InlineData("dotnet run", "dotnet-run", "run")]
  [InlineData("DotNet format-check", "DotNet-format-check", "format-check")]
  public void FilenamesDefineFullNamesAndOptionalShortNames( string filename, string fullName, string? shortName )
  {
    using var project = new TestProject();
    var file = project.Target(filename, metadata: "// dotask: 1\n// description: \"A reusable target.\"\n// end-dotask\n");
    var target = MetadataReader.Read(file);
    Assert.Null(target.Error);
    Assert.Equal(fullName, target.Name);
    Assert.Equal(shortName, target.ShortName);
    Assert.Equal(file, target.FilePath);
    var catalog = new TargetCatalog(project.Tasks);
    Assert.Equal(file, catalog.Get(fullName.ToUpperInvariant()).FilePath);
    if (shortName is not null) {
      Assert.Same(catalog.Get(fullName), catalog.Get(shortName.ToUpperInvariant()));
      Assert.Equal($"{shortName} ({fullName})", catalog.DisplayName(catalog.Get(fullName)));
    }
  }

  [Theory]
  [InlineData("dotnet  run")]
  [InlineData("dotnet run check")]
  [InlineData(" dotnet-run")]
  [InlineData("dotnet-run ")]
  [InlineData("dotnet\trun")]
  [InlineData("dotnet^run")]
  public void GroupedNamesRequireExactlyOneSpaceBetweenValidIdentifiers( string filename )
  {
    using var project = new TestProject();
    var target = MetadataReader.Read(Path.Combine(project.Tasks, filename + ".cs"));
    Assert.Contains("Invalid target filename", target.Error);
  }

  [Fact]
  public void AmbiguousShortNamesRequireFullNamesUnlessAnExactTargetExists()
  {
    using var project = new TestProject();
    project.Target("dotnet format");
    project.Target("rust format");
    var catalog = new TargetCatalog(project.Tasks);
    var error = Assert.Throws<TaskException>(() => catalog.Get("FORMAT"));
    Assert.Contains("dotask dotnet-format", error.Message);
    Assert.Contains("dotask rust-format", error.Message);
    Assert.All(catalog.Targets, t => Assert.Null(t.Error));
    Assert.All(catalog.Targets, t => Assert.Null(catalog.AliasFor(t)));
    Assert.Equal("dotnet-format", catalog.Get("dotnet-format").Name);
    Assert.Equal("rust-format", catalog.Get("rust-format").Name);
    var entryPoint = project.Target("format");
    catalog = new TargetCatalog(project.Tasks);
    Assert.Equal(entryPoint, catalog.Get("FORMAT").FilePath);
    Assert.Null(catalog.AliasFor(catalog.Get("dotnet-format")));
    Assert.Null(catalog.AliasFor(catalog.Get("rust-format")));
  }

  [Theory]
  [InlineData("dotnet run", "dotnet-run")]
  [InlineData("dotnet format-check", "dotnet-format check")]
  public void FullNameCollisionsAreErrorsForEveryAffectedFile( string first, string second )
  {
    using var project = new TestProject();
    project.Target(first);
    project.Target(second);
    var catalog = new TargetCatalog(project.Tasks);
    Assert.All(catalog.Targets, target =>
    {
      Assert.Contains("Ambiguous full target name", target.Error);
      Assert.Contains(first + ".cs", target.Error);
      Assert.Contains(second + ".cs", target.Error);
      Assert.Throws<TaskException>(() => catalog.Get(target.Name));
      Assert.Throws<TaskException>(() => catalog.Get(target.ShortName ?? target.Name));
      Assert.Null(catalog.AliasFor(target));
    });
  }

  [Fact]
  public void FullNamesTakePrecedenceOverOtherTargetsShortNames()
  {
    using var project = new TestProject();
    project.Target("dotnet run");
    project.Target("other dotnet-run");
    var catalog = new TargetCatalog(project.Tasks);
    Assert.Equal("dotnet-run", catalog.Get("dotnet-run").Name);
    Assert.Null(catalog.AliasFor(catalog.Get("other-dotnet-run")));
  }

  [Fact]
  public void ReservedCommandsNeverBecomeShortAliases()
  {
    using var project = new TestProject();
    project.Target("dotnet help");
    project.Target("dotnet completion");
    var catalog = new TargetCatalog(project.Tasks);
    Assert.All(catalog.Targets, target =>
    {
      Assert.Null(target.Error);
      Assert.Null(catalog.AliasFor(target));
      Assert.Same(target, catalog.Get(target.Name));
      Assert.Throws<TaskException>(() => catalog.Get(target.ShortName!));
    });
  }

  [Fact]
  public void MetadataBelongsToFileAndIgnoresCommentLikeTextInCode()
  {
    using var project = new TestProject();
    var file = project.Write(".tasks/test.cs", """
      // dotask: 1
      // description: A documented task.
      // options: [{name: name, alias: n, description: A name.}]
      // end-dotask
      // Ordinary comments and XML documentation remain code documentation.
      public static class Target
      {
        public static void Main() { Console.WriteLine("// description: wrong"); }
      }
      """);
    var target = MetadataReader.Read(file);
    Assert.Null(target.Error);
    Assert.Equal("A documented task.", target.Description);
    Assert.Single(target.Options);
  }
}

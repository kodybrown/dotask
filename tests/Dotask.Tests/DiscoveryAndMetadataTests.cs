using DoTask.Cli.Discovery;
using DoTask.Cli.Metadata;

namespace DoTask.Tests;

public sealed class DiscoveryAndMetadataTests
{
  [Fact]
  public void DiscoveryOnlyTreatsSupportedCodeExtensionsAsTargets()
  {
    using var project = new TestProject();
    project.Target("run");
    foreach (var extension in new[] { "txt", "md", "target", "targets", "yaml", "yml", "json", "rs", "py", "go" })
    {
      project.Write("notes." + extension, "not a task");
      project.Write(".tasks/misc/bootstrap." + extension, "/// <summary>This must not be discovered.</summary>");
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
  public void MetadataUsesClassDocumentationAndRealXmlEntities()
  {
    using var project = new TestProject();
    var file = project.Target("run", metadata: """
      /// <summary>Run &amp; inspect <c>the app</c>.</summary>
      /// <option name="configuration" alias="c" default="Debug" choices="Debug, Release">The build mode.</option>
      /// <requires tool="dotnet" />
      /// <requires setting="application" />
      /// <requires task="dotnet/restore" />
      /// <requires file="dotnet/_support/Helper.cs" />
      /// <capability name="network" />
      /// <remarks>More detail.</remarks>
      /// <example>dotask run -c Release</example>
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
  [InlineData("/// <option name=\"configuration\" alias=\"cc\" />", "single-letter")]
  [InlineData("/// <option name=\"help\" />", "reserved")]
  [InlineData("/// <option name=\"a\" alias=\"c\" /><option name=\"C\" />", "Duplicate")]
  [InlineData("/// <option name=\"x\" type=\"date\" />", "unsupported type")]
  [InlineData("/// <summary>Broken", "Metadata error")]
  [InlineData("/// <requires tool=\"dotnet\" setting=\"x\" />", "one tool")]
  [InlineData("/// <requires task=\"\" />", "one tool")]
  [InlineData("/// <requires task=\"dotnet/restore\" file=\"dotnet/helper.txt\" />", "one tool")]
  public void InvalidMetadataProducesAnIndividualTargetError(string metadata, string expected)
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
    if (catalog.Targets.Count == 2)
    {
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
  public void FilenamesDefineFullNamesAndOptionalShortNames(string filename, string fullName, string? shortName)
  {
    using var project = new TestProject();
    var file = project.Target(filename, metadata: "/// <summary>A reusable target.</summary>");
    var target = MetadataReader.Read(file);
    Assert.Null(target.Error);
    Assert.Equal(fullName, target.Name);
    Assert.Equal(shortName, target.ShortName);
    Assert.Equal(file, target.FilePath);
    var catalog = new TargetCatalog(project.Tasks);
    Assert.Equal(file, catalog.Get(fullName.ToUpperInvariant()).FilePath);
    if (shortName is not null)
    {
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
  public void GroupedNamesRequireExactlyOneSpaceBetweenValidIdentifiers(string filename)
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
  public void FullNameCollisionsAreErrorsForEveryAffectedFile(string first, string second)
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
  public void MetadataCanBeOnMainAndCanUseBlockComments()
  {
    using var project = new TestProject();
    var file = project.Write(".tasks/test.cs", """
      public static class Target
      {
        /** <summary>A documented method.</summary>
         * <option name="name" alias="n">A name.</option>
         */
        public static void Main() { }
      }
      """);
    var target = MetadataReader.Read(file);
    Assert.Null(target.Error);
    Assert.Equal("A documented method.", target.Description);
    Assert.Single(target.Options);
  }
}

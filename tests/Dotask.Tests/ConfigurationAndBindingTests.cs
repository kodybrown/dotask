using DoTask.Cli.Configuration;
using DoTask.Cli.Metadata;
using DoTask.Cli.Parsing;

namespace DoTask.Tests;

public sealed class ConfigurationAndBindingTests
{
  private const string Metadata = """
    /// <summary>Test parameters.</summary>
    /// <option name="configuration" alias="c" choices="Debug,Release" default="Debug" />
    /// <option name="name" alias="n" />
    /// <option name="count" type="int" />
    /// <option name="verbose" alias="v" type="bool" />
    /// <option name="output" type="path" />
    """;

  [Fact]
  public void ProjectIdentityIsOptionalAndSeparateFromSharedSettings()
  {
    using var project = new TestProject();
    Assert.Null(ProjectConfiguration.Load(project.Tasks).Name);
    Assert.Null(ProjectConfiguration.Load(project.Tasks).Description);
    project.Write(".tasks/config.yaml", """
      NAME: "  Example project  "
      Description: |
        First line.
        Second line.
      settings:
        name: Shared value
      """);
    var config = ProjectConfiguration.Load(project.Tasks);
    Assert.Equal("Example project", config.Name);
    Assert.Equal("First line.\nSecond line.", config.Description);
    Assert.Equal("Shared value", new Values(config.Settings, project.Root).Get<string>("name"));
    Assert.Empty(config.DefaultsFor("anything"));
  }

  [Theory]
  [InlineData("version: 1")]
  [InlineData("name: null\ndescription: null")]
  [InlineData("name: ''\ndescription: '  '")]
  public void MissingOrBlankProjectIdentityUsesDefaults(string yaml)
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", yaml);
    var config = ProjectConfiguration.Load(project.Tasks);
    Assert.Null(config.Name);
    Assert.Null(config.Description);
  }

  [Theory]
  [InlineData("name", "42")]
  [InlineData("name", "[one, two]")]
  [InlineData("description", "true")]
  [InlineData("description", "{ text: value }")]
  public void ProjectIdentityRequiresStrings(string key, string value)
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", $"{key}: {value}");
    var error = Assert.Throws<TaskException>(() => ProjectConfiguration.Load(project.Tasks));
    Assert.Equal($"config.yaml {key} must be a string.", error.Message);
  }

  [Fact]
  public void YamlProvidesTypedNestedValuesWithoutRegistration()
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", """
      version: 1
      settings:
        app:
          path: src/My App/app.csproj
          retries: 3
          enabled: true
          literal: "true"
          names: [one, two]
      """);
    var configuration = ProjectConfiguration.Load(project.Tasks);
    var values = new Values(configuration.Settings, project.Root);
    Assert.Equal(3, values.Get<int>("APP.Retries"));
    Assert.True(values.Get<bool>("app.enabled"));
    Assert.Equal("true", values.Get<string>("app.literal"));
    Assert.Equal(["one", "two"], values.Get<string[]>("app.names"));
    Assert.Equal(Path.Combine(project.Root, "src", "My App", "app.csproj"), values.GetPath("app.path"));
    Assert.Empty(configuration.DefaultsFor("anything"));
  }

  [Fact]
  public void DefaultsResolveTargetThenProjectThenCli()
  {
    using var project = new TestProject();
    var target = MetadataReader.Read(project.Target("run", metadata: Metadata));
    project.Write(".tasks/config.yaml", """
      targets:
        RUN:
          defaults:
            Configuration: Release
      """);
    var config = ProjectConfiguration.Load(project.Tasks);
    Assert.Equal("Release", Bind([]).Get<string>("configuration"));
    Assert.Equal("Debug", Bind(["-C", "debug"]).Get<string>("configuration"));
    Values Bind(string[] args) => new(OptionBinder.Bind(target, config, args, project.Root), project.Root);
  }

  [Theory]
  [InlineData("--configuration", "release")]
  [InlineData("-c", "release")]
  [InlineData("--CONFIGURATION=release", null)]
  [InlineData("Configuration=release", null)]
  [InlineData("C=release", null)]
  public void LongAliasAndMakeSyntaxShareCaseInsensitiveValidation(string first, string? second)
  {
    using var project = new TestProject();
    var target = MetadataReader.Read(project.Target("run", metadata: Metadata));
    var result = new Values(OptionBinder.Bind(target, ProjectConfiguration.Empty,
      second is null ? [first] : [first, second], project.Root), project.Root);
    Assert.Equal("Release", result.Get<string>("configuration"));
  }

  [Fact]
  public void ValuesPreserveCaseEqualsEmptyStringsAndNegativeNumbers()
  {
    using var project = new TestProject();
    var target = MetadataReader.Read(project.Target("run", metadata: Metadata));
    var result = new Values(OptionBinder.Bind(target, ProjectConfiguration.Empty,
      ["name=A=B=c", "--count", "-42", "-v", "output=some\\Path.txt"], project.Root), project.Root);
    Assert.Equal("A=B=c", result.Get<string>("name"));
    Assert.Equal(-42, result.Get<int>("count"));
    Assert.True(result.Get<bool>("verbose"));
    Assert.Equal(Path.Combine(project.Root, "some", "Path.txt"), result.GetPath("output"));
    var empty = new Values(OptionBinder.Bind(target, ProjectConfiguration.Empty, ["name="], project.Root), project.Root);
    Assert.Equal("", empty.Get<string>("name"));
  }

  [Theory]
  [InlineData("configuration=invalid")]
  [InlineData("missing=value")]
  [InlineData("count=abc")]
  [InlineData("verbose=maybe")]
  [InlineData("--name")]
  public void InvalidArgumentsAreRejected(string argument)
  {
    using var project = new TestProject();
    var target = MetadataReader.Read(project.Target("run", metadata: Metadata));
    Assert.Throws<TaskException>(() => OptionBinder.Bind(target, ProjectConfiguration.Empty, [argument], project.Root));
  }

  [Fact]
  public void DuplicateAliasesAndMissingRequiredValuesAreRejected()
  {
    using var project = new TestProject();
    var target = MetadataReader.Read(project.Target("run", metadata: Metadata));
    Assert.Throws<TaskException>(() => OptionBinder.Bind(target, ProjectConfiguration.Empty, ["-c", "Debug", "configuration=Release"], project.Root));
    var required = MetadataReader.Read(project.Target("required", metadata: "/// <option name=\"name\" required=\"true\" />"));
    Assert.Throws<TaskException>(() => OptionBinder.Bind(required, ProjectConfiguration.Empty, [], project.Root));
    OptionBinder.Bind(required, ProjectConfiguration.Empty, [], project.Root, requireValues: false);
  }

  [Theory]
  [InlineData("settings: { Name: one, NAME: two }")]
  [InlineData("version: 2")]
  [InlineData("settings: [one, two]")]
  [InlineData("unexpected: true")]
  [InlineData("targets: { run: { description: no } }")]
  [InlineData("settings: &a { next: *a }")]
  public void InvalidConfigurationFailsClearly(string yaml)
  {
    using var project = new TestProject();
    project.Write(".tasks/config.yaml", yaml);
    Assert.Throws<TaskException>(() => ProjectConfiguration.Load(project.Tasks));
  }

  [Fact]
  public void UnknownProjectDefaultsFailInsteadOfBeingIgnored()
  {
    using var project = new TestProject();
    var target = MetadataReader.Read(project.Target("run", metadata: Metadata));
    project.Write(".tasks/config.yaml", "targets: { run: { defaults: { confguration: Release } } }");
    Assert.Throws<TaskException>(() => OptionBinder.Bind(target, ProjectConfiguration.Load(project.Tasks), [], project.Root));
  }
}

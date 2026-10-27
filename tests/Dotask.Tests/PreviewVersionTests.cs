using System.Text;
using System.Xml.Linq;
using DoTask.Shared.DotNet;

namespace DoTask.Tests;

public sealed class PreviewVersionTests
{
  [Theory]
  [InlineData("0.1.0-preview.1", false, false, false, "0.1.2610.2714-preview.1")]
  [InlineData("1.2.2601.1514-preview.9", false, false, true, "1.2.2610.2714-preview.10")]
  [InlineData("1.2.3", false, false, true, "1.2.2610.2714-preview.1")]
  [InlineData("1.2.3-preview.0", false, false, true, "1.2.2610.2714-preview.1")]
  [InlineData("1.2.3-preview.9", true, false, false, "1.3.2610.2714")]
  [InlineData("1.2.3-preview.9", false, true, false, "2.0.2610.2714")]
  public void PreparesPreviewAndStableReleases( string current, bool minor, bool major, bool preview, string expected )
    => Assert.Equal(expected, CalendarVersion.Prepare(current, minor, major, preview,
      DateTimeOffset.Parse("2026-10-27T20:00:00Z")));

  [Theory]
  [InlineData("1.2.3-beta.1", false)]
  [InlineData("1.2.3-preview.X", false)]
  [InlineData("1.2.3-preview.01", false)]
  [InlineData("1.2.3-preview.-1", false)]
  [InlineData("1.2.3-preview.2147483648", false)]
  [InlineData("1.2.3-preview.2147483647", true)]
  [InlineData("1.2.9999999999999999999999999-preview.1", false)]
  public void RefusesMalformedVersionsAndOverflow( string current, bool preview )
    => Assert.ThrowsAny<ArgumentException>(() => CalendarVersion.Prepare(current, false, false, preview,
      DateTimeOffset.Parse("2026-10-27T20:00:00Z")));

  [Theory]
  [InlineData("", false)]
  [InlineData("--increment-preview", true)]
  [InlineData("--increment-minor", false)]
  [InlineData("--increment-major", true)]
  public async Task UpdatesAllReleasePropertiesPreservingBindingCommentsAndEncoding( string option, bool utf16 )
  {
    using var project = Fixture();
    const string source = "<Project>\r\n<!-- <Version>99.99.99</Version> -->\r\n<PropertyGroup>\r\n"
      + "  <Version> 1.2.3-preview.9 </Version> <!-- keep -->\r\n"
      + "  <AssemblyVersion>1.0.0.0</AssemblyVersion>\r\n"
      + "  <FileVersion>1.2.3.0</FileVersion>\r\n"
      + "  <InformationalVersion>1.2.3-preview.9+$(SourceRevisionId)</InformationalVersion>\r\n"
      + "</PropertyGroup>\r\n</Project>\r\n";
    var path = project.Write("App.csproj", source);
    Encoding encoding = utf16 ? Encoding.Unicode : new UTF8Encoding(true);
    File.WriteAllText(path, source, encoding);
    var before = File.ReadAllBytes(path);
    string[] command = option.Length == 0 ? ["update-version"] : ["update-version", option];
    var dryRun = await project.RunAsync([.. command, "--project", "App.csproj", "--dry-run"]);
    Assert.True(dryRun.ExitCode == 0, dryRun.StandardError);
    Assert.Equal(before, File.ReadAllBytes(path));
    var beforeWrite = DateTimeOffset.UtcNow;
    var result = await project.RunAsync([.. command, "--project", "App.csproj"]);
    Assert.True(result.ExitCode == 0, result.StandardError);
    var document = XDocument.Load(path);
    var version = document.Descendants("Version").Single().Value.Trim();
    Assert.Contains(version, new[] {
      CalendarVersion.Prepare("1.2.3-preview.9", option == "--increment-minor",
        option == "--increment-major", option == "--increment-preview", beforeWrite),
      CalendarVersion.Prepare("1.2.3-preview.9", option == "--increment-minor",
        option == "--increment-major", option == "--increment-preview", DateTimeOffset.UtcNow)
    });
    var expected = source.Replace("1.2.3-preview.9", version, StringComparison.Ordinal)
      .Replace("<FileVersion>1.2.3.0", "<FileVersion>" + version.Split('-')[0], StringComparison.Ordinal);
    Assert.Equal(encoding.GetPreamble().Concat(encoding.GetBytes(expected)), File.ReadAllBytes(path));
    Assert.Equal("1.0.0.0", document.Descendants("AssemblyVersion").Single().Value);
  }

  [Fact]
  public async Task KeepsVersionExpressionAndCustomGitSuffix()
  {
    using var project = Fixture();
    var path = project.Write("App.csproj", "<Project><PropertyGroup><Version>1.2.3-preview.1</Version>"
      + "<InformationalVersion>$(Version)+$(SourceRevisionId)</InformationalVersion></PropertyGroup></Project>");
    var result = await project.RunAsync("update-version", "--project", "App.csproj", "--increment-minor");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Equal("$(Version)+$(SourceRevisionId)", XDocument.Load(path).Descendants("InformationalVersion").Single().Value);
    Assert.Empty(XDocument.Load(path).Descendants("AssemblyVersion"));
    Assert.Empty(XDocument.Load(path).Descendants("FileVersion"));
  }

  [Theory]
  [InlineData("<FileVersion>$(Version)</FileVersion>")]
  [InlineData("<FileVersion>1.0.0.0</FileVersion><FileVersion>2.0.0.0</FileVersion>")]
  [InlineData("<FileVersion Condition=\"'x'=='x'\">1.0.0.0</FileVersion>")]
  [InlineData("<InformationalVersion>inherited-$(Other)</InformationalVersion>")]
  [InlineData("<InformationalVersion>1.2.3-preview.X</InformationalVersion>")]
  public async Task RefusesUnsafePropertyDefinitionsWithoutPartialWrites( string property )
  {
    using var project = Fixture();
    var path = project.Write("App.csproj", "<Project><PropertyGroup><Version>1.2.3-preview.1</Version>"
      + property + "</PropertyGroup></Project>");
    var before = File.ReadAllBytes(path);
    var result = await project.RunAsync("update-version", "--project", "App.csproj");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Equal(before, File.ReadAllBytes(path));
  }

  [Fact]
  public async Task RefusesPreviewCombinedWithAnotherIncrement()
  {
    using var project = Fixture();
    var path = project.Write("App.csproj", "<Project><PropertyGroup><Version>1.2.3-preview.1</Version></PropertyGroup></Project>");
    var before = File.ReadAllBytes(path);
    var result = await project.RunAsync("update-version", "--project", "App.csproj", "--increment-preview", "--increment-major");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Equal(before, File.ReadAllBytes(path));
  }

  private static TestProject Fixture()
  {
    var project = new TestProject();
    foreach (var relative in new[] { "dotnet/update-version.cs", "dotnet/_support/CalendarVersion.cs" }) {
      using var stream = typeof(PreviewVersionTests).Assembly.GetManifestResourceStream("Shared/" + relative)!;
      using var reader = new StreamReader(stream);
      project.Write(".tasks/_/" + relative, reader.ReadToEnd());
    }
    return project;
  }
}

using System.IO.Compression;

namespace DoTask.Tests;

public sealed class SharedDotNetTaskTests
{
  [Fact]
  public async Task PackCreatesConfiguredNuGetPackageInRequestedDirectory()
  {
    using var project = new TestProject();
    CopyShared(project, "dotnet/pack.cs", ".tasks");
    project.Write(".dotasks.yaml", "settings:\n  project: source library/Library.csproj\n");
    project.WriteBuildProperties();
    project.Write("source library/Library.csproj", """
      <Project Sdk="Microsoft.NET.Sdk">
        <PropertyGroup>
          <TargetFramework>net10.0</TargetFramework>
          <PackageId>DoTask.PackFixture</PackageId>
          <Version>1.2.3</Version>
        </PropertyGroup>
      </Project>
      """);
    project.Write("source library/Library.cs", "public class Library { }");
    var result = await project.RunAsync("pack", "-c", "Debug", "-o", "packages with spaces");
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    var package = Assert.Single(Directory.GetFiles(Path.Combine(project.Root, "packages with spaces"), "*.nupkg"));
    Assert.Equal("DoTask.PackFixture.1.2.3.nupkg", Path.GetFileName(package));
    using var archive = ZipFile.OpenRead(package);
    Assert.NotNull(archive.GetEntry("lib/net10.0/Library.dll"));
    Assert.False(Directory.Exists(Path.Combine(project.Root, "artifacts/packages")));
    project.Write(".dotasks.yaml", "settings:\n  project: missing.csproj\n");
    var missing = await project.RunAsync("pack");
    Assert.Equal(1, missing.ExitCode);
    Assert.Contains("Project file does not exist:", missing.StandardError);
  }

  [Fact]
  public async Task FormatChecksAndFormatsAllSelectedTaskGroupsAndRunsOptionalFixeolOnlyWhenWriting()
  {
    using var project = new TestProject();
    const string tasks = "automation/tasks";
    CopyShared(project, "dotnet/format.cs", tasks);
    project.Write(".dotasks.yaml", "settings:\n  solution: Sample.slnx\n");
    project.WriteBuildProperties();
    project.Write(".editorconfig", """
      root = true
      [*.cs]
      indent_style = space
      indent_size = 2
      csharp_new_line_before_open_brace = all
      """);
    project.Write("Sample.slnx", "<Solution><Project Path=\"src/Sample.csproj\" /></Solution>");
    project.Write("src/Sample.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>");
    project.Write("src/Marker.cs", "namespace Sample;\n\npublic class Marker\n{\n}\n");
    var untidy = project.Write(tasks + "/custom/untidy.cs",
      "public static class Other\n{\npublic static void Main()\n{\nSystem.Console.WriteLine(\"done\");\n}\n}\n");
    var original = File.ReadAllBytes(untidy);
    var probe = await project.BuildProcessProbeAsync("""
      if (args[0] == "fail") { Environment.Exit(17); }
      File.WriteAllText("normalized", "yes");
      """);
    var fixeol = project.Write(tasks + "/_/text/fixeol.task", $$"""
      steps:
        - exec: {executable: dotnet, arguments: [{{System.Text.Json.JsonSerializer.Serialize(probe)}}, write]}
      """);
    var marker = Path.Combine(project.Root, "normalized");
    string[] command = ["--use-dir", tasks, "dotnet/format"];
    var check = await project.RunAsync([.. command, "--verify"]);
    Assert.NotEqual(0, check.ExitCode);
    Assert.True((check.StandardOutput + check.StandardError).Contains("untidy.cs", StringComparison.Ordinal),
      check.StandardOutput + check.StandardError);
    Assert.Equal(original, File.ReadAllBytes(untidy));
    Assert.False(File.Exists(marker));

    var formatted = await project.RunAsync(command);
    Assert.True(formatted.ExitCode == 0, formatted.StandardOutput + formatted.StandardError);
    Assert.NotEqual(original, File.ReadAllBytes(untidy));
    Assert.Equal("yes", File.ReadAllText(marker));
    File.Delete(marker);
    check = await project.RunAsync([.. command, "--verify"]);
    Assert.True(check.ExitCode == 0, check.StandardOutput + check.StandardError);
    Assert.False(File.Exists(marker));

    File.WriteAllText(fixeol, File.ReadAllText(fixeol).Replace(", write]", ", fail]", StringComparison.Ordinal));
    Assert.Equal(17, (await project.RunAsync(command)).ExitCode);
  }


  [Theory]
  [InlineData("", false)]
  [InlineData("--increment", true)]
  [InlineData("--increment-minor", false)]
  [InlineData("--increment-major", true)]
  public async Task UpdateVersionPreservesBytesAndNeverBuilds( string option, bool utf16 )
  {
    using var project = new TestProject();
    CopyShared(project, "dotnet/update-version.cs", ".tasks");
    project.Write(".dotasks.yaml", "settings:\n  project: source path/App.csproj\n");
    const string source = "<Project>\r\n  <!-- café -->\r\n  <PropertyGroup><Version>1.2.3</Version></PropertyGroup>\r\n</Project>\r\n";
    var manifest = project.Write("source path/App.csproj", source);
    System.Text.Encoding encoding = utf16 ? System.Text.Encoding.Unicode : new System.Text.UTF8Encoding(true);
    File.WriteAllText(manifest, source, encoding);
    var original = File.ReadAllBytes(manifest);
    var lockFile = project.Write("source path/packages.lock.json", "{ \"version\": 1 }");
    var lockBytes = File.ReadAllBytes(lockFile);
    string[] command = option.Length == 0 ? ["dotnet/update-version"] : ["dotnet/update-version", option];
    var beforePreview = DateTimeOffset.UtcNow;
    var preview = await project.RunAsync([.. command, "--dry-run"]);
    Assert.True(preview.ExitCode == 0, preview.StandardError);
    Assert.Equal(original, File.ReadAllBytes(manifest));
    var previewExpected = DoTask.Shared.DotNet.CalendarVersion.Create(1, 2,
      option == "--increment-minor", option == "--increment-major", beforePreview);
    var previewAfter = DoTask.Shared.DotNet.CalendarVersion.Create(1, 2,
      option == "--increment-minor", option == "--increment-major", DateTimeOffset.UtcNow);
    Assert.True(preview.StandardOutput.Contains("1.2.3 -> " + previewExpected, StringComparison.Ordinal)
      || preview.StandardOutput.Contains("1.2.3 -> " + previewAfter, StringComparison.Ordinal), preview.StandardOutput);
    var beforeWrite = DateTimeOffset.UtcNow;
    var result = await project.RunAsync(command);
    Assert.True(result.ExitCode == 0, result.StandardError);
    var expected = System.Xml.Linq.XDocument.Load(manifest).Descendants("Version").Single().Value;
    Assert.Contains(expected, new[] {
      DoTask.Shared.DotNet.CalendarVersion.Create(1, 2, option == "--increment-minor", option == "--increment-major", beforeWrite),
      DoTask.Shared.DotNet.CalendarVersion.Create(1, 2, option == "--increment-minor", option == "--increment-major", DateTimeOffset.UtcNow)
    });
    var expectedBytes = encoding.GetPreamble().Concat(encoding.GetBytes(source.Replace("1.2.3", expected, StringComparison.Ordinal))).ToArray();
    Assert.Equal(expectedBytes, File.ReadAllBytes(manifest));
    Assert.Equal(lockBytes, File.ReadAllBytes(lockFile));
    Assert.False(Directory.Exists(Path.Combine(project.Root, "bin")));
    Assert.False(Directory.Exists(Path.Combine(project.Root, "obj")));
    var stableBytes = File.ReadAllBytes(manifest);
    var refresh = await project.RunAsync("dotnet/update-version");
    Assert.True(refresh.ExitCode == 0, refresh.StandardError);
    // For the same Utah hour, refreshing a four-component version is a byte no-op.
    var refreshed = System.Xml.Linq.XDocument.Load(manifest).Descendants("Version").Single().Value;
    if (refreshed == expected)
      Assert.Equal(stableBytes, File.ReadAllBytes(manifest));
  }

  [Theory]
  [InlineData("<Version Condition=\"'x'=='x'\">1.2.3</Version>")]
  [InlineData("<Version>1.2.3</Version><Version>1.2.4</Version>")]
  [InlineData("<VersionPrefix>1.2.3</VersionPrefix>")]
  [InlineData("<Version>$(InheritedVersion)</Version>")]
  [InlineData("<Version>1.2.65535</Version>")]
  public async Task UpdateVersionRejectsAmbiguousInheritedOrOverflowWithoutWriting( string definition )
  {
    using var project = new TestProject();
    CopyShared(project, "dotnet/update-version.cs", ".tasks");
    var path = project.Write("App.csproj", "<Project><PropertyGroup>" + definition + "</PropertyGroup></Project>");
    var before = File.ReadAllBytes(path);
    var result = await project.RunAsync("dotnet/update-version", "--project", "App.csproj");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Equal(before, File.ReadAllBytes(path));
  }

  [Fact]
  public async Task UpdateVersionRejectsConflictingIncrementFlags()
  {
    using var project = new TestProject();
    CopyShared(project, "dotnet/update-version.cs", ".tasks");
    var path = project.Write("App.csproj", "<Project><PropertyGroup><Version>1.2.3</Version></PropertyGroup></Project>");
    var before = File.ReadAllBytes(path);
    var result = await project.RunAsync("dotnet/update-version", "--project", "App.csproj", "--increment", "--increment-major");
    Assert.NotEqual(0, result.ExitCode);
    Assert.Equal(before, File.ReadAllBytes(path));
  }

  private static void CopyShared( TestProject project, string relative, string directory )
  {
    using var stream = typeof(SharedDotNetTaskTests).Assembly.GetManifestResourceStream("Shared/" + relative)!;
    using var reader = new StreamReader(stream);
    project.Write(directory + "/_/" + relative, reader.ReadToEnd());
    if (relative == "dotnet/update-version.cs")
      CopyShared(project, "dotnet/_support/CalendarVersion.cs", directory);
  }
}

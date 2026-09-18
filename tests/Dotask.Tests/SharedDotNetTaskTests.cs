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
    project.Write("Directory.Build.props", "<Project />");
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
    project.Write("Directory.Build.props", "<Project />");
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
    var fixeol = project.Write(tasks + "/_/text/fixeol.cs", """
      using DoTask;
      public static class Target
      {
        public static void Main()
        {
          BuildContext.Current.Files.WriteText("normalized", "yes");
        }
      }
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

    File.WriteAllText(fixeol, "public static class Target { public static void Main() { System.Environment.Exit(17); } }\n");
    Assert.Equal(17, (await project.RunAsync(command)).ExitCode);
  }

  private static void CopyShared( TestProject project, string relative, string directory )
  {
    using var stream = typeof(SharedDotNetTaskTests).Assembly.GetManifestResourceStream("Shared/" + relative)!;
    using var reader = new StreamReader(stream);
    project.Write(directory + "/_/" + relative, reader.ReadToEnd());
  }
}

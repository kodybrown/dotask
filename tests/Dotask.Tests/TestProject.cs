using System.Text.Json;
using System.Xml.Linq;
using DoTask.Runtime;

namespace DoTask.Tests;

public sealed class TestProject : IDisposable
{
  public string Root { get; } = Path.Combine(Path.GetTempPath(), "dotask-tests", "project with spaces " + Guid.NewGuid().ToString("N"));
  public string Tasks => Path.Combine(Root, ".tasks");
  public string OutputRoot => Path.Combine(OperatingSystem.IsWindows() ? @"C:\tmp\_dotnet" : "/tmp/_dotnet",
    "dotask-tests", Path.GetFileName(Root));

  public TestProject() => Directory.CreateDirectory(Tasks);

  public string Write( string relative, string text )
  {
    var file = Path.GetFullPath(Path.Combine(Root, relative));
    Directory.CreateDirectory(Path.GetDirectoryName(file)!);
    File.WriteAllText(file, text);
    return file;
  }

  public void WriteBuildProperties( string relative = "Directory.Build.props", string properties = "" )
  {
    // Synthetic SDK projects must preserve the same user output policy as the
    // repository. On machines without that policy, keep fixture outputs external.
    var userProps = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Directory.Build.props");
    var document = new XElement("Project");
    if (File.Exists(userProps)) {
      document.Add(new XElement("Import", new XAttribute("Project", userProps)));
    } else {
      document.Add(new XElement("PropertyGroup",
        new XElement("BaseOutputPath", Path.Combine(OutputRoot, "$(MSBuildProjectName)", "bin") + Path.DirectorySeparatorChar),
        new XElement("BaseIntermediateOutputPath", Path.Combine(OutputRoot, "$(MSBuildProjectName)", "obj") + Path.DirectorySeparatorChar),
        new XElement("PublishDir", Path.Combine(OutputRoot, "$(MSBuildProjectName)", "publish") + Path.DirectorySeparatorChar)));
    }

    document.Add(new XElement("PropertyGroup", new XElement("FixtureOutputRoot", OutputRoot)));
    document.Add(XElement.Parse("<PropertyGroup>" + properties + "</PropertyGroup>"));
    Write(relative, document.ToString());
  }

  public string Target( string name, string body = "", string metadata = "", bool async = false )
    => Write($".tasks/{name}.cs", $$"""
      {{metadata}}
      using DoTask;
      public static class Target
      {
        public static {{(async ? "async Task" : "void")}} Main()
        {
          {{body}}
        }
      }
      """);

  public BuildContext Context() => new(new ExecutionContextData {
    RootDirectory = Root,
    InvocationDirectory = Root,
    TaskDirectory = Tasks,
    TargetFile = Path.Combine(Tasks, "test.cs"),
    TargetName = "test",
    CliExecutable = RustArtifacts.Binary("dotask"),
    CliArguments = [],
    SessionDirectory = Root,
    ProjectSettings = JsonSerializer.SerializeToElement(new { }),
    Settings = JsonSerializer.SerializeToElement(new { }),
    Parameters = JsonSerializer.SerializeToElement(new { }),
    TargetDefaults = JsonSerializer.SerializeToElement(new { })
  });

  public async Task<ProcessResult> RunAsync( params string[] arguments ) => await RunFromAsync(Root, arguments);

  public Task<ProcessResult> RunFromAsync( string directory, params string[] arguments )
    => RustArtifacts.Run(directory, arguments);

  public async Task<string> BuildProcessProbeAsync( string source )
  {
    // ProcessRunner belongs to the maintained C# helper. Build its child probe
    // directly with the SDK rather than depending on a CLI compiler API.
    WriteBuildProperties("probe/Directory.Build.props", "<TargetFramework>net10.0</TargetFramework>");
    var project = Write("probe/Probe.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><OutputType>Exe</OutputType><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>");
    Write("probe/Program.cs", source);
    var result = await Context().RunAsync(new ProcessDefinition {
      Executable = DotnetHost.Find(),
      Arguments = ["build", project, "-c", "Release", "--nologo", "-t:Build", "-getProperty:TargetPath"],
      CaptureOutput = true
    });
    var path = result.StandardOutput.Trim();
    Assert.True(File.Exists(path), result.StandardOutput + result.StandardError);
    return path;
  }

  public void Dispose()
  {
    if (Directory.Exists(Root)) {
      // Git marks object files read-only on Windows. Clear that bit only on
      // fixture-owned files, without following links into external directories.
      foreach (var file in Directory.EnumerateFiles(Root, "*", new EnumerationOptions {
        RecurseSubdirectories = true,
        AttributesToSkip = FileAttributes.ReparsePoint
      })) {
        var attributes = File.GetAttributes(file);
        if ((attributes & FileAttributes.ReadOnly) != 0) {
          File.SetAttributes(file, attributes & ~FileAttributes.ReadOnly);
        }
      }
      Directory.Delete(Root, recursive: true);
    }
  }
}

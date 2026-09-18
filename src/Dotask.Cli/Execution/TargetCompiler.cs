using System.Security.Cryptography;
using System.Text;
using System.Xml.Linq;
using DoTask.Cli.Metadata;

namespace DoTask.Cli.Execution;

public sealed record CompilationResult(bool Success, string? AssemblyPath, string Diagnostics)
{
  public string Summary => Diagnostics.Split('\n', StringSplitOptions.RemoveEmptyEntries)
    .FirstOrDefault(line => line.Contains("error", StringComparison.OrdinalIgnoreCase))?.Trim()
    ?? Diagnostics.Trim();
}

public sealed class TargetCompiler
{
  private readonly string _dotnet;
  private readonly string _cacheRoot;

  public TargetCompiler(string? dotnet = null, string? cacheRoot = null)
  {
    _dotnet = dotnet ?? DotnetHost.Find();
    _cacheRoot = cacheRoot ?? Path.Combine(Path.GetTempPath(), "_dotnet", "dotask", Hash(Environment.UserName));
    CreatePrivateDirectory(_cacheRoot);
  }

  public async Task<CompilationResult> CompileAsync(TargetDefinition target, CancellationToken cancellationToken,
    string? snapshotDirectory = null)
  {
    var cache = Path.Combine(_cacheRoot, Hash(Path.GetFullPath(target.FilePath)));
    Directory.CreateDirectory(cache);
    await using var lease = await AcquireLockAsync(Path.Combine(cache, "build.lock"), cancellationToken);
    var props = Path.Combine(cache, "dotask.props");
    var targets = Path.Combine(cache, "dotask.targets");
    var outputRecord = Path.Combine(cache, "target-path.txt");
    var bootstrap = Path.Combine(cache, "dotask-bootstrap.cs");
    var entry = Escape(target.FilePath + ".csproj");
    var rootCondition = $"'$(MSBuildProjectFullPath)' == '{entry}'";
    var otherCondition = $"'$(MSBuildProjectFullPath)' != '{entry}'";
    var propsXml = new XElement("Project",
      new XElement("PropertyGroup", new XAttribute("Condition", otherCondition),
        new XElement("_DotaskOriginalProps", "$([MSBuild]::GetPathOfFileAbove('Directory.Build.props', '$(MSBuildProjectDirectory)'))")),
      new XElement("Import", new XAttribute("Condition", otherCondition + " and '$(_DotaskOriginalProps)' != ''"),
        new XAttribute("Project", "$(_DotaskOriginalProps)")),
      new XElement("PropertyGroup", new XAttribute("Condition", rootCondition),
        new XElement("BaseOutputPath", Escape(Path.Combine(cache, "bin") + Path.DirectorySeparatorChar)),
        new XElement("BaseIntermediateOutputPath", Escape(Path.Combine(cache, "obj") + Path.DirectorySeparatorChar)),
        new XElement("MSBuildProjectExtensionsPath", Escape(Path.Combine(cache, "obj") + Path.DirectorySeparatorChar)),
        new XElement("UseAppHost", "false"), new XElement("PublishAot", "false"),
        new XElement("ManagePackageVersionsCentrally", "false")));
    var targetsXml = new XElement("Project",
      new XElement("PropertyGroup", new XAttribute("Condition", otherCondition),
        new XElement("_DotaskOriginalTargets", "$([MSBuild]::GetPathOfFileAbove('Directory.Build.targets', '$(MSBuildProjectDirectory)'))")),
      new XElement("Import", new XAttribute("Condition", otherCondition + " and '$(_DotaskOriginalTargets)' != ''"),
        new XAttribute("Project", "$(_DotaskOriginalTargets)")),
      new XElement("ItemGroup", new XAttribute("Condition", rootCondition),
        new XElement("Reference", new XAttribute("Include", "Dotask.Library"),
          new XElement("HintPath", Escape(typeof(BuildContext).Assembly.Location)), new XElement("Private", "true")),
        new XElement("Compile", new XAttribute("Include", Escape(bootstrap)))),
      new XElement("Target", new XAttribute("Name", "DotaskRecordOutput"), new XAttribute("AfterTargets", "Build"),
        new XAttribute("Condition", rootCondition),
        new XElement("WriteLinesToFile", new XAttribute("File", Escape(outputRecord)),
          new XAttribute("Lines", "$(TargetPath)"), new XAttribute("Overwrite", "true"))));
    WriteIfChanged(props, propsXml.ToString());
    WriteIfChanged(targets, targetsXml.ToString());
    WriteIfChanged(bootstrap, """
      internal static class __DotaskBootstrap
      {
        [System.Runtime.CompilerServices.ModuleInitializer]
        internal static void Initialize() => DoTask.Runtime.TargetRuntime.Initialize();
      }
      """);
    File.Delete(outputRecord);
    // Run from the source directory so its SDK and NuGet selection are respected.
    // Only the task's implicit MSBuild props/targets are replaced; explicitly
    // referenced projects retain their normal build configuration.
    var build = await ProcessRunner.RunAsync(new ProcessDefinition
    {
      Executable = _dotnet,
      Arguments = ["build", target.FilePath, "-c", "Release", "--nologo", "-v:quiet",
        $"-p:DirectoryBuildPropsPath={props}", $"-p:DirectoryBuildTargetsPath={targets}"],
      WorkingDirectory = Path.GetDirectoryName(target.FilePath),
      CaptureOutput = true,
      ThrowOnError = false,
      Environment = new Dictionary<string, string?> { [Runtime.ExecutionContextData.EnvironmentVariable] = null }
    }, cancellationToken);
    var diagnostics = (build.StandardOutput + "\n" + build.StandardError).Trim();
    if (build.ExitCode != 0 || !File.Exists(outputRecord))
    {
      return new(false, null, string.IsNullOrWhiteSpace(diagnostics) ? "Compilation failed without diagnostics." : diagnostics);
    }
    var assembly = File.ReadAllText(outputRecord).Trim();
    if (!File.Exists(assembly))
    {
      return new(false, null, $"Compiler output is missing: {assembly}");
    }
    if (snapshotDirectory is not null)
    {
      CopyOutput(Path.GetDirectoryName(assembly)!, snapshotDirectory);
      assembly = Path.Combine(snapshotDirectory, Path.GetFileName(assembly));
    }
    return new(true, assembly, diagnostics);
  }

  private static async Task<FileStream> AcquireLockAsync(string path, CancellationToken cancellationToken)
  {
    while (true)
    {
      cancellationToken.ThrowIfCancellationRequested();
      try
      {
        return new FileStream(path, FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
      }
      catch (IOException)
      {
        await Task.Delay(50, cancellationToken);
      }
    }
  }

  private static void CopyOutput(string source, string destination)
  {
    Directory.CreateDirectory(destination);
    foreach (var file in Directory.EnumerateFiles(source))
    {
      File.Copy(file, Path.Combine(destination, Path.GetFileName(file)), overwrite: true);
    }
    foreach (var child in Directory.EnumerateDirectories(source))
    {
      CopyOutput(child, Path.Combine(destination, Path.GetFileName(child)));
    }
  }

  internal static void CreatePrivateDirectory(string path)
  {
    if (OperatingSystem.IsWindows())
    {
      Directory.CreateDirectory(path);
    }
    else
    {
      Directory.CreateDirectory(path, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
    }
  }

  private static void WriteIfChanged(string path, string text)
  {
    if (!File.Exists(path) || File.ReadAllText(path) != text)
    {
      File.WriteAllText(path, text);
    }
  }

  private static string Hash(string value) => Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(value)))[..24];
  private static string Escape(string value) => value.Replace("%", "%25").Replace("$", "%24")
    .Replace("@", "%40").Replace("'", "%27").Replace(";", "%3B").Replace("(", "%28").Replace(")", "%29");
}

#:include _support/RustBuild.cs
using DoTask;

/// <summary>Build the Rust CLI preview, or run its tests and formatting checks.</summary>
/// <option name="verify" type="bool" default="false">Run Rust tests, formatting, and clippy.</option>
/// <requires tool="cargo" />
/// <requires tool="dotnet" />
/// <remarks>Stage the C# support host beside the preview. The C# CLI remains the repository runner.</remarks>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var output = await RustBuild.TargetDirectory(project);
    var manifest = project.Path("Cargo.toml");
    // Both workspace members honor Cargo's configured external target directory.
    // Static CRT keeps Windows delivery independent of VC runtime DLLs.
    Dictionary<string, string?> environment = new();
    if (project.IsWindows)
      environment["RUSTFLAGS"] = "-C target-feature=+crt-static";
    var hostProject = project.Path("src/Dotask.CSharpHost/Dotask.CSharpHost.csproj");
    await project.RunAsync("dotnet", ["publish", hostProject, "-c", "Release", "--self-contained=false"]);
    var properties = await project.RunAsync(new ProcessDefinition {
      Executable = "dotnet",
      Arguments = ["msbuild", hostProject, "-p:Configuration=Release", "-getProperty:PublishDir"],
      CaptureOutput = true
    });
    var published = Path.GetFullPath(properties.StandardOutput.Trim(), Path.GetDirectoryName(hostProject)!);
    var support = Path.Combine(output, "release", "csharp");
    // Stage physical copies from the evaluated output policy, never run from
    // live .NET build output and never depend on the installed C# CLI.
    foreach (var source in Directory.EnumerateFiles(published, "*", SearchOption.AllDirectories)) {
      var destination = Path.Combine(support, Path.GetRelativePath(published, source));
      Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
      File.Copy(source, destination, overwrite: true);
    }
    async Task Run( params string[] arguments ) => await project.RunAsync(new ProcessDefinition {
      Executable = "cargo",
      Arguments = arguments,
      Environment = environment
    });
    if (project.Parameters.Get<bool>("verify")) {
      await Run("fmt", "--manifest-path", manifest, "--package", "dotask-cli", "--", "--check");
      await Run("test", "--manifest-path", manifest, "--package", "dotask-cli", "--locked", "--release");
      await Run("clippy", "--manifest-path", manifest, "--package", "dotask-cli", "--locked", "--all-targets", "--", "-D", "warnings");
    } else {
      await Run("build", "--manifest-path", manifest, "--package", "dotask-cli", "--locked", "--release");
    }
    Console.WriteLine(Path.Combine(output, "release", project.IsWindows ? "dotask.exe" : "dotask"));
  }
}

#:include _support/RustBuild.cs
using DoTask;

/// <summary>Build the host Rust installer, or run its tests and formatting checks.</summary>
/// <option name="verify" type="bool" default="false">Run Rust tests and formatting checks.</option>
/// <requires tool="cargo" />
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var output = await RustBuild.TargetDirectory(project);
    var manifest = project.Path("src/dotask-installer/Cargo.toml");
    // Honor Cargo's external output policy. Static CRT removes the VC runtime DLL
    // dependency from the standalone Windows delivery executable.
    Dictionary<string, string?> environment = new();
    if (project.IsWindows)
      environment["RUSTFLAGS"] = "-C target-feature=+crt-static";
    async Task Run( params string[] arguments ) => await project.RunAsync(new ProcessDefinition {
      Executable = "cargo",
      Arguments = arguments,
      Environment = environment
    });
    if (project.Parameters.Get<bool>("verify")) {
      await Run("fmt", "--manifest-path", manifest, "--", "--check");
      await Run("test", "--manifest-path", manifest, "--locked", "--release");
      await Run("clippy", "--manifest-path", manifest, "--locked", "--all-targets", "--", "-D", "warnings");
    } else {
      await Run("build", "--manifest-path", manifest, "--locked", "--release");
    }
    Console.WriteLine(Path.Combine(output, "release", project.IsWindows ? "dotask-installer.exe" : "dotask-installer"));
  }
}

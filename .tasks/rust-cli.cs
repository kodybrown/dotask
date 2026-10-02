using DoTask;

/// <summary>Build the Rust CLI preview, or run its tests and formatting checks.</summary>
/// <option name="verify" type="bool" default="false">Run Rust tests, formatting, and clippy.</option>
/// <requires tool="cargo" />
/// <remarks>The preview does not execute tasks. The C# CLI remains the repository runner.</remarks>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var output = project.IsWindows ? "C:/tmp/_dotnet/dotask-rust" : "/tmp/_dotnet/dotask-rust";
    var manifest = project.Path("Cargo.toml");
    // Match the installer build so both workspace members share external
    // artifacts and Windows delivery does not require VC runtime DLLs.
    Dictionary<string, string?> environment = new() { ["CARGO_TARGET_DIR"] = output };
    if (project.IsWindows)
      environment["RUSTFLAGS"] = "-C target-feature=+crt-static";
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

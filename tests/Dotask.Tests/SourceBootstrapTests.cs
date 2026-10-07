namespace DoTask.Tests;

public sealed class SourceBootstrapTests
{
  [Fact]
  public async Task CSharpOverridePreparesSdkFromSourceCheckoutAndPreservesNestedConsumerContextAndFailures()
  {
    using var source = new TestProject();
    using var consumer = new TestProject();
    var staged = RustArtifacts.Binary("dotask");
    var runner = source.Write("runner/placeholder", "");
    runner = Path.Combine(Path.GetDirectoryName(runner)!, Path.GetFileName(staged));
    File.Copy(staged, runner);
    if (!OperatingSystem.IsWindows()) {
      File.SetUnixFileMode(runner, File.GetUnixFileMode(staged));
    }
    var sdk = Path.Combine(Path.GetDirectoryName(runner)!, "sdk");
    source.Write(".dotasks.yaml", "settings: { helper: '" + Path.Combine(Path.GetDirectoryName(staged)!, "sdk/dotnet/Dotask.dotnet.dll").Replace('\\', '/') + "' }");
    const string metadata = "// dotask: 1\n// options: [{name: stage-dir, type: path, required: true}]\n// end-dotask\n";
    source.Write(".tasks/build-sdks.rs", metadata + """
      use dotask_sdk::{BuildContext, Result};
      fn main() { dotask_sdk::run(task); }
      fn task(p: &BuildContext) -> Result<()> {
        let destination = p.path(p.string("stage-dir")?).join("dotnet");
        std::fs::create_dir_all(&destination)?;
        std::fs::copy(p.setting_path("helper")?, destination.join("Dotask.dotnet.dll"))?;
        std::fs::write(p.path("prepared"), "source")?;
        Ok(())
      }
      """);
    consumer.Write(".dotasks.yaml", "settings: {message: consumer}\n");
    // The consumer deliberately uses the preparation task's name. Source
    // preparation must not mistake these different project identities for a cycle.
    consumer.Target("build-sdks", "await BuildContext.Current.ExecTargetAsync(\"child\");", async: true);
    consumer.Target("child", "Console.WriteLine(BuildContext.Current.Config.Get<string>(\"message\")); File.WriteAllText(\"executed\", \"consumer\");");
    using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(90));
    async Task<ProcessResult> Run() => await ProcessRunner.RunAsync(new ProcessDefinition {
      Executable = runner,
      Arguments = ["--use-dir", consumer.Tasks, "build-sdks"],
      WorkingDirectory = source.Root,
      Environment = new Dictionary<string, string?> {
        ["DOTASK_SOURCE_ROOT"] = source.Root,
        ["DOTASK_SOURCE_SDK_STAGE"] = sdk,
        ["DOTASK_SOURCE_RUST_SDK"] = Path.Combine(Path.GetDirectoryName(staged)!, "sdk/rust")
      },
      CaptureOutput = true,
      ThrowOnError = false
    }, timeout.Token);
    var result = await Run();
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Contains("consumer", result.StandardOutput);
    Assert.Equal("source", File.ReadAllText(Path.Combine(source.Root, "prepared")));
    Assert.Equal("consumer", File.ReadAllText(Path.Combine(consumer.Root, "executed")));
    Assert.False(File.Exists(Path.Combine(consumer.Root, "prepared")));
    File.Delete(Path.Combine(sdk, "dotnet/Dotask.dotnet.dll"));
    File.Delete(Path.Combine(consumer.Root, "executed"));
    source.Write(".tasks/build-sdks.rs", metadata + "fn main() { std::process::exit(23); }");
    Assert.Equal(23, (await Run()).ExitCode);
    Assert.False(File.Exists(Path.Combine(consumer.Root, "executed")));
  }
}

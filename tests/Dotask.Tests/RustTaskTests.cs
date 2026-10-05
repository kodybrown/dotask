using System.Text.Json;

namespace DoTask.Tests;

public sealed class RustTaskTests
{
  [Fact]
  public async Task RustTasksBindTypedDefaultsAndPreserveContextArgumentsAndSupportSources()
  {
    using var project = new TestProject();
    project.Write(".dotasks.yaml", "settings: { nested: { message: '日本語 café' } }\ntargets: { run: { defaults: { label: 'two words' } } }\n");
    project.Write(".tasks/_support/message.rs", "pub fn message() -> &'static str { \"support\" }\n");
    project.Write(".tasks/run.rs", """
      // dotask: 1
      // description: A Rust task.
      // options:
      //   - { name: label, alias: l, default: fallback }
      //   - { name: count, type: int, default: 3 }
      //   - { name: flag, type: bool, default: true }
      //   - { name: output, type: path, default: result.json }
      // requires:
      //   - { kind: file, value: _support/message.rs }
      // end-dotask
      #[path = "_support/message.rs"] mod helper;
      use dotask_sdk::{BuildContext, Result, json, serde_json};
      fn main() { dotask_sdk::run(task); }
      fn task(p: &BuildContext) -> Result<()> {
        std::fs::write(p.string("output")?, serde_json::to_vec(&json!({
          "root":p.root(), "invocation":p.invocation(), "name":p.task_name(),
          "parameters":p.parameters(), "message":p.setting("nested.message")?, "support":helper::message()
        }))?)?;
        println!("Rust stdout 日本語"); eprintln!("Rust stderr café"); Ok(())
      }
      """);
    var result = await RustArtifacts.Run(project.Root, ["run", "--label", "literal $() ` ! spaces", "--count=7"]);
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.Contains("Rust stdout 日本語", result.StandardOutput);
    Assert.Contains("Rust stderr café", result.StandardError);
    using var json = JsonDocument.Parse(File.ReadAllText(Path.Combine(project.Root, "result.json")));
    Assert.Equal(project.Root, json.RootElement.GetProperty("root").GetString());
    Assert.Equal(project.Root, json.RootElement.GetProperty("invocation").GetString());
    Assert.Equal("run", json.RootElement.GetProperty("name").GetString());
    Assert.Equal("literal $() ` ! spaces", json.RootElement.GetProperty("parameters").GetProperty("label").GetString());
    Assert.Equal(7, json.RootElement.GetProperty("parameters").GetProperty("count").GetInt32());
    Assert.True(json.RootElement.GetProperty("parameters").GetProperty("flag").GetBoolean());
    Assert.Equal("日本語 café", json.RootElement.GetProperty("message").GetString());
    Assert.Equal("support", json.RootElement.GetProperty("support").GetString());
    project.Write(".tasks/_support/message.rs", "pub fn message() -> &'static str { \"changed\" }\n");
    Assert.Equal(0, (await RustArtifacts.Run(project.Root, ["run"])).ExitCode);
    Assert.Contains("changed", File.ReadAllText(Path.Combine(project.Root, "result.json")));
    Assert.False(Directory.Exists(Path.Combine(project.Root, "target")));
    Assert.False(File.Exists(Path.Combine(project.Tasks, "Cargo.toml")));
  }

  [Fact]
  public async Task RustAndCSharpCallsShareSnapshotsAndDistinguishAbsenceFailuresAndCycles()
  {
    using var project = new TestProject();
    project.Write(".dotasks.yaml", "settings: { marker: original }\n");
    project.Write(".tasks/start.rs", """
      use dotask_sdk::{BuildContext, Result, json};
      fn main() { dotask_sdk::run(task); }
      fn task(p: &BuildContext) -> Result<()> {
        std::fs::write(p.path(".dotasks.yaml"), "settings: { marker: changed }\n")?;
        p.exec_target("managed", json!({}))?;
        assert!(!p.target_exists("missing")?);
        assert_eq!(p.exec_if_exists("missing", json!({}))?["Exists"], false);
        assert!(p.exec_if_exists("bad", json!({}))?["Error"].is_string());
        assert_eq!(p.exec_if_exists("fail", json!({}))?["ExitCode"], 29);
        assert!(p.exec_if_exists("start", json!({}))?["Error"].as_str().unwrap().contains("cycle"));
        Ok(())
      }
      """);
    project.Target("managed", "await BuildContext.Current.ExecTargetAsync(\"native\", new { label = \"nested words\" });", async: true);
    project.Write(".tasks/native.rs", """
      // dotask: 1
      // options: [{ name: label, required: true }]
      // end-dotask
      use dotask_sdk::{BuildContext, Result};
      fn main() { dotask_sdk::run(task); }
      fn task(p: &BuildContext) -> Result<()> {
        assert_eq!(p.setting("marker")?, "original");
        assert_eq!(p.string("label")?, "nested words");
        std::fs::write(p.path("nested-called"), "yes")?; Ok(())
      }
      """);
    project.Write(".tasks/bad.rs", "not valid Rust;");
    project.Write(".tasks/fail.rs", "fn main() { std::process::exit(29); }");
    var result = await RustArtifacts.Run(project.Root, ["start"]);
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.True(File.Exists(Path.Combine(project.Root, "nested-called")));
    Assert.Equal(29, (await RustArtifacts.Run(project.Root, ["fail"])).ExitCode);
  }

  [Fact]
  public async Task InstallerResultsCrossBothLanguageBoundariesAndRemainInvocationLocal()
  {
    using var project = new TestProject();
    var artifact = project.Write(OperatingSystem.IsWindows() ? "installer.exe" : "installer", "fixture artifact");
    var os = OperatingSystem.IsWindows() ? 0 : OperatingSystem.IsMacOS() ? 2 : 1;
    var architecture = (int)System.Runtime.InteropServices.RuntimeInformation.OSArchitecture;
    var result = JsonSerializer.Serialize(new { FilePath = artifact, Kind = 0, OS = os, Architecture = architecture, DefaultArguments = new[] { "", "two words" } });
    project.Write(".tasks/native-installer.rs", $$"""
      use dotask_sdk::{BuildContext, Result, serde_json};
      fn main() { dotask_sdk::run(task); }
      fn task(p: &BuildContext) -> Result<()> { p.set_installer_result(&serde_json::from_str(r###"{{result}}"###)?) }
      """);
    project.Target("managed", "var p = BuildContext.Current; var artifact = await p.CreateInstallerAsync(\"native-installer\"); await p.SetInstallerResultAsync(artifact);", async: true);
    project.Write(".tasks/start.rs", """
      use dotask_sdk::{BuildContext, Result, json};
      fn main() { dotask_sdk::run(task); }
      fn task(p: &BuildContext) -> Result<()> {
        let artifact = p.create_installer("managed", json!({}))?;
        assert_eq!(artifact["DefaultArguments"], json!(["", "two words"]));
        assert!(p.create_installer("missing-result", json!({})).is_err());
        Ok(())
      }
      """);
    project.Write(".tasks/missing-result.rs", "fn main() {}\n");
    var run = await RustArtifacts.Run(project.Root, ["start"]);
    Assert.True(run.ExitCode == 0, run.StandardOutput + run.StandardError);
    Assert.Empty(Directory.EnumerateFiles(project.Root, "installer-result.json", SearchOption.AllDirectories));
  }

  [Theory]
  [InlineData("options: [{ name: help }]", "reserved")]
  [InlineData("options: [{ name: count, type: int, default: wrong }]", "Invalid int")]
  [InlineData("requires: [{ kind: file, value: ../outside.rs }]", "portable relative path")]
  [InlineData("unknown: true", "unknown field")]
  public async Task InvalidRustMetadataFailsBeforeCompilationOrExecution( string metadata, string diagnostic )
  {
    using var project = new TestProject();
    project.Write(".tasks/run.rs", "// dotask: 1\n// " + metadata + "\n// end-dotask\nfn main() { panic!(\"must not execute\"); }\n");
    var result = await RustArtifacts.Run(project.Root, ["run"]);
    Assert.Equal(1, result.ExitCode);
    Assert.Contains(diagnostic, result.StandardError);
    Assert.DoesNotContain("panicked", result.StandardError);
  }
}

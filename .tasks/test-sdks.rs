// dotask: 1
// description: Test the maintained C# helper and native CLI integration fixtures.
// options:
//   - { name: configuration, alias: c, choices: [Debug, Release], default: Release }
// requires: [{ kind: tool, value: dotnet }, { kind: setting, value: solution }]
// end-dotask
use dotask_sdk::{json, tempfile, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  // Installer adapters are tested against external tools, not installer source
  // compiled inside dotask's workspace. Independent verification belongs there.
  let tools = tempfile::Builder::new().prefix("dotask-doinstall-tools-").tempdir()?;
  project.exec_target("_/doinstall/build", json!({"stage-dir":tools.path()}))?;
  // Direct SDK invocation keeps repository verification authored in Rust while
  // preserving consumer C# task and helper-library regression coverage.
  project.execute(
    project
      .command("dotnet")
      .env("DOINSTALL_TOOL_DIR", tools.path())
      .arg("test")
      .arg(project.setting_path("solution")?)
      .args([
        "--nologo",
        "-m:1",
        "-nr:false",
        "-c",
        project.string("configuration")?,
      ]),
  )
}

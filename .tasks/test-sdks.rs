// dotask: 1
// description: Test the maintained C# helper and native CLI integration fixtures.
// options:
//   - { name: configuration, alias: c, choices: [Debug, Release], default: Release }
// requires: [{ kind: tool, value: dotnet }, { kind: setting, value: solution }]
// end-dotask
use dotask_sdk::{BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  // Direct SDK invocation keeps repository verification authored in Rust while
  // preserving consumer C# task and helper-library regression coverage.
  project.execute(
    project
      .command("dotnet")
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

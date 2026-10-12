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
  // Verification consumes prebuilt tools just like a consumer project. Source
  // preparation is an explicit development/CI step, not an execution side effect.
  let tools = std::env::var_os("DOINSTALL_TOOL_DIR")
    .map(std::path::PathBuf::from)
    .ok_or_else(|| dotask_sdk::anyhow!("Set DOINSTALL_TOOL_DIR to a prebuilt, host-matching doinstall/doinstall-builder pair. To build a development checkout explicitly, run build.cmd _/doinstall/build --stage-dir PATH (Unix: ./build.sh)."))?;
  let tools = project.path(tools);
  for name in ["doinstall", "doinstall-builder"] {
    let file = tools.join(format!("{name}{}", if cfg!(windows) { ".exe" } else { "" }));
    if !file.is_file() {
      dotask_sdk::bail!("Required prebuilt doinstall tool is missing: {}", file.display());
    }
  }
  // Direct SDK invocation keeps repository verification authored in Rust while
  // preserving consumer C# task and helper-library regression coverage.
  project.execute(
    project
      .command("dotnet")
      .env("DOINSTALL_TOOL_DIR", tools)
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

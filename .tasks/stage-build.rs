// dotask: 1
// description: Stage the inspected dotask executable and complete SDK layout without rebuilding the CLI.
// options:
//   - {name: stage-dir, default: '', completion: directory}
//   - {name: configuration, alias: c, choices: [Debug, Release], default: Release}
// requires: [{kind: file, value: _support/RustBuild.rs}, {kind: task, value: build-sdks}]
// end-dotask
#[path = "_support/RustBuild.rs"]
mod rust_build;
use dotask_sdk::{json, BuildContext, Context, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let binary = rust_build::inspected_binary(project)?;
  let directory = match project.string("stage-dir")? {
    "" => binary.parent().context("Executable has no directory")?.to_path_buf(),
    value => project.path(value),
  };
  rust_build::external_directory(project, &directory)?;
  std::fs::create_dir_all(&directory)?;
  rust_build::external_directory(project, &directory)?;
  let destination = directory.join(binary.file_name().unwrap());
  if destination != binary {
    std::fs::copy(&binary, &destination)?;
  }
  project.exec_target(
    "build-sdks",
    json!({
      "stage-dir": directory.join("sdk"), "configuration": project.string("configuration")?
    }),
  )?;
  std::fs::copy(project.path("LICENSE.md"), directory.join("LICENSE.md"))?;
  println!("Application: {}", directory.display());
  Ok(())
}

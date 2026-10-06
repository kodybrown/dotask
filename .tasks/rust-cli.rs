// dotask: 1
// description: Build the native CLI with its Rust task crate and C# helper DLL.
// options:
//   - { name: verify, type: bool, default: false, description: 'Run Rust tests, formatting, and clippy.' }
//   - { name: build-info, description: 'JSON build metadata supplied by create-installer before compiling.' }
// requires:
//   - { kind: tool, value: cargo }
//   - { kind: tool, value: dotnet }
//   - { kind: file, value: _support/RustBuild.rs }
//   - { kind: file, value: _support/BuildInfo.rs }
// end-dotask
#[path = "_support/RustBuild.rs"]
mod rust_build;
use dotask_sdk::{serde_json, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let (published, _) = rust_build::publish_library(
    project,
    &[
      "-p:Configuration=Release".into(),
      "-p:SelfContained=false".into(),
      "-p:UseAppHost=false".into(),
    ],
  )?;
  let output = rust_build::target_directory(project)?.join("release");
  // Build and stage from physical copies. The running bootstrap is independent
  // of this output and remains usable while repository tasks rebuild it.
  std::fs::copy(published.path().join("Dotask.dotnet.dll"), output.join("Dotask.dotnet.dll"))?;
  rust_build::stage_sdk(project, &output.join("sdk"))?;
  if project.boolean("verify")? {
    let mut format = project.command("rustfmt");
    format.args(["--check", "--edition", "2024"]);
    let tasks = project.path(".tasks");
    for directory in [&tasks, &tasks.join("_support")] {
      for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "rs") {
          format.arg(path);
        }
      }
    }
    project.execute(&mut format)?;
    rust_build::cargo(
      project,
      &[
        "fmt",
        "--package",
        "dotask-cli",
        "--package",
        "dotask-sdk",
        "--",
        "--check",
      ],
      "Cargo.toml",
    )?;
    rust_build::cargo(
      project,
      &[
        "test",
        "--package",
        "dotask-cli",
        "--package",
        "dotask-sdk",
        "--locked",
        "--release",
      ],
      "Cargo.toml",
    )?;
    rust_build::cargo(
      project,
      &[
        "clippy",
        "--package",
        "dotask-cli",
        "--package",
        "dotask-sdk",
        "--locked",
        "--all-targets",
        "--",
        "-D",
        "warnings",
      ],
      "Cargo.toml",
    )?;
  } else {
    let build = project
      .parameter("build-info")
      .ok()
      .and_then(|value| value.as_str())
      .map(serde_json::from_str::<dotask_sdk::Value>)
      .transpose()?;
    rust_build::cargo_build(
      project,
      &["build", "--package", "dotask-cli", "--locked", "--release"],
      "Cargo.toml",
      build.as_ref(),
    )?;
  }
  println!(
    "{}",
    output
      .join(if cfg!(windows) {
        "dotask.exe"
      } else {
        "dotask"
      })
      .display()
  );
  Ok(())
}

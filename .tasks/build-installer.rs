// dotask: 1
// description: Build the standalone installer and its build-machine creator.
// options:
//   - { name: stage-dir, type: path, description: Copy the installer and builder into this directory. }
//   - { name: verify, type: bool, default: false, description: 'Run Rust tests, formatting, and clippy.' }
// requires:
//   - { kind: tool, value: cargo }
//   - { kind: file, value: _support/RustBuild.rs }
//   - { kind: file, value: _support/BuildInfo.rs }
// end-dotask
#[path = "_support/RustBuild.rs"]
mod rust_build;
use dotask_sdk::{BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  if project.boolean("verify")? {
    rust_build::cargo(
      project,
      &["fmt", "--package", "dotask-installer", "--", "--check"],
      "Cargo.toml",
    )?;
    rust_build::cargo(
      project,
      &[
        "test",
        "--package",
        "dotask-installer",
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
        "dotask-installer",
        "--locked",
        "--all-targets",
        "--",
        "-D",
        "warnings",
      ],
      "Cargo.toml",
    )?;
  } else {
    rust_build::cargo(
      project,
      &[
        "build",
        "--package",
        "dotask-installer",
        "--locked",
        "--release",
      ],
      "Cargo.toml",
    )?;
  }
  if let Ok(stage) = project.string("stage-dir") {
    let stage = project.path(stage);
    std::fs::create_dir_all(&stage)?;
    let output = rust_build::target_directory(project)?.join("release");
    for binary in ["dotask-installer", "simple-installer-builder"] {
      let name = format!("{binary}{}", if cfg!(windows) { ".exe" } else { "" });
      std::fs::copy(output.join(&name), stage.join(name))?;
    }
  }
  println!(
    "{}",
    rust_build::target_directory(project)?
      .join("release")
      .join(if cfg!(windows) {
        "dotask-installer.exe"
      } else {
        "dotask-installer"
      })
      .display()
  );
  Ok(())
}

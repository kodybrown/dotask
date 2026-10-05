//! ---
//! description: Build the host Rust installer, or run its tests and formatting checks.
//! options:
//!   - { name: verify, type: bool, default: false, description: 'Run Rust tests, formatting, and clippy.' }
//! requires:
//!   - { kind: tool, value: cargo }
//!   - { kind: file, value: _support/RustBuild.rs }
//! ---
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

// Each file-based task compiles its own copy and uses only its needed helpers.
#![allow(dead_code)]
#[path = "BuildInfo.rs"]
pub mod build_info;
use dotask_sdk::{serde_json, BuildContext, Context, Result, Value};
use std::{
  fs,
  path::{Path, PathBuf},
};

pub fn target_directory(project: &BuildContext) -> Result<PathBuf> {
  // Query Cargo rather than guessing or overriding the user's output policy.
  let output = project.capture(
    project
      .command("cargo")
      .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
      .arg("--manifest-path")
      .arg(project.path("Cargo.toml")),
  )?;
  let metadata: Value = serde_json::from_str(&output)?;
  Ok(PathBuf::from(
    metadata["target_directory"]
      .as_str()
      .context("Cargo returned no target_directory")?,
  ))
}
pub fn cargo(project: &BuildContext, arguments: &[&str], manifest: &str) -> Result<()> {
  cargo_build(project, arguments, manifest, None)
}
pub fn cargo_build(project: &BuildContext, arguments: &[&str], manifest: &str, build: Option<&Value>) -> Result<()> {
  let mut command = project.command("cargo");
  let split = arguments.iter().position(|a| *a == "--").unwrap_or(arguments.len());
  command
    .args(&arguments[..split])
    .arg("--manifest-path")
    .arg(project.path(manifest))
    .args(&arguments[split..]);
  if cfg!(windows) {
    command.env("RUSTFLAGS", "-C target-feature=+crt-static");
  }
  if let Some(build) = build {
    command.env("DOTASK_BUILD_STAMP", build["stamp"].as_str().context("Missing build stamp")?);
    command.env("DOTASK_BUILD_COMMIT", build["commit"].as_str().unwrap_or(""));
    command.env("DOTASK_BUILD_DIRTY", build["dirty"].as_bool().unwrap_or(false).to_string());
    if let Some(version) = build["version"].as_str() {
      command.env("DOTASK_APP_VERSION", version);
    }
  }
  project.execute(&mut command)
}
pub fn package_lock(platform: &Path) -> Result<fs::File> {
  use dotask_sdk::sha2::{Digest, Sha256};
  let directory = fs::canonicalize(std::env::temp_dir())?.join("dotask-package-locks");
  fs::create_dir_all(&directory)?;
  let key = format!("{:x}", Sha256::digest(platform.to_string_lossy().to_lowercase().as_bytes()));
  let file = fs::OpenOptions::new()
    .create(true)
    .truncate(false)
    .read(true)
    .write(true)
    .open(directory.join(key))?;
  file
    .try_lock()
    .context("Another create-installer task is using this output directory")?;
  Ok(file)
}

// Product staging consumes Cargo's evaluated paths and the shared build's
// successful executable inventory. Never infer a binary from an old output.
#![allow(dead_code)]
use dotask_sdk::{
  serde_json,
  sha2::{Digest, Sha256},
  BuildContext, Context, Result, Value,
};
use std::{fs, path::PathBuf};
pub fn metadata(project: &BuildContext) -> Result<Value> {
  Ok(serde_json::from_str(&project.capture(project.command("cargo").args([
    "metadata",
    "--format-version",
    "1",
    "--no-deps",
    "--locked",
  ]))?)?)
}
pub fn target_directory(project: &BuildContext) -> Result<PathBuf> {
  let data = metadata(project)?;
  Ok(PathBuf::from(
    data["target_directory"].as_str().context("Missing Cargo target_directory")?,
  ))
}
pub fn external_directory(project: &BuildContext, path: &std::path::Path) -> Result<()> {
  let root = fs::canonicalize(project.root())?;
  let directory = std::path::absolute(path)?;
  // Resolve the nearest existing ancestor before creating anything, including
  // Windows junctions and extended-length spellings of an otherwise new path.
  let mut ancestor = directory.as_path();
  while !ancestor.exists() {
    ancestor = ancestor.parent().context("Output path has no existing ancestor")?;
  }
  let resolved = fs::canonicalize(ancestor)?.join(directory.strip_prefix(ancestor)?);
  if resolved.starts_with(&root) || root.starts_with(&resolved) {
    dotask_sdk::bail!("Generated output must stay outside source: {}", directory.display());
  }
  Ok(())
}
pub fn inspected_binary(project: &BuildContext) -> Result<PathBuf> {
  inspected_executable(project, "dotask")
}
pub fn inspected_executable(project: &BuildContext, name: &str) -> Result<PathBuf> {
  let data = metadata(project)?;
  let package = data["packages"]
    .as_array()
    .context("Missing packages")?
    .iter()
    .find(|package| package["name"] == "dotask-cli")
    .context("Missing dotask-cli package")?;
  let report: Value = serde_json::from_slice(
    &fs::read(target_directory(project)?.join("rust-build/dotask-cli-executables.json"))
      .context("Run build or test before staging dotask")?,
  )?;
  if report["version"] != package["version"] || report["profile"] != "release" {
    dotask_sdk::bail!("Build the current dotask package in release profile before staging");
  }
  let binary = PathBuf::from(report["binaries"][name].as_str().context("Missing built dotask executable")?);
  let expected = report["sha256"][name].as_str().context("Missing inspected dotask hash")?;
  if format!("{:x}", Sha256::digest(fs::read(&binary)?)) != expected {
    dotask_sdk::bail!("Dotask executable changed since inspection; run build or test again");
  }
  Ok(binary)
}

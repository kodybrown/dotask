//! Cargo-only release tooling; never included in dotask's runtime payload.
use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf, process::Command};
mod release;
fn root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().into()
}
fn metadata() -> Result<Value> {
  Ok(serde_json::from_str(&release::capture(
    Command::new("cargo")
      .current_dir(root())
      .args(["metadata", "--locked", "--format-version", "1", "--no-deps"]),
  )?)?)
}
fn target() -> Result<PathBuf> {
  Ok(PathBuf::from(
    metadata()?["target_directory"].as_str().context("Missing target directory")?,
  ))
}
fn executable(name: &str) -> String {
  format!("{name}{}", std::env::consts::EXE_SUFFIX)
}
fn execute(command: &mut Command) -> Result<()> {
  ensure!(
    command.status().context("Cannot start release tool")?.success(),
    "Release tool failed"
  );
  Ok(())
}
fn main() -> Result<()> {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let action = args.first().context("Expected version, prepare, build or collect")?;
  let mut options = BTreeMap::new();
  let mut publishing = false;
  let mut rest = args[1..].iter();
  while let Some(key) = rest.next() {
    if key == "--publishing" {
      ensure!(!publishing, "Duplicate publishing option");
      publishing = true;
    } else {
      ensure!(
        [
          "--output",
          "--source",
          "--stage",
          "--tools",
          "--commit",
          "--result-file"
        ]
        .contains(&key.as_str()),
        "Unknown argument"
      );
      ensure!(
        options
          .insert(key.as_str(), rest.next().context("Missing option value")?.as_str())
          .is_none(),
        "Duplicate argument"
      );
    }
  }
  let get = |name| options.get(name).copied().with_context(|| format!("Missing {name}"));
  match action.as_str() {
    "version" => println!("{}", release::version(None)?),
    "prepare" => release::prepare(std::path::Path::new(get("--output")?))?,
    "build" => release::build(
      std::path::Path::new(get("--output")?),
      std::path::Path::new(get("--stage")?),
      std::path::Path::new(get("--tools")?),
      options.get("--result-file").map(std::path::Path::new),
    )?,
    "collect" => release::collect(
      std::path::Path::new(get("--source")?),
      std::path::Path::new(get("--output")?),
      None,
      get("--commit")?,
      publishing,
    )?,
    _ => bail!("Expected version, prepare, build or collect"),
  }
  Ok(())
}

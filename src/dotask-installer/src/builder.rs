//! Build-machine packaging lives in this executable, never in the shipped installer.
#![allow(dead_code)]
#[path = "../../../.tasks/_support/BuildInfo.rs"]
mod build_info;
mod config;
mod console;
mod engine;
mod files;
mod launchers;
mod model;
mod package;
mod user_path;
use anyhow::{bail, ensure, Context, Result};
use std::path::PathBuf;
fn main() {
  if let Err(error) = run() {
    eprintln!("simple-installer-builder: {error:#}");
    std::process::exit(1);
  }
}
fn run() -> Result<()> {
  let mut arguments = std::env::args().skip(1);
  let mut installer = None;
  let mut config = None;
  let mut output = None;
  let mut named = false;
  let mut result_file = None;
  while let Some(argument) = arguments.next() {
    match argument.as_str() {
      "--installer" => {
        ensure!(installer.is_none(), "Duplicate --installer");
        installer = Some(PathBuf::from(arguments.next().context("--installer requires a file")?));
      }
      "--config" => {
        ensure!(config.is_none(), "Duplicate --config");
        config = Some(PathBuf::from(arguments.next().context("--config requires a file")?));
      }
      "--output" | "--output-parent" => {
        ensure!(output.is_none(), "Specify one output directory");
        named = argument == "--output-parent";
        output = Some(PathBuf::from(arguments.next().context("Output requires a directory")?));
      }
      "--result-file" => {
        ensure!(result_file.is_none(), "Duplicate --result-file");
        result_file = Some(PathBuf::from(arguments.next().context("--result-file requires a path")?));
      }
      "--help" | "-h" => {
        println!("simple-installer-builder --installer FILE --config FILE --output NEW_DIRECTORY\n  --output-parent DIRECTORY  Derive directory from application build metadata\n  --result-file FILE         Write the created installer path as JSON");
        return Ok(());
      }
      _ => bail!("Unknown builder argument: {argument}. Use --help."),
    }
  }
  let installer = installer.context("--installer is required")?;
  let config = config.context("--config is required")?;
  let output = output.context("--output or --output-parent is required")?;
  let artifact = package::package(&config, &output, named, &installer)?;
  if let Some(result) = result_file {
    files::write_atomic(&result, &serde_json::to_vec(&serde_json::json!({"FilePath":artifact}))?)?;
  }
  Ok(())
}

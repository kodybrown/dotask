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
pub fn choose_stamp(
  platform: &Path,
  app: &str,
  prefix: &str,
  custom_version: Option<&str>,
  requested: build_info::Stamp,
) -> Result<build_info::Stamp> {
  let mut stamp = requested;
  loop {
    let version = custom_version.map(String::from).unwrap_or_else(|| stamp.version(prefix));
    let component = build_info::version_component(&version).map_err(|error| dotask_sdk::anyhow!(error))?;
    let start = format!("{app}-{component}-{}", stamp.text());
    let mut taken = false;
    for entry in fs::read_dir(platform)? {
      let entry = entry?;
      let name = entry.file_name().to_string_lossy().into_owned();
      if entry.path().is_dir() && (name == start || name.starts_with(&format!("{start}-"))) {
        taken = true;
        break;
      }
    }
    if !taken {
      return Ok(stamp);
    }
    let next = stamp.next();
    println!(
      "Warning: build minute {} already exists; advancing the build stamp to {} (UTC).",
      stamp.text(),
      next.text()
    );
    stamp = next;
  }
}
pub fn publish_library(
  project: &BuildContext,
  properties: &[String],
) -> Result<(dotask_sdk::tempfile::TempDir, String)> {
  let application = project.path("src/Dotask/Dotask.csproj");
  let output = project.capture(
    project
      .command("dotnet")
      .arg("publish")
      .arg(&application)
      .args([
        "--nologo",
        "--verbosity",
        "quiet",
        "-getProperty:PublishDir,Version",
        "-getItem:ResolvedFileToPublish",
      ])
      .args(properties),
  )?;
  // MSBuild may print warnings before its evaluated-property JSON. Keep those
  // visible and use the evaluated PublishDir, never a source-local bin path.
  let start = output
    .find("{\n")
    .or_else(|| output.find("{\r\n"))
    .context("The .NET SDK did not return evaluated publish metadata")?;
  if !output[..start].trim().is_empty() {
    println!("{}", output[..start].trim());
  }
  let metadata: Value = serde_json::from_str(&output[start..])?;
  let directory = PathBuf::from(metadata["Properties"]["PublishDir"].as_str().context("Missing PublishDir")?);
  let directory = if directory.is_absolute() {
    directory
  } else {
    application.parent().unwrap().join(directory)
  };
  let staged = dotask_sdk::tempfile::Builder::new()
    .prefix("dotask-library-publish-")
    .tempdir()?;
  // The task helper is a platform-neutral DLL with no package dependencies.
  // Explicitly select it from evaluated PublishDir: no runtime, symbols, stale
  // host or other publish files can enter the application payload.
  fs::copy(directory.join("Dotask.dotnet.dll"), staged.path().join("Dotask.dotnet.dll"))?;
  Ok((
    staged,
    metadata["Properties"]["Version"].as_str().context("Missing Version")?.into(),
  ))
}
pub fn stage_sdk(project: &BuildContext, destination: &Path) -> Result<()> {
  fs::create_dir_all(destination.join("src"))?;
  for name in ["Cargo.toml", "src/lib.rs"] {
    fs::copy(project.path("src/dotask-sdk").join(name), destination.join(name))?;
  }
  Ok(())
}

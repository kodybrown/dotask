// Each file-based task compiles its own copy and uses only its needed helpers.
#![allow(dead_code)]
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
  project.execute(&mut command)
}
pub fn publish_host(project: &BuildContext, properties: &[String]) -> Result<(dotask_sdk::tempfile::TempDir, String)> {
  let application = project.path("src/Dotask.CSharpHost/Dotask.CSharpHost.csproj");
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
  let staged = dotask_sdk::tempfile::Builder::new().prefix("dotask-host-publish-").tempdir()?;
  // Copy the evaluated publish inventory, not every file left in PublishDir.
  // Switching self-contained mode cannot accidentally package stale runtimes.
  for item in metadata["Items"]["ResolvedFileToPublish"]
    .as_array()
    .context("Missing publish inventory")?
  {
    let relative = Path::new(item["RelativePath"].as_str().context("Missing publish RelativePath")?);
    if relative.is_absolute() || relative.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
      dotask_sdk::bail!("Publish inventory must use relative paths.");
    }
    let destination = staged.path().join(relative);
    fs::create_dir_all(destination.parent().unwrap())?;
    fs::copy(directory.join(relative), destination)?;
  }
  Ok((
    staged,
    metadata["Properties"]["Version"].as_str().context("Missing Version")?.into(),
  ))
}
pub fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
  fs::create_dir_all(destination)?;
  for entry in fs::read_dir(source)? {
    let entry = entry?;
    let file_type = entry.file_type()?;
    if file_type.is_symlink() {
      dotask_sdk::bail!("Cannot stage symbolic link {}", entry.path().display());
    }
    let target = destination.join(entry.file_name());
    if file_type.is_dir() {
      copy_tree(&entry.path(), &target)?;
    } else {
      fs::copy(entry.path(), target)?;
    }
  }
  Ok(())
}
pub fn stage_sdk(project: &BuildContext, destination: &Path) -> Result<()> {
  fs::create_dir_all(destination.join("src"))?;
  for name in ["Cargo.toml", "src/lib.rs"] {
    fs::copy(project.path("src/dotask-sdk").join(name), destination.join(name))?;
  }
  Ok(())
}

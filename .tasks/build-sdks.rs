// dotask: 1
// description: Build and prepare the configured C# and Rust task SDK files.
// options:
//   - { name: configuration, alias: c, choices: [Debug, Release], default: Release }
//   - { name: stage-dir, type: path, required: true, description: Fresh SDK staging directory. }
// requires: [{ kind: setting, value: sdks.config }, { kind: tool, value: dotnet }]
// end-dotask
use dotask_sdk::{serde_json, serde_saphyr, BuildContext, Context, Result, Value};
use std::{
  fs,
  path::{Component, Path},
};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let config: Value = serde_saphyr::from_str(&fs::read_to_string(project.setting_path("sdks.config")?)?)?;
  let destination = project.path(project.string("stage-dir")?);
  let application = project.path(
    config["dotnet"]["project"]
      .as_str()
      .context("sdks.dotnet.project is required")?,
  );
  let output = project.capture(
    project
      .command("dotnet")
      .arg("publish")
      .arg(&application)
      .args([
        "--nologo",
        "--verbosity",
        "quiet",
        "--self-contained=false",
        "-p:UseAppHost=false",
        "-getProperty:PublishDir",
        "-getItem:ResolvedFileToPublish",
      ])
      .arg(format!("-p:Configuration={}", project.string("configuration")?)),
  )?;
  let start = output
    .find("{\n")
    .or_else(|| output.find("{\r\n"))
    .context("MSBuild returned no publish metadata")?;
  if !output[..start].trim().is_empty() {
    println!("{}", output[..start].trim());
  }
  let metadata: Value = serde_json::from_str(&output[start..])?;
  let published = Path::new(metadata["Properties"]["PublishDir"].as_str().context("Missing PublishDir")?);
  let published = if published.is_absolute() {
    published.to_path_buf()
  } else {
    application.parent().unwrap().join(published)
  };
  // Use publish items to reject a stale file left by an earlier build.
  let items = metadata["Items"]["ResolvedFileToPublish"]
    .as_array()
    .context("Missing publish file inventory")?;
  for file in config["dotnet"]["files"].as_array().context("Missing .NET SDK files")? {
    let name = file.as_str().context("SDK file must be a string")?;
    if !items.iter().any(|item| item["RelativePath"].as_str() == Some(name)) {
      dotask_sdk::bail!("Expected SDK file is not part of this publish: {name}");
    }
    copy(&published, &destination.join("dotnet"), name)?;
  }
  let source = project.path(config["rust"]["directory"].as_str().context("Missing Rust SDK directory")?);
  for file in config["rust"]["files"].as_array().context("Missing Rust SDK files")? {
    copy(
      &source,
      &destination.join("rust"),
      file.as_str().context("SDK file must be a string")?,
    )?;
  }
  Ok(())
}
fn copy(source: &Path, destination: &Path, name: &str) -> Result<()> {
  let relative = Path::new(name);
  if relative.components().any(|c| !matches!(c, Component::Normal(_))) || name.contains('\\') {
    dotask_sdk::bail!("SDK files must be portable relative paths: {name}");
  }
  let target = destination.join(relative);
  fs::create_dir_all(target.parent().unwrap())?;
  fs::copy(source.join(relative), target)?;
  Ok(())
}

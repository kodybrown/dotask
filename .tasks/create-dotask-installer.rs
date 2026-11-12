// dotask: 1
// description: Build dotask and DoInstall from source and exercise the portable distribution and installer.
// options:
//   - { name: doinstall-tools, type: path, description: Use a matching prebuilt DoInstall bundle instead of building from source. }
//   - { name: source, type: path, description: DoInstall checkout override; defaults to settings.doinstall.source. }
//   - { name: reuse-build, type: bool, default: false, description: Package the exact inspected build after the full test gate. }
//   - { name: output, type: path, description: Fresh external delivery directory; otherwise artifacts/releases/OS-ARCH in the checkout. }
// requires:
//   - { kind: task, value: _/doinstall/build }
//   - { kind: task, value: build }
//   - { kind: task, value: stage-build }
//   - { kind: task, value: _/doinstall/create-installer }
//   - { kind: file, value: _support/RustBuild.rs }
//   - { kind: file, value: _support/DoinstallTools.rs }
//   - { kind: file, value: _support/ReleaseDelivery.rs }
// examples: [dotask create-dotask-installer, dotask create-dotask-installer --source ../doinstall]
// remarks: DOINSTALL_TOOL_DIR selects prebuilt tools unless --source is explicit. Tests temporary installations without changing PATH or active installs.
// end-dotask
#[path = "_support/DoinstallTools.rs"]
mod doinstall_tools;
#[path = "_support/ReleaseDelivery.rs"]
mod release_delivery;
#[path = "_support/RustBuild.rs"]
mod rust_build;
use doinstall_tools::Tools;
use dotask_sdk::{json, serde_json, tempfile, BuildContext, Context, Result, Value};
use std::path::{Path, PathBuf};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let prepared = Tools::prepare(project)?;
  let tools = std::fs::canonicalize(prepared.path())?;
  if !project.boolean("reuse-build")? {
    project.exec_target("build", json!({}))?;
  }
  // Inspect before staging; neither bootstrap nor assembly may replace this file.
  let binary = rust_build::inspected_binary(project)?;
  let output = project.string("output").ok().map(|value| project.path(value));
  if let Some(path) = &output {
    rust_build::external_directory(project, path)?;
  }
  if cfg!(target_os = "macos") {
    let destination = match output {
      Some(path) => path,
      None => release_delivery::directory(project)?,
    };
    let identity: Value = serde_json::from_str(&project.capture(project.command(binary).arg("__build-info"))?)?;
    let artifact = project.create_installer(
      "_/doinstall/create-installer",
      json!({
        "builder":tools.join("doinstall-builder"), "installer":tools.join("doinstall"),
        "launcher-template":tools.join("doinstall-launcher"), "output":destination,
        "app-version":identity["version"], "build-stamp":identity["stamp"]
      }),
    )?;
    return project.set_installer_result(&artifact);
  }
  let stage = tempfile::Builder::new().prefix("dotask-release-stage-").tempdir()?;
  let application = stage.path().join("app");
  project.exec_target("stage-build", json!({"stage-dir":application}))?;
  let destination = output.clone().unwrap_or_else(|| stage.path().join("release"));
  let report = stage.path().join("result.json");
  project.execute(
    project
      .command("cargo")
      .args([
        "run",
        "--quiet",
        "--locked",
        "--package",
        "dotask-release",
        "--",
        "build",
      ])
      .arg("--stage")
      .arg(&application)
      .arg(if matches!(prepared, Tools::Source(_)) {
        "--source-tools"
      } else {
        "--tools"
      })
      .arg(&tools)
      .arg("--output")
      .arg(&destination)
      .arg("--result-file")
      .arg(&report),
  )?;
  let result: Value = serde_json::from_slice(&std::fs::read(report)?)?;
  let file = Path::new(result["FilePath"].as_str().context("Missing installer FilePath")?);
  let file = if output.is_some() {
    file.to_owned()
  } else {
    let files: Vec<PathBuf> = serde_json::from_value(result["Files"].clone())?;
    release_delivery::deliver(project, &destination, file, &files)?
  };
  let artifact = project.installer_artifact(&file)?;
  project.record_installer(&artifact)?;
  println!("Installer: {}", file.display());
  project.set_installer_result(&artifact)
}

// dotask: 1
// description: Build dotask and DoInstall from source and exercise the portable distribution and installer.
// options:
//   - { name: doinstall-tools, type: path, description: Use a matching prebuilt DoInstall bundle instead of building from source. }
//   - { name: source, type: path, description: DoInstall checkout override; defaults to settings.doinstall.source. }
//   - { name: reuse-build, type: bool, default: false, description: Package the exact inspected build after the full test gate. }
//   - { name: output, type: path, description: Fresh native delivery directory; otherwise external Cargo output. }
// requires:
//   - { kind: task, value: _/doinstall/build }
//   - { kind: task, value: build }
//   - { kind: task, value: test }
//   - { kind: task, value: stage-build }
//   - { kind: task, value: _/doinstall/create-installer }
//   - { kind: file, value: _support/RustBuild.rs }
//   - { kind: file, value: _support/DoinstallTools.rs }
// examples: [dotask create-dotask-installer, dotask create-dotask-installer --source ../doinstall]
// remarks: DOINSTALL_TOOL_DIR selects prebuilt tools unless --source is explicit. Tests temporary installations without changing PATH or active installs.
// end-dotask
#[path = "_support/DoinstallTools.rs"]
mod doinstall_tools;
#[path = "_support/RustBuild.rs"]
mod rust_build;
use doinstall_tools::Tools;
use dotask_sdk::{json, serde_json, tempfile, BuildContext, Context, Result, Value};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let prepared = Tools::prepare(project)?;
  let tools = std::fs::canonicalize(prepared.path())?;
  if cfg!(target_os = "macos") && !project.boolean("reuse-build")? {
    // Preserve the existing macOS source-development verification/package gate.
    // Published portable bundles currently exist only for Windows/Linux x64.
    project.exec_target("test", json!({"doinstall-tools":tools}))?;
  } else if !project.boolean("reuse-build")? {
    project.exec_target("build", json!({}))?;
  }
  // Inspect before staging; neither bootstrap nor assembly may replace this file.
  let binary = rust_build::inspected_binary(project)?;
  let output_root = rust_build::target_directory(project)?.join("installers");
  std::fs::create_dir_all(&output_root)?;
  let output = if project.string("output").is_err() {
    Some(tempfile::Builder::new().prefix("dotask-").tempdir_in(output_root)?)
  } else {
    None
  };
  let destination = match project.string("output") {
    Ok(value) => project.path(value),
    Err(_) => output.as_ref().unwrap().path().join("release"),
  };
  rust_build::external_directory(project, &destination)?;
  if cfg!(target_os = "macos") {
    let identity: Value = serde_json::from_str(&project.capture(project.command(binary).arg("__build-info"))?)?;
    let artifact = project.create_installer(
      "_/doinstall/create-installer",
      json!({
        "builder":tools.join("doinstall-builder"), "installer":tools.join("doinstall"),
        "launcher-template":tools.join("doinstall-launcher"), "output":destination,
        "app-version":identity["version"], "build-stamp":identity["stamp"]
      }),
    )?;
    if let Some(output) = output {
      let _ = output.keep();
    }
    return project.set_installer_result(&artifact);
  }
  let stage = tempfile::Builder::new().prefix("dotask-release-stage-").tempdir()?;
  project.exec_target("stage-build", json!({"stage-dir":stage.path()}))?;
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
      .arg(stage.path())
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
  let artifact: Value = serde_json::from_slice(&std::fs::read(report)?)?;
  if let Some(output) = output {
    let _ = output.keep();
  }
  println!(
    "Installer: {}",
    artifact["FilePath"].as_str().context("Missing installer FilePath")?
  );
  let artifact =
    project.installer_artifact(std::path::Path::new(artifact["FilePath"].as_str().context("Missing FilePath")?))?;
  project.record_installer(&artifact)?;
  project.set_installer_result(&artifact)
}

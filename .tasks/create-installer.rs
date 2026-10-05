// dotask: 1
// description: Create dotask's native CLI installer without installing it.
// options:
//   - { name: configuration, alias: c, choices: [Debug, Release], default: Release, description: C# helper build configuration. }
// requires:
//   - { kind: tool, value: dotnet }
//   - { kind: tool, value: cargo }
//   - { kind: setting, value: installer-output }
//   - { kind: file, value: _support/RustBuild.rs }
// examples: [dotask create-installer]
// end-dotask
#[path = "_support/RustBuild.rs"]
mod rust_build;
use dotask_sdk::{json, serde_json, tempfile, BuildContext, Result};
use std::fs;
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let output = project.setting_path("installer-output")?;
  project.exec_target("installer-engine", json!({}))?;
  project.exec_target("rust-cli", json!({}))?;
  let (published, version) = rust_build::publish_library(
    project,
    &[
      format!("-p:Configuration={}", project.string("configuration")?),
      "-p:SelfContained=false".into(),
      "-p:UseAppHost=false".into(),
    ],
  )?;
  let temporary = tempfile::Builder::new().prefix("dotask-native-package-").tempdir()?;
  let payload = temporary.path().join("payload");
  fs::create_dir(&payload)?;
  let cargo_output = rust_build::target_directory(project)?.join("release");
  let executable = if cfg!(windows) {
    "dotask.exe"
  } else {
    "dotask"
  };
  fs::copy(cargo_output.join(executable), payload.join(executable))?;
  fs::copy(published.path().join("Dotask.dotnet.dll"), payload.join("Dotask.dotnet.dll"))?;
  rust_build::stage_sdk(project, &payload.join("sdk"))?;
  let config = temporary.path().join("installer.yaml");
  // JSON is a YAML subset. The installer emits the final readable YAML and
  // immutable receipt; the task never installs or modifies the active command.
  fs::write(
    &config,
    serde_json::to_vec(&json!({
      "schema":1,"application":{"id":"dotask","name":"dotask","version":version,"author":"Kody Brown","description":"Portable project tasks"},
      "platform":project.os(),"architecture":project.architecture(),"payload":payload,
      "commands":[{"name":"dotask","executable":executable}]
    }))?,
  )?;
  let engine = cargo_output.join(if cfg!(windows) {
    "dotask-installer.exe"
  } else {
    "dotask-installer"
  });
  let platform = output.join(format!("{}-{}", project.os(), project.architecture()));
  fs::create_dir_all(&platform)?;
  let package_root = tempfile::Builder::new().prefix("dotask-").tempdir_in(&platform)?;
  let package = package_root.path().join("package");
  project.execute(
    project
      .command(engine)
      .arg("package")
      .arg("--config")
      .arg(config)
      .arg("--output")
      .arg(&package),
  )?;
  let artifact = project.installer_artifact(&package.join(if cfg!(windows) {
    "installer.exe"
  } else {
    "installer"
  }))?;
  project.set_installer_result(&artifact)?;
  let _ = package_root.keep();
  println!("Distribute the entire package directory, including installer.yaml and payload.");
  Ok(())
}

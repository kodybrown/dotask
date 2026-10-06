// dotask: 1
// description: Create dotask's native CLI installer without installing it.
// options:
//   - { name: configuration, alias: c, choices: [Debug, Release], default: Release, description: C# helper build configuration. }
//   - { name: app-version, description: 'Optional app version; otherwise major.minor.yyMM.ddhh.' }
//   - { name: build-stamp, description: 'Optional UTC YYDDD-HHMM stamp for reproducible builds.' }
//   - { name: git-hash, type: bool, default: true, description: Include the Git revision when available. }
// requires:
//   - { kind: tool, value: dotnet }
//   - { kind: tool, value: cargo }
//   - { kind: setting, value: installer-output }
//   - { kind: file, value: _support/RustBuild.rs }
//   - { kind: file, value: _support/BuildInfo.rs }
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
  let platform = output.join(format!("{}-{}", project.os(), project.architecture()));
  fs::create_dir_all(&platform)?;
  let _lock = rust_build::package_lock(&platform)?;
  let custom = project.parameter("app-version").ok().and_then(|v| v.as_str());
  let metadata: dotask_sdk::Value = serde_json::from_str(&project.capture(project.command("cargo").args([
    "metadata",
    "--format-version",
    "1",
    "--no-deps",
    "--locked",
  ]))?)?;
  let package_version = metadata["packages"]
    .as_array()
    .unwrap()
    .iter()
    .find(|p| p["name"] == "dotask-cli")
    .unwrap()["version"]
    .as_str()
    .unwrap();
  let prefix = package_version.split('.').take(2).collect::<Vec<_>>().join(".");
  let requested = project
    .string("build-stamp")
    .ok()
    .map(rust_build::build_info::Stamp::parse)
    .transpose()
    .map_err(|error| dotask_sdk::anyhow!(error))?
    .unwrap_or_else(rust_build::build_info::Stamp::now);
  let stamp = rust_build::choose_stamp(&platform, "dotask", &prefix, custom, requested)?;
  let (mut commit, mut dirty) = rust_build::build_info::git(project.root());
  if !project.boolean("git-hash")? {
    commit.clear();
    dirty = false;
  }
  let version = custom.map(String::from).unwrap_or_else(|| stamp.version(&prefix));
  let build = json!({"version":version,"stamp":stamp.text(),"commit":if commit.is_empty(){None}else{Some(commit.as_str())},"dirty":dirty});
  project.exec_target("installer-engine", json!({}))?;
  project.exec_target("rust-cli", json!({"build-info":serde_json::to_string(&build)?}))?;
  let (published, _) = rust_build::publish_library(
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
  let embedded: dotask_sdk::Value =
    serde_json::from_str(&project.capture(project.command(cargo_output.join(executable)).arg("__build-info"))?)?;
  if embedded != build {
    dotask_sdk::bail!("Compiled app build information does not match the packaging plan");
  }
  fs::copy(cargo_output.join(executable), payload.join(executable))?;
  fs::copy(published.path().join("Dotask.dotnet.dll"), payload.join("Dotask.dotnet.dll"))?;
  rust_build::stage_sdk(project, &payload.join("sdk"))?;
  let config = temporary.path().join("installer.yaml");
  // JSON is a YAML subset. The installer emits the final readable YAML and
  // immutable receipt; the task never installs or modifies the active command.
  fs::write(
    &config,
    serde_json::to_vec(&json!({
      "schema":1,"application":{"id":"dotask","name":"DoTask","version":version,"build":{"stamp":build["stamp"],"commit":build["commit"],"dirty":dirty},"author":"Kody Brown","copyright":"Copyright (C) 2026 Kody Brown","description":"Portable project tasks"},
      "platform":project.os(),"architecture":project.architecture(),"payload":payload,"interactive":true,
      "inputs":{"install-dir":{"type":"path","required":true,"prompt":"Install dotask in"},
        "additional-command":{"type":"boolean","default":false,"prompt":"Place an additional dotask command in another directory?"}},
      "defaults":{"common":{"additional-command":false},"windows":{"add-to-path":true}},
      "commands":[{"name":"dotask","executable":executable}]
    }))?,
  )?;
  let engine = cargo_output.join(if cfg!(windows) {
    "dotask-installer.exe"
  } else {
    "dotask-installer"
  });
  let component = rust_build::build_info::version_component(&version).map_err(|error| dotask_sdk::anyhow!(error))?;
  let package = platform.join(format!(
    "dotask-{component}-{}{}",
    stamp.text(),
    rust_build::build_info::revision(&commit, dirty)
  ));
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
  println!("Distribute the entire package directory, including installer.yaml and payload.");
  Ok(())
}

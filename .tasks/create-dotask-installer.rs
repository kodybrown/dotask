// dotask: 1
// description: Test dotask and create its self-extracting installer using matching prebuilt doinstall tools.
// options:
//   - { name: doinstall-tools, type: path, description: Prebuilt matching doinstall tools; otherwise DOINSTALL_TOOL_DIR or builder discovery. }
//   - { name: source, type: path, description: Explicit doinstall source-development workflow; builds and verifies this checkout instead. }
//   - { name: output, type: path, description: Installer parent directory; otherwise a fresh external Cargo output directory. }
// requires:
//   - { kind: task, value: _/doinstall/build }
//   - { kind: task, value: test }
//   - { kind: task, value: _/doinstall/create-installer }
//   - { kind: file, value: _support/RustBuild.rs }
// examples: [dotask create-dotask-installer, dotask create-dotask-installer --source ../doinstall]
// remarks: Prebuilt tools are never downloaded or compiled implicitly. Source mode is explicit. Never installs the result.
// end-dotask
#[path = "_support/RustBuild.rs"]
mod rust_build;
use dotask_sdk::{json, serde_json, tempfile, BuildContext, Context, Result, Value};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  if project.string("source").is_ok() && project.string("doinstall-tools").is_ok() {
    dotask_sdk::bail!("Choose explicit source development or prebuilt doinstall tools, not both");
  }
  // Source development remains a deliberate choice. Ordinary packaging consumes
  // the real runtime/builder/template from one existing native tools bundle.
  let temporary = tempfile::Builder::new().prefix("dotask-installer-tools-").tempdir()?;
  let extension = if cfg!(windows) { ".exe" } else { "" };
  let tools = if let Ok(source) = project.string("source") {
    project.exec_target("_/doinstall/build", json!({"source":source,"stage-dir":temporary.path()}))?;
    project.exec_target("_/doinstall/build", json!({"source":source,"verify":true}))?;
    temporary.path().to_path_buf()
  } else if let Some(path) = project
    .string("doinstall-tools")
    .ok()
    .map(std::path::PathBuf::from)
    .or_else(|| std::env::var_os("DOINSTALL_TOOL_DIR").map(std::path::PathBuf::from))
  {
    project.path(path)
  } else {
    let paths: Value =
      serde_json::from_str(&project.capture(project.command("doinstall-builder").arg("--print-tool-paths"))?)?;
    let runtime = std::path::Path::new(paths["Installer"].as_str().context("Builder returned no Installer path")?);
    runtime.parent().context("Installer has no tools directory")?.to_path_buf()
  };
  let tools = std::fs::canonicalize(tools)?;
  for name in ["doinstall", "doinstall-builder", "doinstall-launcher"] {
    if !tools.join(format!("{name}{extension}")).is_file() {
      dotask_sdk::bail!("Missing matching prebuilt doinstall tool: {name}{extension}");
    }
  }
  project.exec_target("test", json!({"doinstall-tools":tools}))?;
  let binary = rust_build::inspected_binary(project)?;
  let identity: Value = serde_json::from_str(&project.capture(project.command(&binary).arg("__build-info"))?)?;
  // Use the exact inspected build's version and clock for package metadata.
  // Each local package gets a fresh external parent, without changing SemVer.
  let output_root = rust_build::target_directory(project)?.join("installers");
  std::fs::create_dir_all(&output_root)?;
  let output = if project.string("output").is_err() {
    Some(tempfile::Builder::new().prefix("dotask-").tempdir_in(output_root)?)
  } else {
    None
  };
  let destination = match project.string("output") {
    Ok(value) => project.path(value),
    Err(_) => output.as_ref().unwrap().path().to_path_buf(),
  };
  rust_build::external_directory(project, &destination)?;
  let parameters = json!({
    "builder":tools.join(format!("doinstall-builder{extension}")),
    "installer":tools.join(format!("doinstall{extension}")),
    "launcher-template":tools.join(format!("doinstall-launcher{extension}")),
    "app-version":identity["version"], "build-stamp":identity["stamp"],
    "output":destination
  });
  let artifact = project.create_installer("_/doinstall/create-installer", parameters)?;
  if let Some(output) = output {
    let _ = output.keep();
  }
  println!(
    "Installer: {}",
    artifact["FilePath"].as_str().context("Missing installer FilePath")?
  );
  project.set_installer_result(&artifact)
}

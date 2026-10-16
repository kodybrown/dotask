// dotask: 1
// description: Build and verify doinstall and dotask, then create dotask's installer.
// options:
//   - { name: source, type: path, description: Doinstall checkout; otherwise DOINSTALL_SOURCE or doinstall.source. }
//   - { name: app-version, description: Optional application version pattern override. }
//   - { name: build-stamp, description: Optional reproducible UTC YYDDD-HHMM build stamp. }
// requires:
//   - { kind: task, value: _/doinstall/build }
//   - { kind: task, value: verify }
//   - { kind: task, value: _/doinstall/create-installer }
// examples: [dotask create-dotask-installer, dotask create-dotask-installer --source ../doinstall]
// remarks: Checkout-local workflow. Runs both full verification gates before packaging; never installs the result.
// end-dotask
use dotask_sdk::{json, tempfile, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  // Keep build tools outside source and deliverables, and alive through every
  // verification and packaging step. Consumers still install doinstall separately.
  let temporary = tempfile::Builder::new().prefix("dotask-installer-tools-").tempdir()?;
  let tools = std::fs::canonicalize(temporary.path())?;
  let mut build = json!({"stage-dir":tools});
  let mut verify = json!({"verify":true});
  if let Ok(source) = project.string("source") {
    build["source"] = source.into();
    verify["source"] = source.into();
  }
  project.exec_target("_/doinstall/build", build)?;
  project.exec_target("_/doinstall/build", verify)?;
  // Pass the directory through invocation parameters so only this workflow's
  // verification uses the staged tools; no process-global environment mutation.
  project.exec_target("verify", json!({"doinstall-tools":tools}))?;
  let extension = if cfg!(windows) { ".exe" } else { "" };
  let mut parameters = json!({
    "builder":tools.join(format!("doinstall-builder{extension}")),
    "installer":tools.join(format!("doinstall{extension}"))
  });
  for name in ["app-version", "build-stamp"] {
    if let Ok(value) = project.string(name) {
      parameters[name] = value.into();
    }
  }
  let artifact = project.create_installer("_/doinstall/create-installer", parameters)?;
  project.set_installer_result(&artifact)
}

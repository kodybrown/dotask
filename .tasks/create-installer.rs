// dotask: 1
// description: Create the dotask installer using its configured build steps.
// options:
//   - { name: app-version, description: Optional application version pattern override. }
//   - { name: build-stamp, description: Optional reproducible UTC YYDDD-HHMM build stamp. }
//   - { name: builder, description: Separately supplied builder command/path; otherwise doinstall.builder or doinstall-builder. }
//   - { name: installer, type: path, description: Matching actual doinstall runtime path; otherwise doinstall.installer. }
// requires: [{ kind: task, value: _/doinstall/create-installer }]
// end-dotask
use dotask_sdk::{json, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let mut parameters = json!({});
  for name in ["app-version", "build-stamp", "builder"] {
    if let Ok(value) = project.string(name) {
      parameters[name] = value.into();
    }
  }
  let installer = project
    .string("installer")
    .ok()
    .or_else(|| project.setting("doinstall.installer").ok().and_then(dotask_sdk::Value::as_str))
    .ok_or_else(|| {
      dotask_sdk::anyhow!(
        "Supply separately installed doinstall tools: --installer PATH (the actual runtime, not a shim), or settings.doinstall.installer. Use --builder PATH if doinstall-builder is not on PATH."
      )
    })?;
  let installer = project.path(installer);
  // Fail before building dotask or creating deliverable directories.
  if !installer.is_file() {
    dotask_sdk::bail!("Doinstall runtime is missing: {}", installer.display());
  }
  parameters["installer"] = json!(installer);
  let artifact = project.create_installer("_/doinstall/create-installer", parameters)?;
  project.set_installer_result(&artifact)
}

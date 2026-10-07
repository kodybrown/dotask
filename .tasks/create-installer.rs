// dotask: 1
// description: Create the dotask installer using its configured build steps.
// options:
//   - { name: app-version, description: Optional application version pattern override. }
//   - { name: build-stamp, description: Optional reproducible UTC YYDDD-HHMM build stamp. }
// requires: [{ kind: task, value: _/doinstall/create-installer }]
// end-dotask
use dotask_sdk::{json, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let mut parameters = json!({});
  for name in ["app-version", "build-stamp"] {
    if let Ok(value) = project.string(name) {
      parameters[name] = value.into();
    }
  }
  let artifact = project.create_installer("_/doinstall/create-installer", parameters)?;
  project.set_installer_result(&artifact)
}

// dotask: 1
// description: Run the retained installer to uninstall an application without rebuilding it.
// options:
//   - { name: install-dir, type: path, required: true }
//   - { name: non-interactive, type: bool, default: false }
//   - { name: remove-settings, type: bool, default: false }
// end-dotask
use dotask_sdk::{BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let root = project.path(project.string("install-dir")?);
  let installer = project.installer_artifact(&root.join("installer").join(if cfg!(windows) {
    "installer.exe"
  } else {
    "installer"
  }))?;
  let mut arguments = vec![
    "uninstall".into(),
    "--install-dir".into(),
    root.to_string_lossy().into_owned(),
  ];
  if project.boolean("non-interactive")? {
    arguments.push("--non-interactive".into());
  }
  if project.boolean("remove-settings")? {
    arguments.push("--remove-settings".into());
  }
  project.run_installer(&installer, Some(&arguments))
}

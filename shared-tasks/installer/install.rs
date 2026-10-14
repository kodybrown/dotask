// dotask: 1
// description: Find and run the project's existing installer with optional guided configuration.
// options:
//   - { name: installer-args, description: JSON array of exact installer argument tokens; replaces defaults. }
// examples: [dotask install]
// remarks: Reads installer.installer_file or offers setup from saved builds/discovery. Never builds a target or installs tools.
// end-dotask
use dotask_sdk::{serde_json, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let arguments = project
    .string("installer-args")
    .ok()
    .map(serde_json::from_str::<Vec<String>>)
    .transpose()?;
  if arguments.as_ref().is_some_and(|args| args.iter().any(|arg| arg.contains('\0'))) {
    dotask_sdk::bail!("Installer arguments cannot contain NUL");
  }
  project.install(arguments.as_deref())?;
  println!("Installer exited with code 0.");
  Ok(())
}

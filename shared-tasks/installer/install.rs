// dotask: 1
// description: Create and run the project's installer for this OS and architecture.
// options:
//   - { name: installer-args, description: JSON array of exact installer argument tokens; replaces defaults. }
// examples: [dotask install]
// end-dotask
use dotask_sdk::{json, serde_json, BuildContext, Result};
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
  if !project.target_exists("create-installer")? {
    dotask_sdk::bail!("Installation requires a 'create-installer' target returning an installer artifact");
  }
  let installer = project.create_installer("create-installer", json!({}))?;
  println!("Running installer: {}", installer["FilePath"].as_str().unwrap());
  project.run_installer(&installer, arguments.as_deref())?;
  println!("Installer exited with code 0.");
  Ok(())
}

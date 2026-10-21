// dotask: 1
// description: Run the last successful local dotask installer; never builds or packages.
// options: [{name: installer-args, description: JSON string array replacing the installer's default arguments.}]
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
  // Local packages live outside source. The successful-build record provides
  // their exact path without guessing among equal-version review builds.
  if let Some(artifact) = project.find_installer()? {
    project.run_installer(&artifact, arguments.as_deref())
  } else {
    project.install(arguments.as_deref())
  }
}

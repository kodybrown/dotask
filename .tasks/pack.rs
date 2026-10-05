// dotask: 1
// description: Package the native dotask CLI with the existing standalone installer.
// options:
//   - { name: configuration, alias: c, choices: [Debug, Release], default: Release, description: C# helper build configuration. }
// end-dotask
use dotask_sdk::{json, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let artifact =
    project.create_installer("create-installer", json!({"configuration":project.string("configuration")?}))?;
  println!("{}", artifact["FilePath"].as_str().unwrap());
  Ok(())
}

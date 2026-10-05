//! ---
//! description: Package the native dotask CLI with the existing standalone installer.
//! options:
//!   - { name: configuration, alias: c, choices: [Debug, Release], default: Release, description: C# support build configuration. }
//!   - { name: self-contained, type: bool, default: true, description: Include the .NET runtime for C# metadata support. }
//! ---
use dotask_sdk::{json, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let artifact = project.create_installer(
    "create-installer",
    json!({"configuration":project.string("configuration")?,"self-contained":project.boolean("self-contained")?}),
  )?;
  println!("{}", artifact["FilePath"].as_str().unwrap());
  Ok(())
}

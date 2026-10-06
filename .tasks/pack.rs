// dotask: 1
// description: Package the native dotask CLI with the existing standalone installer.
// options:
//   - { name: configuration, alias: c, choices: [Debug, Release], default: Release, description: C# helper build configuration. }
//   - { name: app-version, description: 'Optional application version override.' }
//   - { name: build-stamp, description: 'Optional UTC YYDDD-HHMM stamp for reproducible builds.' }
//   - { name: git-hash, type: bool, default: true, description: Include the Git revision when available. }
// end-dotask
use dotask_sdk::{json, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let mut parameters =
    json!({"configuration":project.string("configuration")?,"git-hash":project.boolean("git-hash")?});
  if let Ok(version) = project.string("app-version") {
    parameters["app-version"] = version.into();
  }
  if let Ok(stamp) = project.string("build-stamp") {
    parameters["build-stamp"] = stamp.into();
  }
  let artifact = project.create_installer("create-installer", parameters)?;
  println!("{}", artifact["FilePath"].as_str().unwrap());
  Ok(())
}

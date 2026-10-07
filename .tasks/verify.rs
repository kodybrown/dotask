// dotask: 1
// description: Build and test dotask, then check formatting, documentation, and the shared catalog.
// options:
//   - { name: configuration, alias: c, choices: [Debug, Release], default: Release, description: Build configuration. }
// remarks: Every verification task is required. A missing task or failure stops verification.
// examples: [dotask verify, dotask verify -c Debug]
// end-dotask
use dotask_sdk::{json, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  project.exec_target("check", json!({}))?;
  project.exec_target("build-installer", json!({"verify":true}))?;
  project.exec_target("build", json!({"verify":true}))?;
  project.exec_target("test-sdks", json!({"configuration":project.string("configuration")?}))?;
  // project.exec_target("_/rust/test", json!({"configuration":project.string("configuration")?}))?;
  // project.exec_target("_/rust/format", json!({"verify":true}))?;
  project.exec_target("verify-docs", json!({}))?;
  project.exec_target("catalog", json!({"verify":true}))?;
  project.exec_target("shim", json!({"verify":true}))
}

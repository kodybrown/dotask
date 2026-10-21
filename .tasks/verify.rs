// dotask: 1
// description: Run dotask's extra SDK, integration, task-formatting, documentation and catalog checks after native tests.
// options:
//   - { name: configuration, alias: c, choices: [Debug, Release], default: Release, description: Build configuration. }
//   - { name: doinstall-tools, completion: directory, description: 'Prebuilt runtime, builder, and launcher template; otherwise DOINSTALL_TOOL_DIR.' }
// remarks: Called by test after the shared Rust gate. Every extra check is required; never call test here.
// examples: [dotask verify, dotask verify -c Debug]
// end-dotask
use dotask_sdk::{json, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  project.exec_target("check", json!({}))?;
  project.exec_target("stage-build", json!({"configuration":project.string("configuration")?}))?;
  project.exec_target("_/rust/cargo.rs", json!({"operation":"test","package":"dotask-sdk"}))?;
  project.exec_target("_/rust/clippy", json!({"package":"dotask-sdk"}))?;
  let mut tests = json!({"configuration":project.string("configuration")?});
  if let Ok(tools) = project.string("doinstall-tools")
    && !tools.is_empty()
  {
    tests["doinstall-tools"] = tools.into();
  }
  project.exec_target("test-sdks", tests)?;
  project.exec_target("format-tasks", json!({"check":true}))?;
  project.exec_target("verify-docs", json!({}))?;
  project.exec_target("catalog", json!({"verify":true}))?;
  Ok(())
}

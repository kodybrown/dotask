// dotask: 1
// description: Run dotask's extra SDK, integration, task-formatting, documentation and catalog checks after native tests.
// options:
//   - { name: configuration, alias: c, choices: [Debug, Release], default: Release, description: Build configuration. }
//   - { name: doinstall-tools, completion: directory, description: 'Prebuilt runtime, builder, and launcher template; otherwise DOINSTALL_TOOL_DIR.' }
// remarks: Called by test-all after the shared Rust gate. Every extra check is required; never call test or test-all here.
// examples: [dotask verify, dotask verify -c Debug]
// end-dotask
use dotask_sdk::{json, BuildContext, Result, Value};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  check(project, "check", json!({}))?;
  check(
    project,
    "stage-build",
    json!({"configuration":project.string("configuration")?}),
  )?;
  check(project, "_/rust/cargo.rs", json!({"operation":"test","package":"dotask-sdk"}))?;
  check(project, "_/rust/clippy", json!({"package":"dotask-sdk"}))?;
  check(
    project,
    "_/rust/cargo.rs",
    json!({"operation":"test","package":"dotask-release"}),
  )?;
  check(project, "_/rust/clippy", json!({"package":"dotask-release"}))?;
  let mut tests = json!({"configuration":project.string("configuration")?});
  if let Ok(tools) = project.string("doinstall-tools")
    && !tools.is_empty()
  {
    tests["doinstall-tools"] = tools.into();
  }
  check(
    project,
    "test-go",
    json!({"doinstall-tools":tests.get("doinstall-tools").cloned().unwrap_or_else(|| json!(""))}),
  )?;
  check(project, "test-sdks", tests)?;
  check(project, "format-tasks", json!({"check":true}))?;
  check(project, "verify-docs", json!({}))?;
  check(project, "catalog", json!({"verify":true}))?;
  Ok(())
}

fn check(project: &BuildContext, target: &str, parameters: Value) -> Result<()> {
  println!("==> Complete verification: {target}");
  let started = std::time::Instant::now();
  let result = project.exec_target(target, parameters);
  println!(
    "{target}: {} ({:.1}s)",
    if result.is_ok() { "passed" } else { "FAILED" },
    started.elapsed().as_secs_f64()
  );
  result
}

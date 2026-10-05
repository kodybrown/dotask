//! ---
//! description: Build and test dotask, then check formatting, documentation, and the shared catalog.
//! options:
//!   - { name: configuration, alias: c, choices: [Debug, Release], default: Release, description: Build configuration. }
//! remarks: Every verification task is required. A missing task or failure stops verification.
//! examples: [dotask verify, dotask verify -c Debug]
//! ---
use dotask_sdk::{json, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  project.exec_target("check", json!({}))?;
  project.exec_target("installer-engine", json!({"verify":true}))?;
  project.exec_target("rust-cli", json!({"verify":true}))?;
  project.exec_target("_/dotnet/test", json!({"configuration":project.string("configuration")?}))?;
  project.exec_target("_/dotnet/format", json!({"verify":true}))?;
  project.exec_target("verify-docs", json!({}))?;
  project.exec_target("catalog", json!({"verify":true}))?;
  project.exec_target("shim", json!({"verify":true}))
}

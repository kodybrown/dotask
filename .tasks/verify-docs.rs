//! ---
//! description: Check required documentation and Git whitespace errors.
//! requires:
//!   - { kind: task, value: git/check }
//! remarks: Checks required files, then calls _/git/check --whitespace; does not validate links or execute documentation examples.
//! ---
use dotask_sdk::{bail, json, BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  for document in [
    "README.md",
    "LICENSE.md",
    "AGENTS.md",
    "docs/README.md",
    "docs/USAGE.md",
    "docs/SHARED-TASKS.md",
    "docs/INSTALLATION.md",
    "docs/TARGETS.md",
    "docs/AI-ASSISTANTS.md",
    "docs/DESIGN.md",
    "docs/VERIFICATION.md",
    "docs/CHANGELOG.md",
    "examples/basic/README.md",
  ] {
    if !project.path(document).is_file() {
      bail!("Required document is missing: {document}");
    }
  }
  project.exec_target("_/git/check", json!({"whitespace":true}))?;
  println!("Required documentation exists and Git diffs have no whitespace errors.");
  Ok(())
}

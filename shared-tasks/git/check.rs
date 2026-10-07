// dotask: 1
// description: Check that Git is available, optionally checking repository whitespace.
// options:
//   - { name: whitespace, type: bool, default: false, description: Check staged and unstaged diffs for whitespace errors. }
// requires: [{ kind: tool, value: git }]
// examples: [dotask git/check, dotask git/check --whitespace]
// end-dotask
use dotask_sdk::{BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  println!("OK: {}", project.capture(project.command("git").arg("--version"))?.trim());
  if project.boolean("whitespace")? {
    project.execute(project.command("git").args(["diff", "--check"]))?;
    project.execute(project.command("git").args(["diff", "--cached", "--check"]))?;
    println!("Git diffs have no whitespace errors.");
  }
  Ok(())
}

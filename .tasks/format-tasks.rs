// dotask: 1
// description: Format or check dotask-owned Rust tasks without rewriting managed copies.
// options: [{name: check, type: bool, default: false}]
// requires: [{kind: tool, value: rustfmt}]
// end-dotask
use dotask_sdk::{BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let mut command = project.command("rustfmt");
  command.args(["--edition", "2024"]);
  if project.boolean("check")? {
    command.arg("--check");
  }
  // Include task entry/support files and the release driver's included source
  // fragment (Cargo fmt does not follow include!). Managed .tasks/_ copies
  // remain owned by the shared-task manager.
  for directory in [
    ".tasks",
    ".tasks/_support",
    ".tasks/_release/src",
    "shared-tasks/rust",
  ] {
    for entry in std::fs::read_dir(project.path(directory))? {
      let path = entry?.path();
      if path.extension().is_some_and(|extension| extension == "rs") {
        command.arg(path);
      }
    }
  }
  project.execute(&mut command)
}

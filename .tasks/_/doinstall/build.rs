// dotask: 1
// description: Build an explicitly selected doinstall source checkout using its own Cargo tooling.
// options:
//   - { name: source, type: path, description: Source checkout; otherwise doinstall.source. }
//   - { name: stage-dir, type: path, description: Copy both release tools here. }
//   - { name: verify, type: bool, default: false, description: Run doinstall's independent verification. }
// requires: [{ kind: tool, value: cargo }]
// end-dotask
use dotask_sdk::{BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let source = match project.string("source") {
    Ok(path) => project.path(path),
    Err(_) => std::env::var_os("DOINSTALL_SOURCE")
      .map(std::path::PathBuf::from)
      .map_or_else(|| project.setting_path("doinstall.source"), Ok)?,
  };
  let mut command = project.command("cargo");
  command
    .current_dir(&source)
    .args(["run", "--quiet", "--locked", "--manifest-path"])
    .arg(source.join("Cargo.toml"))
    .args(["--package", "doinstall-dev", "--"]);
  if project.boolean("verify")? {
    command.arg("verify");
  } else {
    command.arg("build");
    if let Ok(stage) = project.string("stage-dir") {
      command.arg("--stage-dir").arg(project.path(stage));
    }
  }
  project.execute(&mut command)
}

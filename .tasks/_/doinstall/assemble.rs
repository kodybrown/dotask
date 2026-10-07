// dotask: 1
// description: Assemble a distributable installer directory from resolved configuration and payload.
// options:
//   - { name: builder, type: path, required: true }
//   - { name: installer, type: path, required: true }
//   - { name: config, type: path, required: true }
//   - { name: output, type: path, required: true }
// end-dotask
use dotask_sdk::{BuildContext, Result};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  project.execute(
    project
      .command(project.path(project.string("builder")?))
      .arg("--installer")
      .arg(project.path(project.string("installer")?))
      .arg("--config")
      .arg(project.path(project.string("config")?))
      .arg("--output")
      .arg(project.path(project.string("output")?)),
  )
}

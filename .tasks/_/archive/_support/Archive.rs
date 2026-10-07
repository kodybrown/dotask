// Archive creation is explicit and fails rather than updating an existing file.
use dotask_sdk::{BuildContext, Result};
pub fn create(project: &BuildContext, format: &str) -> Result<()> {
  let source = std::fs::canonicalize(project.path(project.string("source")?))?;
  let output = project.path(project.string("output")?);
  if !source.is_dir() {
    dotask_sdk::bail!("Archive source must be a directory");
  }
  if output.exists() || output.starts_with(&source) {
    dotask_sdk::bail!("Archive output must be new and outside its source directory");
  }
  std::fs::create_dir_all(output.parent().unwrap())?;
  let parent = std::fs::canonicalize(output.parent().unwrap())?;
  if parent.starts_with(&source) {
    dotask_sdk::bail!("Archive output must be outside its source directory");
  }
  // Stage beside the destination so a failed archiver leaves no misleading
  // deliverable. A hard link publishes without replacing a concurrent output.
  let staging = dotask_sdk::tempfile::Builder::new()
    .prefix("dotask-archive-")
    .tempdir_in(parent)?;
  let archive = staging.path().join(format!("archive.{format}"));
  let name = source.file_name().unwrap();
  // Run from the parent and pass the directory name so both formats retain
  // the named top-level folder, including when the directory contains spaces.
  project.execute(
    project
      .command(project.string("archiver")?)
      .current_dir(source.parent().unwrap())
      .args(["a", &format!("-t{format}"), "-y"])
      .arg(&archive)
      .arg("--")
      .arg(name),
  )?;
  std::fs::hard_link(&archive, &output)?;
  Ok(())
}

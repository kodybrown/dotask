use crate::{
  configuration,
  project::{portable_path, Directory},
};
use anyhow::{bail, Result};
use std::{fs, io::Write, path::Path};

pub(crate) fn check_entry(path: &Path, directory: bool) -> Result<()> {
  match fs::symlink_metadata(path) {
    Ok(metadata) => {
      if is_link(&metadata) {
        bail!("--init does not follow symbolic links or reparse points: {}", path.display());
      }
      if metadata.is_dir() != directory {
        bail!(
          "Expected a {} at '{}'; the existing entry was left unchanged.",
          if directory { "directory" } else { "file" },
          path.display()
        );
      }
    }
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
    Err(e) => return Err(e.into()),
  }
  Ok(())
}
pub(crate) fn is_link(metadata: &fs::Metadata) -> bool {
  #[cfg(windows)]
  {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
  }
  #[cfg(unix)]
  {
    metadata.file_type().is_symlink()
  }
}
pub(crate) fn run(root: &Path, selected: Option<&str>, args: &[String]) -> Result<i32> {
  if !args.is_empty() {
    bail!(
      "Usage: dotask --init [--use-dir PATH]. Run it in the project directory; existing files are never overwritten."
    );
  }
  let tasks = portable_path(root, selected.unwrap_or(".tasks"))?;
  let relative = tasks
    .strip_prefix(root)
    .ok()
    .filter(|p| !p.as_os_str().is_empty())
    .ok_or_else(|| {
      anyhow::anyhow!("--init requires the task directory to be a subdirectory of the current project directory.")
    })?;
  let first = relative.components().next().unwrap().as_os_str().to_string_lossy();
  if [".dotasks.yaml", ".dotasks-lock.yaml"]
    .iter()
    .any(|n| first.eq_ignore_ascii_case(n))
  {
    bail!("The task directory cannot overlap .dotasks.yaml or .dotasks-lock.yaml.");
  }
  check_entry(root, true)?;
  let config = root.join(".dotasks.yaml");
  check_entry(&config, false)?;
  let mut current = root.to_path_buf();
  for part in relative.components() {
    current.push(part);
    check_entry(&current, true)?;
  }
  let legacy = tasks.join("config.yaml");
  if fs::symlink_metadata(&legacy).is_ok() {
    bail!(
      "Legacy configuration exists at '{}'. Review and move it to '{}' before using --init; no files were changed.",
      legacy.display(),
      config.display()
    );
  }
  configuration::load(&Directory {
    root_directory: root.into(),
    task_directory: tasks.clone(),
    invocation_directory: root.into(),
  })?;
  crate::process::check_cancelled()?;
  let mut created = false;
  if !config.exists() {
    let name = root.file_name().unwrap_or(root.as_os_str()).to_string_lossy();
    let content = format!(
      "version: 1\nname: {}\ndescription: ''\nsettings: {{}}\n",
      serde_json::to_string(&name)?
    );
    let mut temporary = tempfile::NamedTempFile::new_in(root)?;
    temporary.write_all(content.as_bytes())?;
    temporary.as_file().sync_all()?;
    // Persist without clobbering: a concurrent initializer/editor may win.
    match temporary.persist_noclobber(&config) {
      Ok(_) => created = true,
      Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => check_entry(&config, false)?,
      Err(e) => return Err(e.into()),
    }
  }
  let existed = tasks.is_dir();
  fs::create_dir_all(&tasks)?;
  check_entry(&tasks, true)?;
  let relative = relative.to_string_lossy().replace('\\', "/");
  println!(
    "{}",
    if created {
      "Created .dotasks.yaml."
    } else {
      "Kept existing .dotasks.yaml unchanged."
    }
  );
  println!(
    "{} task directory: ./{relative}/",
    if existed { "Kept existing" } else { "Created" }
  );
  println!("Project initialized. Edit .dotasks.yaml to set the project description and shared settings.");
  println!("Create your own tasks in ./{relative}/, or add shared tasks with dotask --add GROUP/TASK.");
  if selected.is_some() {
    println!(
      "Continue passing --use-dir {} to dotask commands; this override is not saved in the configuration.",
      serde_json::to_string(&relative)?
    );
  }
  println!("Commit .dotasks.yaml and your task files. Adding shared tasks will create .dotasks-lock.yaml.");
  Ok(0)
}

//! Staged process output is generic: the producing tool writes one file, then
//! dotask publishes it without replacing an existing or concurrently created file.
use anyhow::{bail, Context, Result};
use std::{
  fs,
  path::{Path, PathBuf},
};

pub(crate) struct FileOutput {
  pub staged: PathBuf,
  destination: PathBuf,
  // Own staging through command execution and publication, including failures.
  _directory: tempfile::TempDir,
}
impl FileOutput {
  pub fn prepare(destination: PathBuf, outside: Option<PathBuf>, suffix: &str) -> Result<Self> {
    let outside = outside.map(fs::canonicalize).transpose()?;
    if let Some(source) = &outside
      && !source.is_dir()
    {
      bail!("output-file outside-directory must be a directory: {}", source.display());
    }
    match fs::symlink_metadata(&destination) {
      Ok(_) => bail!("Process output must be new: {}", destination.display()),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(error) => return Err(error.into()),
    }
    let parent = destination.parent().context("Process output needs a parent directory")?;
    // Resolve the nearest existing ancestor before creating parents. This also
    // detects links leading inside the excluded source without modifying it.
    let mut existing = parent;
    let mut missing = vec![];
    loop {
      match fs::symlink_metadata(existing) {
        Ok(_) => break,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
          missing.push(existing.file_name().context("Cannot resolve process output parent")?);
          existing = existing.parent().context("Cannot resolve process output parent")?;
        }
        Err(error) => return Err(error.into()),
      }
    }
    let mut parent = fs::canonicalize(existing)?;
    if !parent.is_dir() {
      bail!("Process output parent must be a directory: {}", parent.display());
    }
    for part in missing.into_iter().rev() {
      parent.push(part);
    }
    if let Some(source) = &outside
      && within(&parent, source)
    {
      bail!("Process output must be outside its source directory: {}", destination.display());
    }
    fs::create_dir_all(&parent)?;
    let parent = fs::canonicalize(parent)?;
    if let Some(source) = &outside
      && within(&parent, source)
    {
      bail!("Process output must be outside its source directory: {}", destination.display());
    }
    let name = destination.file_name().context("Process output needs a filename")?;
    let destination = parent.join(name);
    let directory = tempfile::Builder::new().prefix("dotask-output-").tempdir_in(parent)?;
    let staged = directory.path().join(format!("output{suffix}"));
    Ok(Self {
      staged,
      destination,
      _directory: directory,
    })
  }
  pub fn publish(&self) -> Result<()> {
    let metadata = fs::symlink_metadata(&self.staged)
      .with_context(|| format!("Process succeeded without producing output: {}", self.staged.display()))?;
    if !metadata.is_file() || linked(&metadata) {
      bail!("Process output must be an ordinary file: {}", self.staged.display());
    }
    // Same-filesystem hard-link creation is atomic and refuses a race winner.
    // Dropping staging removes its link, leaving only the published file.
    fs::hard_link(&self.staged, &self.destination)
      .with_context(|| format!("Cannot publish new process output: {}", self.destination.display()))
  }
}
fn linked(metadata: &fs::Metadata) -> bool {
  #[cfg(windows)]
  {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
  }
  #[cfg(not(windows))]
  {
    metadata.file_type().is_symlink()
  }
}
fn within(path: &Path, directory: &Path) -> bool {
  #[cfg(windows)]
  {
    let mut path = path.components();
    directory.components().all(|expected| {
      path
        .next()
        .is_some_and(|actual| match (actual.as_os_str().to_str(), expected.as_os_str().to_str()) {
          (Some(actual), Some(expected)) => crate::configuration::same_key(actual, expected),
          _ => actual == expected,
        })
    })
  }
  #[cfg(not(windows))]
  {
    path.starts_with(directory)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn publication_keeps_contents_and_removes_staging() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("new/output.bin");
    let output = FileOutput::prepare(destination.clone(), None, ".bin").unwrap();
    let stage = output.staged.parent().unwrap().to_owned();
    fs::write(&output.staged, b"completed").unwrap();
    output.publish().unwrap();
    drop(output);
    assert_eq!(fs::read(destination).unwrap(), b"completed");
    assert!(!stage.exists());
  }

  #[test]
  fn racing_destination_is_never_replaced() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("output.bin");
    let output = FileOutput::prepare(destination.clone(), None, "").unwrap();
    let stage = output.staged.parent().unwrap().to_owned();
    fs::write(&output.staged, b"produced").unwrap();
    fs::write(&destination, b"race winner").unwrap();
    assert!(output.publish().is_err());
    drop(output);
    assert_eq!(fs::read(destination).unwrap(), b"race winner");
    assert!(!stage.exists());
  }

  #[test]
  fn existing_and_nested_outputs_fail_before_directory_creation() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    let existing = root.path().join("output.bin");
    fs::write(&existing, b"existing").unwrap();
    assert!(FileOutput::prepare(existing.clone(), Some(source.clone()), "").is_err());
    assert_eq!(fs::read(existing).unwrap(), b"existing");
    assert!(FileOutput::prepare(source.join("new/output.bin"), Some(source.clone()), "").is_err());
    assert!(!source.join("new").exists());
  }

  #[test]
  fn successful_command_must_produce_an_ordinary_file() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("output.bin");
    let output = FileOutput::prepare(destination.clone(), None, "").unwrap();
    assert!(output.publish().is_err());
    fs::create_dir(&output.staged).unwrap();
    assert!(output.publish().is_err());
    assert!(!destination.exists());
  }

  #[cfg(windows)]
  #[test]
  fn nested_output_cannot_bypass_boundary_using_case() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("Source");
    fs::create_dir(&source).unwrap();
    assert!(FileOutput::prepare(root.path().join("sOuRcE/new/output.bin"), Some(source.clone()), "").is_err());
    assert!(!source.join("new").exists());
  }

  #[cfg(unix)]
  #[test]
  fn linked_parent_cannot_write_into_source() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(&source, &alias).unwrap();
    assert!(FileOutput::prepare(alias.join("new/output.bin"), Some(source.clone()), "").is_err());
    assert!(!source.join("new").exists());
  }
}

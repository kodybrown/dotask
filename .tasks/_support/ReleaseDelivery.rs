use dotask_sdk::{
  bail,
  sha2::{Digest, Sha256},
  tempfile, BuildContext, Context, Result,
};
use std::{
  fs::{self, File, OpenOptions},
  io::{self, ErrorKind},
  path::{Path, PathBuf},
};

pub fn directory(project: &BuildContext) -> Result<PathBuf> {
  let mut path = fs::canonicalize(project.root())?;
  for part in [
    "artifacts",
    "releases",
    &format!("{}-{}", project.os(), project.architecture()),
  ] {
    path.push(part);
    if !check_path(&path, true)? {
      match fs::create_dir(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists && check_path(&path, true)? => {}
        Err(error) => return Err(error).with_context(|| format!("Cannot create {}", path.display())),
      }
    }
  }
  Ok(path)
}

pub fn deliver(project: &BuildContext, staged: &Path, installer: &Path, files: &[PathBuf]) -> Result<PathBuf> {
  let staged = fs::canonicalize(staged)?;
  if !files.iter().any(|file| file == installer) || !files.iter().all(|file| file.parent() == Some(staged.as_path())) {
    bail!("Release result does not describe the staged delivery");
  }
  let directory = directory(project)?;
  let _lock = delivery_lock(&directory)?;
  let mut pending = Vec::new();
  for file in files {
    let target = directory.join(file.file_name().context("Release file has no name")?);
    check_path(&target, false)?;
    let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
    let mut source = File::open(file)?;
    io::copy(&mut source, temporary.as_file_mut()).with_context(|| format!("Cannot copy {}", file.display()))?;
    #[cfg(unix)]
    temporary.as_file().set_permissions(source.metadata()?.permissions())?;
    pending.push((temporary, target));
  }
  // The driver lists checksums last. Publish them only after all other files.
  for (file, target) in pending {
    file
      .persist(&target)
      .with_context(|| format!("Cannot deliver {}", target.display()))?;
  }
  Ok(directory.join(installer.file_name().context("Installer has no name")?))
}

fn check_path(path: &Path, directory: bool) -> Result<bool> {
  let metadata = match fs::symlink_metadata(path) {
    Ok(metadata) => metadata,
    Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
    Err(error) => return Err(error.into()),
  };
  let linked = metadata.file_type().is_symlink();
  #[cfg(windows)]
  let linked = linked || {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
  };
  if linked
    || if directory {
      !metadata.is_dir()
    } else {
      !metadata.is_file()
    }
  {
    bail!(
      "Release delivery path is not an ordinary {}: {}",
      if directory { "directory" } else { "file" },
      path.display()
    );
  }
  Ok(true)
}

fn delivery_lock(directory: &Path) -> Result<File> {
  let locks = std::env::temp_dir().join("dotask-release-delivery-locks");
  fs::create_dir_all(&locks)?;
  let key = Sha256::digest(directory.to_string_lossy().to_lowercase().as_bytes());
  let lock = OpenOptions::new()
    .create(true)
    .truncate(false)
    .write(true)
    .open(locks.join(format!("{key:x}")))?;
  lock
    .try_lock()
    .context("Another installer build is delivering to this directory")?;
  Ok(lock)
}

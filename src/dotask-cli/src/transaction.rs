use crate::{project::Directory, shared_files as files};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{
  fs::{self, File, OpenOptions},
  path::PathBuf,
};

pub(crate) struct Change {
  pub path: String,
  pub before: Option<String>,
  pub after: Option<Vec<u8>>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
  version: i32,
  entries: Vec<Entry>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Entry {
  path: String,
  before_hash: Option<String>,
  after_hash: Option<String>,
  backup: String,
}
const OWNER: &str = "dotask shared-task transaction state v1\n";
pub(crate) struct Transaction<'a> {
  pub project: &'a Directory,
  pub prefix: String,
}
impl Transaction<'_> {
  fn state(&self) -> Result<PathBuf> {
    files::resolve(&self.project.root_directory, &format!("{}/.dotask", self.prefix))
  }
  fn staging(&self) -> Result<PathBuf> {
    files::resolve(&self.project.root_directory, &format!("{}/.dotask/transaction", self.prefix))
  }
  fn journal(&self) -> Result<PathBuf> {
    files::resolve(
      &self.project.root_directory,
      &format!("{}/.dotask/transaction/journal.json", self.prefix),
    )
  }
  pub fn pending(&self) -> Result<bool> {
    Ok(self.journal()?.is_file())
  }
  pub fn acquire(&self, cache: &std::path::Path) -> Result<File> {
    files::private_directory(cache)?;
    let identity = self.project.root_directory.to_string_lossy();
    let identity = if cfg!(target_os = "linux") {
      identity.into_owned()
    } else {
      identity.to_uppercase()
    };
    let path = files::resolve(cache, &format!(".operations/{}.lock", files::hash(identity.as_bytes())))?;
    fs::create_dir_all(path.parent().unwrap())?;
    for attempt in 0..=100 {
      crate::process::check_cancelled()?;
      let mut options = OpenOptions::new();
      options.read(true).write(true).create(true).truncate(false);
      #[cfg(windows)]
      {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
      }
      match options
        .open(&path)
        .and_then(|f| f.try_lock().map(|_| f).map_err(std::io::Error::from))
      {
        Ok(file) => return Ok(file),
        Err(e) if attempt == 100 => return Err(e.into()),
        Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
      }
    }
    unreachable!()
  }
  fn ensure_state(&self) -> Result<()> {
    let state = self.state()?;
    let marker = files::resolve(&state, "owner")?;
    if state.is_dir() {
      if fs::read(&marker).ok().as_deref() != Some(OWNER.as_bytes()) {
        bail!(
          "Refusing to use an unrecognized internal state directory: {}. No existing files will be deleted.",
          state.display()
        );
      }
    } else {
      fs::create_dir_all(&state)?;
      files::atomic(&marker, OWNER.as_bytes(), false)?;
    }
    Ok(())
  }
  fn destination(&self, path: &str) -> Result<PathBuf> {
    files::relative(path)?;
    if ![".dotasks.yaml", ".dotasks-lock.yaml"].contains(&path)
      && (!path.starts_with(&format!("{}/", self.prefix)) || path.starts_with(&format!("{}/.dotask/", self.prefix)))
    {
      bail!("Invalid shared-task transaction destination '{path}'.");
    }
    files::resolve(&self.project.root_directory, path)
  }
  fn validate(&self, journal: &Journal) -> Result<()> {
    if journal.version != 1 || journal.entries.is_empty() {
      bail!("Invalid shared-task transaction journal.");
    }
    let mut paths = std::collections::BTreeSet::new();
    for entry in &journal.entries {
      self.destination(&entry.path)?;
      if !paths.insert(entry.path.to_lowercase())
        || entry.before_hash.as_ref().is_some_and(|h| !files::is_hash(h))
        || entry.after_hash.as_ref().is_some_and(|h| !files::is_hash(h))
        || entry.backup.strip_suffix(".original").is_none_or(|s| s.parse::<u32>().is_err())
      {
        bail!("Invalid shared-task recovery entry.");
      }
    }
    Ok(())
  }
  pub fn recover(&self) -> Result<()> {
    if !self.pending()? {
      return Ok(());
    }
    self.ensure_state()?;
    let journal: Journal = serde_json::from_slice(&fs::read(self.journal()?)?)?;
    self.validate(&journal)?;
    let mut applied = vec![];
    for entry in &journal.entries {
      let path = self.destination(&entry.path)?;
      let current = files::hash_file(&path)?;
      if path.is_dir() || current != entry.before_hash && current != entry.after_hash {
        bail!("Interrupted shared-task operation needs manual recovery: '{}' changed afterward. Preserve the originals in {}; no files have been overwritten.",entry.path,self.staging()?.display());
      }
      let before = if let Some(hash) = &entry.before_hash {
        let bytes = fs::read(files::resolve(&self.staging()?, &entry.backup)?)?;
        if files::hash(&bytes) != *hash {
          bail!("Interrupted-operation backup failed verification. Project files have been left untouched.");
        }
        Some(bytes)
      } else {
        None
      };
      if current != entry.before_hash {
        applied.push((entry, before));
      }
    }
    for (entry, before) in applied.into_iter().rev() {
      let path = self.destination(&entry.path)?;
      if files::hash_file(&path)? != entry.after_hash {
        bail!("File changed during recovery: {}. Recovery data was retained.", entry.path);
      }
      if let Some(bytes) = before {
        files::atomic(&path, &bytes, true)?;
      } else {
        fs::remove_file(&path)?;
      }
    }
    fs::remove_file(self.journal()?)?;
    self.cleanup()
  }
  fn verify(&self, change: &Change) -> Result<()> {
    let path = self.destination(&change.path)?;
    if path.is_dir() || files::hash_file(&path)? != change.before {
      bail!(
        "Refusing to change '{}': it changed after the operation was planned.",
        change.path
      );
    }
    Ok(())
  }
  pub fn apply(&self, changes: &[Change]) -> Result<()> {
    if changes.is_empty() {
      return self.cleanup();
    }
    if self.pending()? {
      bail!("An interrupted shared-task operation must be recovered first.");
    }
    for change in changes {
      self.verify(change)?;
    }
    let journal = Journal {
      version: 1,
      entries: changes
        .iter()
        .enumerate()
        .map(|(i, c)| Entry {
          path: c.path.clone(),
          before_hash: c.before.clone(),
          after_hash: c.after.as_ref().map(|b| files::hash(b)),
          backup: format!("{i}.original"),
        })
        .collect(),
    };
    self.validate(&journal)?;
    self.cleanup()?;
    self.ensure_state()?;
    let staging = self.staging()?;
    if staging.exists() {
      bail!(
        "Unrecognized files remain in {}; they were preserved. Review them before retrying.",
        staging.display()
      );
    }
    fs::create_dir(&staging)?;
    let ignore = files::resolve(&self.state()?, ".gitignore")?;
    if !ignore.exists() {
      files::atomic(&ignore, b"*\n", false)?;
    }
    for (change, entry) in changes.iter().zip(&journal.entries) {
      if let Some(hash) = &change.before {
        let before = fs::read(self.destination(&change.path)?)?;
        if files::hash(&before) != *hash {
          bail!("File changed while preparing the update: {}", change.path);
        }
        files::atomic(&staging.join(&entry.backup), &before, false)?;
      }
    }
    files::atomic(
      &self.journal()?,
      files::dotnet_json(&serde_json::to_value(&journal)?).as_bytes(),
      false,
    )?;
    let applied = (|| -> Result<()> {
      for change in changes {
        crate::process::check_cancelled()?;
        self.verify(change)?;
        let path = self.destination(&change.path)?;
        if let Some(bytes) = &change.after {
          files::atomic(&path, bytes, change.before.is_some())?;
        } else if path.exists() {
          fs::remove_file(&path)?;
        }
      }
      // Removing the journal commits the task files and lockfile together.
      fs::remove_file(self.journal()?)?;
      Ok(())
    })();
    if applied.is_err() {
      self.recover()?;
    }
    applied?;
    self.cleanup()
  }
  fn cleanup(&self) -> Result<()> {
    if self.pending()? {
      return Ok(());
    }
    let state = self.state()?;
    if !state.is_dir() {
      return Ok(());
    }
    let marker = files::resolve(&state, "owner")?;
    if fs::read(&marker).ok().as_deref() != Some(OWNER.as_bytes()) {
      return Ok(());
    }
    let staging = self.staging()?;
    if staging.is_dir() {
      // Delete only recognized, owned staging files. Unknown entries and links
      // remain for review even after a successful recovery or no-op sync.
      for entry in fs::read_dir(&staging)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_file()
          && !crate::initialization::is_link(&metadata)
          && name
            .rsplit_once('.')
            .is_some_and(|(stem, ext)| stem.parse::<u32>().is_ok() && ["original", "new"].contains(&ext))
        {
          fs::remove_file(entry.path())?;
        }
      }
      if fs::read_dir(&staging)?.next().is_some() {
        return Ok(());
      }
      fs::remove_dir(&staging)?;
    }
    let ignore = files::resolve(&state, ".gitignore")?;
    for entry in fs::read_dir(&state)? {
      let path = entry?.path();
      if path != marker && path != ignore {
        return Ok(());
      }
    }
    if ignore.is_dir() || ignore.exists() && fs::read(&ignore)? != b"*\n" {
      return Ok(());
    }
    if ignore.exists() {
      fs::remove_file(ignore)?;
    }
    fs::remove_file(marker)?;
    fs::remove_dir(state)?;
    Ok(())
  }
}

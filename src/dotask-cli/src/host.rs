use anyhow::Result;
use std::{fs::OpenOptions, path::Path};
pub(crate) fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
  let mut options = OpenOptions::new();
  options.write(true).create_new(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  serde_json::to_writer(options.open(path)?, value)?;
  Ok(())
}

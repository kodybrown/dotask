use crate::{
  config,
  engine::{engine_name, fingerprint, identity, lock, verify_entries},
  files,
  model::*,
};
use anyhow::{ensure, Context, Result};
use std::{
  fs,
  path::{Path, PathBuf},
};

pub fn package(config_file: &Path, destination: &Path, named: bool, installer: &Path) -> Result<PathBuf> {
  let mut package: Package = files::read_yaml(config_file)?;
  config::validate(&package)?;
  let payload = files::absolute(Path::new(&package.payload), config_file.parent().unwrap())?;
  ensure!(
    !files::overlaps(destination, &payload),
    "Package output and payload must not overlap"
  );
  if !named {
    ensure!(
      !destination.exists(),
      "Package output already exists: {}",
      destination.display()
    );
  }
  // Producers can supply the application's own embedded build information.
  // Otherwise freeze a packaging stamp once; installation never creates one.
  if package.application.build.is_none() {
    package.application.build = Some(Build {
      name: None,
      stamp: crate::build_info::Stamp::now().text(),
      commit: None,
      dirty: false,
    });
  }
  let temporary = files::temporary()?;
  let staged_payload = temporary.path().join("payload");
  fs::create_dir(&staged_payload)?;
  if payload.is_dir() {
    let inventory = files::inventory(&payload)?;
    files::copy_payload(&payload, &staged_payload, &inventory)?;
  } else {
    files::extract(&payload, &staged_payload)?;
  }
  verify_entries(&package, &staged_payload)?;
  let inventory = files::inventory(&staged_payload)?;
  ensure!(!inventory.is_empty(), "Payload is empty");
  let identity = identity(&package, &inventory)?;
  let output = if named {
    destination.join(format!("{}-{}", package.application.id, identity))
  } else {
    destination.to_path_buf()
  };
  let _package_lock = lock(&output)?;
  if named && output.exists() {
    files::safe_path(&output)?;
    let existing: Package = files::read_yaml(&output.join("installer.yaml"))?;
    let existing_payload = files::absolute(Path::new(&existing.payload), &output)?;
    let existing_temporary = files::temporary()?;
    let existing_inventory = if existing_payload.is_dir() {
      files::inventory(&existing_payload)?
    } else {
      files::extract(&existing_payload, existing_temporary.path())?;
      files::inventory(existing_temporary.path())?
    };
    ensure!(
      fingerprint(&existing, &existing_inventory)? == fingerprint(&package, &inventory)?
        && files::file_hash(&output.join(engine_name()))? == files::file_hash(installer)?,
      "Package name already belongs to different contents: {}. Rebuild the application with a new build stamp.",
      output.display()
    );
    println!("Reused installer: {}", output.join(engine_name()).display());
    return Ok(output.join(engine_name()));
  }
  // Only the deliverable uses the source tree's artifact directory. Compiler
  // output remains external and the completed package is an independent copy.
  files::safe_path(&output)?;
  fs::create_dir_all(output.parent().context("Output needs a parent")?)?;
  let final_stage = tempfile::Builder::new()
    .prefix("installer-package-")
    .tempdir_in(output.parent().unwrap())?;
  if payload.is_dir() {
    let target = final_stage.path().join("payload");
    fs::create_dir(&target)?;
    files::copy_payload(&staged_payload, &target, &files::inventory(&staged_payload)?)?;
    package.payload = "payload".into();
  } else {
    fs::copy(&payload, final_stage.path().join("payload.zip"))?;
    package.payload = "payload.zip".into();
  }
  fs::copy(installer, final_stage.path().join(engine_name()))?;
  files::write_yaml(&final_stage.path().join("installer.yaml"), &package)?;
  fs::rename(final_stage.path(), &output)?;
  println!("Created installer: {}", output.join(engine_name()).display());
  Ok(output.join(engine_name()))
}

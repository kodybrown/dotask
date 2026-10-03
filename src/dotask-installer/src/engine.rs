use crate::{
  config::{self, Options},
  files, launchers,
  model::*,
};
use anyhow::{ensure, Context, Result};
use std::{
  fs,
  path::{Path, PathBuf},
};

fn engine_name() -> &'static str {
  if cfg!(windows) {
    "installer.exe"
  } else {
    "installer"
  }
}
pub fn package(config_file: &Path, output: &Path) -> Result<()> {
  let mut package: Package = files::read_yaml(config_file)?;
  config::validate(&package)?;
  let payload = files::absolute(Path::new(&package.payload), config_file.parent().unwrap())?;
  ensure!(
    !files::overlaps(output, &payload),
    "Package output and payload must not overlap"
  );
  ensure!(!output.exists(), "Package output already exists: {}", output.display());
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
  ensure!(!files::inventory(&staged_payload)?.is_empty(), "Payload is empty");
  // Only the deliverable uses the source tree's artifact directory. Compiler
  // output remains external and the completed package is an independent copy.
  files::safe_path(output)?;
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
  fs::copy(std::env::current_exe()?, final_stage.path().join(engine_name()))?;
  files::write_yaml(&final_stage.path().join("installer.yaml"), &package)?;
  fs::rename(final_stage.path(), output)?;
  println!("Created installer: {}", output.join(engine_name()).display());
  Ok(())
}
fn as_string(path: &Path) -> String {
  path.to_string_lossy().into()
}
fn lock(root: &Path) -> Result<fs::File> {
  // Locks live outside installations, so uninstall can remove the entire root.
  // Persistent lock files avoid unlink/reopen races between competing processes.
  let directory = fs::canonicalize(std::env::temp_dir())?.join("dotask-installer-locks");
  files::safe_path(&directory)?;
  fs::create_dir_all(&directory)?;
  let path = directory.join(files::hash(as_string(root).to_lowercase().as_bytes()));
  files::safe_path(&path)?;
  let file = fs::OpenOptions::new()
    .create(true)
    .truncate(false)
    .read(true)
    .write(true)
    .open(path)?;
  file.try_lock().context("Another installer is using this location")?;
  Ok(file)
}
fn load_receipt(root: &Path) -> Result<Receipt> {
  let path = root.join("installer/installation.yaml");
  files::safe_path(&path)?;
  let receipt: Receipt = files::read_yaml(&path)?;
  ensure!(
    receipt.schema == 1 && Path::new(&receipt.root) == root,
    "Installation receipt location/schema mismatch"
  );
  files::name(&receipt.app_id)?;
  files::name(&receipt.active)?;
  for build in receipt.builds.keys() {
    files::name(build)?;
  }
  ensure!(
    receipt.uninstalling || receipt.builds.contains_key(&receipt.active),
    "Active build is not recorded"
  );
  Ok(receipt)
}
fn read_build(root: &Path, id: &str, expected: &str, allow_missing: bool) -> Result<BuildReceipt> {
  files::name(id)?;
  let directory = root.join("app").join(id);
  files::safe_path(&directory)?;
  let path = directory.join("installation.yaml");
  ensure!(files::file_hash(&path)? == expected, "Build receipt changed: {id}");
  let receipt: BuildReceipt = files::read_yaml(&path)?;
  ensure!(receipt.schema == 1 && receipt.build == id, "Build receipt mismatch: {id}");
  ensure!(
    (allow_missing && !directory.join("installer.yaml").exists())
      || files::file_hash(&directory.join("installer.yaml"))? == receipt.config_hash,
    "Build configuration changed: {id}"
  );
  for (rel, file) in &receipt.files {
    files::relative(rel)?;
    ensure!(
      !matches!(rel.as_str(), "installer.yaml" | "installation.yaml"),
      "Reserved receipt filename"
    );
    let path = directory.join(rel);
    files::safe_path(&path)?;
    if allow_missing && !path.exists() {
      continue;
    }
    ensure!(
      path.is_file() && files::file_hash(&path)? == file.sha256,
      "Build file missing or modified: {}",
      path.display()
    );
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      ensure!(
        fs::metadata(&path)?.permissions().mode() & 0o777 == file.mode,
        "Build file permissions changed: {}",
        path.display()
      );
    }
  }
  Ok(receipt)
}
fn remove_build(root: &Path, id: &str, expected: &str, allow_missing: bool) -> Result<()> {
  let directory = root.join("app").join(id);
  if allow_missing && !directory.join("installation.yaml").exists() {
    return Ok(());
  }
  let receipt = read_build(root, id, expected, allow_missing)?;
  #[cfg(windows)]
  if !allow_missing {
    for rel in receipt.files.keys() {
      let path = directory.join(rel);
      fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .with_context(|| format!("Build is in use or not writable: {}", path.display()))?;
    }
  }
  for rel in receipt.files.keys() {
    let path = directory.join(rel);
    if path.exists() {
      fs::remove_file(path)?;
    }
  }
  // Only recorded files are removed. Unknown files survive, including files
  // created by an application that incorrectly writes beside its executable.
  for name in ["installer.yaml", "installation.yaml"] {
    let path = directory.join(name);
    if path.exists() {
      fs::remove_file(path)?;
    }
  }
  remove_empty_tree(&directory)?;
  Ok(())
}
fn remove_empty_tree(root: &Path) -> Result<()> {
  if !root.exists() {
    return Ok(());
  }
  files::safe_path(root)?;
  for item in fs::read_dir(root)? {
    let path = item?.path();
    if fs::symlink_metadata(&path)?.is_dir() && files::safe_path(&path).is_ok() {
      // Unknown links/junctions belong to the user. Never traverse them
      // merely to remove empty directories after owned files are gone.
      remove_empty_tree(&path)?;
    }
  }
  files::remove_empty(root)
}
fn verify_entries(package: &Package, payload: &Path) -> Result<()> {
  for executable in package
    .commands
    .iter()
    .map(|c| c.executable.as_str())
    .chain(package.shortcuts.iter().map(|s| s.executable.as_str()))
  {
    let path = payload.join(files::relative(executable)?);
    files::safe_path(&path)?;
    ensure!(
      path.is_file() || (cfg!(target_os = "macos") && path.is_dir() && path.extension().is_some_and(|x| x == "app")),
      "Missing executable: {executable}"
    );
    #[cfg(windows)]
    ensure!(
      path.extension().is_some_and(|s| s.eq_ignore_ascii_case("exe")),
      "Windows entry point must be .exe: {executable}"
    );
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      ensure!(
        fs::metadata(&path)?.permissions().mode() & 0o111 != 0,
        "Entry point is not executable: {executable}"
      );
    }
  }
  for shortcut in &package.shortcuts {
    if let Some(icon) = &shortcut.icon {
      ensure!(payload.join(icon).is_file(), "Missing icon: {icon}");
    }
    if let Some(dir) = &shortcut.working_directory {
      ensure!(payload.join(dir).is_dir(), "Missing working directory: {dir}");
    }
  }
  Ok(())
}
pub fn install(config_file: &Path, package: Package, options: &Options) -> Result<()> {
  config::validate(&package)?;
  let values = config::resolve(&package, options)?;
  let root = config::path(&values, "install-dir")?;
  ensure!(
    root.parent().is_some() && root.file_name().is_some(),
    "Cannot install at a filesystem root"
  );
  let bin = config::path(&values, "bin-dir")?;
  ensure!(!files::overlaps(&bin, &root), "bin-dir and install-dir must not overlap");
  let payload_source = files::absolute(Path::new(&package.payload), config_file.parent().unwrap())?;
  ensure!(
    !files::overlaps(&root, &payload_source),
    "Payload and installation must not overlap"
  );
  let extracted = files::temporary()?;
  let payload = if payload_source.is_dir() {
    payload_source.clone()
  } else {
    files::extract(&payload_source, extracted.path())?;
    extracted.path().to_path_buf()
  };
  verify_entries(&package, &payload)?;
  let inventory = files::inventory(&payload)?;
  ensure!(!inventory.is_empty(), "Payload is empty");
  let yaml = serde_saphyr::to_string(&package)?;
  let config_hash = files::hash(yaml.as_bytes());
  let fingerprint = files::hash(&serde_json::to_vec(&(&inventory, &config_hash))?);
  let id = format!("{}-{}", package.application.version, &fingerprint[..16]);
  files::name(&id)?;
  let directory = root.join("app").join(&id);
  let mut settings = Vec::new();
  for setting in &package.settings {
    let expanded = config::expand(setting, &values)?;
    let path = PathBuf::from(&expanded);
    files::safe_path(&path)?;
    ensure!(
      path.is_absolute() && path.parent().is_some() && path.file_name().is_some(),
      "Invalid settings path: {expanded}"
    );
    ensure!(
      !files::overlaps(&root, &path) && !files::overlaps(&bin, &path),
      "Settings must be separate from app/command directories"
    );
    for base in config::builtins()?.values() {
      ensure!(path != Path::new(base), "Settings cannot claim an entire user directory");
    }
    settings.push(expanded);
  }
  let prepared_dir = files::temporary()?;
  let prepared = launchers::prepare(&package, &root, &directory, &values, prepared_dir.path())?;
  // Validation is also the packaging smoke check; it performs no installation.
  if options.validate {
    println!(
      "Valid installer: {} {} ({id})",
      package.application.name, package.application.version
    );
    return Ok(());
  }
  let _root_lock = lock(&root)?;
  let _bin_lock = lock(&bin)?;
  let management = root.join("installer");
  let mut old = if root.exists() {
    Some(load_receipt(&root).context(
            "Existing directory has no valid installation.yaml receipt. Choose a fresh install-dir and an unused command location (bin-dir), or remove the previous installation yourself. Existing installations cannot be imported",
        )?)
  } else {
    None
  };
  if let Some(receipt) = &old {
    ensure!(!receipt.uninstalling, "Uninstall is incomplete; retry uninstall first");
    ensure!(
      receipt.app_id == package.application.id,
      "Application identity does not match existing installation"
    );
    ensure!(
      receipt.values.get("bin-dir") == values.get("bin-dir"),
      "Retain the original bin-dir when updating"
    );
    if !management.join("pending.yaml").exists() {
      files::verify_file(&management.join(engine_name()), &receipt.engine_hash)?;
      files::verify_file(&management.join("installer.yaml"), &receipt.config_hash)?;
    }
  }
  let pending_path = management.join("pending.yaml");
  if pending_path.exists() {
    let pending: Receipt = files::read_yaml(&pending_path)?;
    ensure!(
      pending.app_id == package.application.id && pending.root == as_string(&root) && pending.active == id,
      "An interrupted installation must be retried with its original package and locations"
    );
    let previous = old.as_ref().map(|r| r.launchers.clone()).unwrap_or_default();
    let mut owned = previous;
    for item in &pending.launchers {
      if fs::symlink_metadata(&item.path).is_ok() && launchers::record(Path::new(&item.path))? == *item {
        owned.retain(|o| o.path != item.path);
        owned.push(item.clone());
      }
    }
    let receipt = old.get_or_insert_with(|| pending.clone());
    receipt.launchers = owned;
    receipt.builds.extend(pending.builds);
    for (name, expected) in [
      (engine_name(), &pending.engine_hash),
      ("installer.yaml", &pending.config_hash),
    ] {
      let path = management.join(name);
      if path.exists() {
        let current = files::file_hash(&path)?;
        let before = if name == engine_name() {
          &receipt.engine_hash
        } else {
          &receipt.config_hash
        };
        ensure!(
          &current == expected || &current == before,
          "Interrupted installer file changed: {}",
          path.display()
        );
      }
    }
  }
  for item in &prepared {
    if fs::symlink_metadata(&item.destination).is_ok() {
      ensure!(
        old
          .as_ref()
          .is_some_and(|r| r.launchers.iter().any(|l| Path::new(&l.path) == item.destination)),
        "Unowned launcher exists: {}",
        item.destination.display()
      );
    }
  }
  if let Some(receipt) = &old {
    for launcher in &receipt.launchers {
      launchers::verify(launcher)?;
    }
  }
  fs::create_dir_all(&management)?;
  fs::create_dir_all(root.join("app"))?;
  let build_receipt = BuildReceipt {
    schema: 1,
    app_id: package.application.id.clone(),
    build: id.clone(),
    files: inventory.clone(),
    config_hash: config_hash.clone(),
  };
  let mut builds = old.as_ref().map(|r| r.builds.clone()).unwrap_or_default();
  if directory.exists() {
    let expected = builds.get(&id).context("Existing build is not owned")?;
    read_build(&root, &id, expected, false)?;
  } else {
    let stage = tempfile::Builder::new().prefix("staging-").tempdir_in(root.join("app"))?;
    files::copy_payload(&payload, stage.path(), &inventory)?;
    files::write_atomic(&stage.path().join("installer.yaml"), yaml.as_bytes())?;
    files::write_yaml(&stage.path().join("installation.yaml"), &build_receipt)?;
    fs::rename(stage.path(), &directory)?;
    builds.insert(id.clone(), files::file_hash(&directory.join("installation.yaml"))?);
  }
  let previous = old.as_ref().and_then(|r| {
    if r.active == id {
      r.previous.clone()
    } else {
      Some(r.active.clone())
    }
  });
  let engine = std::env::current_exe()?;
  if let Some(receipt) = &old {
    for setting in &receipt.settings {
      if !settings.contains(setting) {
        settings.push(setting.clone());
      }
    }
  }
  let mut receipt = Receipt {
    schema: 1,
    app_id: package.application.id.clone(),
    root: as_string(&root),
    active: id.clone(),
    previous,
    builds,
    launchers: vec![],
    settings,
    values,
    engine_hash: files::file_hash(&engine)?,
    config_hash,
    uninstalling: false,
  };
  for item in &prepared {
    let mut record = launchers::record(&item.source)?;
    record.path = as_string(&item.destination);
    receipt.launchers.push(record);
  }
  // A forward-completion record permits retrying the same package after an
  // interrupted activation without adopting unrelated launcher modifications.
  files::write_yaml(&pending_path, &receipt)?;
  if !management.join("installation.yaml").exists() {
    files::write_yaml(&management.join("installation.yaml"), &receipt)?;
  }
  files::write_atomic(&management.join(engine_name()), &fs::read(&engine)?)?;
  fs::set_permissions(management.join(engine_name()), fs::metadata(&engine)?.permissions())?;
  files::write_atomic(&management.join("installer.yaml"), yaml.as_bytes())?;
  receipt.launchers = launchers::apply(&prepared, &old.map(|r| r.launchers).unwrap_or_default())
    .context("Activate command launchers and shortcuts")?;
  files::write_yaml(&management.join("installation.yaml"), &receipt)?;
  fs::remove_file(pending_path)?;
  if config::enabled(&receipt.values, "prune-old-versions") {
    for (build, expected) in receipt.builds.clone() {
      if build == receipt.active || receipt.previous.as_ref() == Some(&build) {
        continue;
      }
      match remove_build(&root, &build, &expected, false) {
        Ok(()) => {
          receipt.builds.remove(&build);
          println!("Pruned {build}");
        }
        Err(e) => eprintln!("Retained {build}: {e:#}"),
      }
    }
    files::write_yaml(&management.join("installation.yaml"), &receipt)?;
  }
  println!(
    "Installed {} {}\n  Application: {}\n  Active build: {}",
    package.application.name,
    package.application.version,
    root.display(),
    id
  );
  Ok(())
}

pub fn uninstall(root: &Path, options: &Options) -> Result<()> {
  files::safe_path(root)?;
  // InstallerRunner starts in the executable's directory. On Windows that
  // working-directory handle would prevent removing installer/ itself.
  std::env::set_current_dir(fs::canonicalize(std::env::temp_dir())?)?;
  let _root_lock = lock(root)?;
  let management = root.join("installer");
  ensure!(
    !management.join("pending.yaml").exists(),
    "Installation is incomplete; retry its original installer before uninstalling"
  );
  let mut receipt = load_receipt(root)?;
  let bin = config::path(&receipt.values, "bin-dir")?;
  let _bin_lock = lock(&bin)?;
  files::verify_file(&management.join(engine_name()), &receipt.engine_hash)?;
  files::verify_file(&management.join("installer.yaml"), &receipt.config_hash)?;
  for launcher in &receipt.launchers {
    launchers::verify(launcher)?;
  }
  // Preflight every build before deleting any files. Configuration snapshots are
  // checked along with inventories; the latest YAML is not an uninstall recipe.
  for (build, expected) in &receipt.builds {
    if receipt.uninstalling && !root.join("app").join(build).join("installation.yaml").exists() {
      continue;
    }
    let record = read_build(root, build, expected, receipt.uninstalling)?;
    #[cfg(windows)]
    for path in record.files.keys().map(|p| root.join("app").join(build).join(p)) {
      if path.exists() {
        fs::OpenOptions::new()
          .write(true)
          .open(&path)
          .with_context(|| format!("Close applications using {} before uninstall", path.display()))?;
      }
    }
    #[cfg(not(windows))]
    let _ = record;
  }
  let mut remove_settings = options.remove_settings;
  if options.interactive {
    use std::io::Write;
    print!("Uninstall {} from {}? [false]: ", receipt.app_id, root.display());
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    ensure!(answer.trim() == "true", "Uninstall cancelled");
    if !options.settings_explicit {
      print!("Remove declared application settings? [false]: ");
      std::io::stdout().flush()?;
      answer.clear();
      std::io::stdin().read_line(&mut answer)?;
      remove_settings = answer.trim() == "true";
    }
  }
  if remove_settings {
    for setting in &receipt.settings {
      files::safe_path(Path::new(setting))?;
      if Path::new(setting).is_dir() {
        reject_links_tree(Path::new(setting))?;
      }
    }
  }
  receipt.uninstalling = true;
  files::write_yaml(&management.join("installation.yaml"), &receipt)?;
  for launcher in &receipt.launchers {
    if fs::symlink_metadata(&launcher.path).is_ok() {
      fs::remove_file(&launcher.path)?;
    }
  }
  receipt.launchers.clear();
  files::write_yaml(&management.join("installation.yaml"), &receipt)?;
  for (build, expected) in receipt.builds.clone() {
    remove_build(root, &build, &expected, true)?;
    receipt.builds.remove(&build);
    files::write_yaml(&management.join("installation.yaml"), &receipt)?;
  }
  if remove_settings {
    for setting in &receipt.settings {
      let path = Path::new(setting);
      if path.is_dir() {
        reject_links_tree(path)?;
        fs::remove_dir_all(path)?;
      } else if path.is_file() {
        fs::remove_file(path)?;
      }
    }
  }
  // Windows can rename a running executable out of its installation; its
  // temporary copy remains locked until process exit, never blocking app removal.
  let installed_engine = management.join(engine_name());
  if installed_engine.exists() {
    match fs::remove_file(&installed_engine) {
      Ok(()) => (),
      Err(_) if cfg!(windows) && std::env::current_exe()? == installed_engine => {
        let temporary = tempfile::Builder::new().prefix("dotask-uninstalled-").tempdir()?.keep();
        let moved = temporary.join(engine_name());
        fs::rename(&installed_engine, &moved)
          .context("Close the retained installer and uninstall using a package copy")?;
        eprintln!(
          "Temporary running uninstaller can be removed after exit: {}",
          temporary.display()
        );
      }
      Err(e) => return Err(e.into()),
    }
  }
  for name in ["installer.yaml", "installation.yaml"] {
    fs::remove_file(management.join(name))?;
  }
  files::remove_empty(&management)?;
  files::remove_empty(&root.join("app"))?;
  remove_empty_tree(root)?;
  if root.exists() {
    println!("Uninstalled {}; retained unowned files in {}", receipt.app_id, root.display());
  } else {
    println!("Uninstalled {}", receipt.app_id);
  }
  Ok(())
}
fn reject_links_tree(path: &Path) -> Result<()> {
  files::safe_path(path)?;
  for entry in fs::read_dir(path)? {
    let child = entry?.path();
    files::safe_path(&child)?;
    if child.is_dir() {
      reject_links_tree(&child)?;
    }
  }
  Ok(())
}

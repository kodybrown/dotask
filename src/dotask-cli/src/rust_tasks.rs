//! Rust task compilation uses private external manifests and snapshots,
//! isolated from consumer workspaces. Common headers live in task_metadata.
use crate::{host::write_json, process, project::Target};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

pub(crate) fn support(root: &Path, name: &str) -> Result<PathBuf> {
  if name.contains(['\\', ':']) || name.split('/').any(|p| matches!(p, "" | "." | "..")) {
    bail!("Support files must use a portable relative path inside the task directory: {name}");
  }
  let mut path = root.to_path_buf();
  for part in name.split('/') {
    path.push(part);
    if fs::symlink_metadata(&path).is_ok_and(|m| crate::initialization::is_link(&m)) {
      bail!("Rust support files cannot be symbolic links: {}", path.display());
    }
  }
  Ok(path)
}

pub(crate) fn compile(target: &Target, root: &Path, snapshot: &Path) -> Result<PathBuf> {
  if !crate::configuration::tool_exists("cargo", root) || !crate::configuration::tool_exists("rustc", root) {
    bail!("Rust tasks require Rust 1.95+ and Cargo. Install/select the toolchain and retry.");
  }
  let exe = std::env::current_exe()?;
  // Source bootstrapping supplies the Rust SDK before any task can prepare the
  // distribution. Installed commands use their own packaged SDK instead.
  let sdk = if crate::execution::source_sdk_stage()?.is_some() {
    PathBuf::from(std::env::var_os("DOTASK_SOURCE_RUST_SDK").context("Source bootstrap requires the Rust SDK path")?)
  } else {
    exe.parent().unwrap().join("sdk/rust")
  };
  if !sdk.join("Cargo.toml").is_file() {
    bail!("Bundled Rust SDK is missing. Build/stage the complete dotask CLI.");
  }
  let mut inputs = vec![(
    PathBuf::from("task").join(target.file_path.strip_prefix(root)?),
    fs::read(&target.file_path)?,
  )];
  for r in target.metadata["Requirements"]
    .as_array()
    .into_iter()
    .flatten()
    .filter(|r| r["Kind"] == "file")
  {
    let name = r["Value"].as_str().unwrap();
    let relative = support_name(target, root, name)?;
    let portable = relative.to_string_lossy().replace('\\', "/");
    inputs.push((
      PathBuf::from("task").join(&relative),
      fs::read(support(root, &portable)?)
        .with_context(|| format!("Required Rust support file is missing: {portable}"))?,
    ));
  }
  sdk_sources(&sdk, &sdk, &mut inputs)?;
  let mut hash = Sha256::new();
  // Include the source identity as well as bytes: distinct tasks with identical
  // contents still have separate package identities and safe Cargo locking.
  hash.update(b"dotask-rust-snapshot-v1");
  let identity = target.file_path.to_string_lossy();
  hash.update((identity.len() as u64).to_le_bytes());
  hash.update(identity.as_bytes());
  for (name, bytes) in &inputs {
    let name = name.to_string_lossy();
    hash.update((name.len() as u64).to_le_bytes());
    hash.update(name.as_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
  }
  let key = format!("{:x}", hash.finalize());
  let user = format!(
    "{:x}",
    Sha256::digest(std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default())
  );
  let cache = std::env::temp_dir().join("_rust/dotask/tasks").join(&user[..16]);
  let build = cache.join(&key);
  fs::create_dir_all(&build)?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&cache, fs::Permissions::from_mode(0o700))?;
  }
  for (name, bytes) in inputs {
    create_source(&build.join(name), &bytes)?;
  }
  let name = format!("task-{}", &key[..24]);
  let entry = PathBuf::from("task")
    .join(target.file_path.strip_prefix(root)?)
    .to_string_lossy()
    .replace('\\', "/");
  let manifest = format!("[package]\nname = {name:?}\nversion = \"0.0.0\"\nedition = \"2024\"\nrust-version = \"1.95\"\n[workspace]\n[dependencies]\ndotask-sdk = {{ path = \"sdk\" }}\n[[bin]]\nname = {name:?}\npath = {entry:?}\n");
  create_source(&build.join("Cargo.toml"), manifest.as_bytes())?;
  let output = cache.join("target");
  let mut command = Command::new("cargo");
  command
    .current_dir(&build)
    .args(["build", "--release", "--quiet", "--manifest-path"])
    .arg(build.join("Cargo.toml"))
    .env("CARGO_TARGET_DIR", &output)
    .env_remove("DOTASK_EXECUTION_CONTEXT");
  if cfg!(windows) {
    command.env("RUSTFLAGS", "-C target-feature=+crt-static");
  }
  let code = process::run(&mut command)?;
  if code != 0 {
    bail!("Compilation failed for '{}'.", target.name);
  }
  let binary = output.join("release").join(if cfg!(windows) {
    format!("{name}.exe")
  } else {
    name
  });
  let destination = snapshot.join(if cfg!(windows) { "task.exe" } else { "task" });
  fs::copy(binary, &destination)?;
  Ok(destination)
}
fn support_name(target: &Target, root: &Path, name: &str) -> Result<PathBuf> {
  // Catalog file requirements are group-relative (group/_support/file.rs).
  // Installed copies live under a source prefix, which must also be retained
  // in the immutable snapshot for Rust's relative module paths to resolve.
  let parent = target.file_path.strip_prefix(root)?.parent().unwrap();
  if let Some(group) = parent.file_name().and_then(|name| name.to_str())
    && name.starts_with(&format!("{group}/"))
  {
    return Ok(parent.parent().unwrap_or(Path::new("")).join(name));
  }
  Ok(name.into())
}
fn sdk_sources(root: &Path, directory: &Path, inputs: &mut Vec<(PathBuf, Vec<u8>)>) -> Result<()> {
  let mut entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
  entries.sort_by_key(|entry| entry.file_name());
  for entry in entries {
    let metadata = fs::symlink_metadata(entry.path())?;
    if crate::initialization::is_link(&metadata) {
      bail!("Rust SDK cannot contain links: {}", entry.path().display());
    }
    if metadata.is_dir() {
      sdk_sources(root, &entry.path(), inputs)?;
    } else if metadata.is_file() {
      inputs.push((
        PathBuf::from("sdk").join(entry.path().strip_prefix(root)?),
        fs::read(entry.path())?,
      ));
    }
  }
  Ok(())
}
fn create_source(path: &Path, bytes: &[u8]) -> Result<()> {
  fs::create_dir_all(path.parent().unwrap())?;
  // Content-addressed snapshots are immutable; concurrent identical invocations
  // may share them. Cargo owns compilation locking and outputs stay external.
  if path.is_file() {
    if fs::read(path)? != bytes {
      bail!("Rust task cache contents changed unexpectedly: {}", path.display());
    }
  } else {
    use std::io::Write;
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    temporary.write_all(bytes)?;
    match temporary.persist_noclobber(path) {
      Ok(_) => {}
      Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
        if fs::read(path)? != bytes {
          bail!("Rust task cache contents changed unexpectedly: {}", path.display());
        }
      }
      Err(error) => return Err(error.error.into()),
    }
  }
  Ok(())
}
pub(crate) fn batch(request: &Path, response: &Path) -> Result<()> {
  let value: Value = serde_json::from_slice(&fs::read(request)?)?;
  let root = Path::new(value["TaskDirectory"].as_str().context("Missing TaskDirectory")?);
  let files = value["Files"]
    .as_array()
    .context("Missing Files")?
    .iter()
    .map(|v| v.as_str().map(PathBuf::from).context("Invalid metadata filename"))
    .collect::<Result<Vec<_>>>()?;
  let targets: Vec<_> = files.iter().map(|p| crate::task_metadata::metadata(p, root)).collect();
  write_json(response, &targets)
}

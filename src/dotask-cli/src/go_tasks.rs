//! Offline Go task compilation. Consumer modules, workspaces and Go settings
//! never participate in the private task/SDK snapshot.
use crate::{
  process,
  project::Target,
  rust_tasks::{create_source, support, support_name},
};
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::{
  fs,
  path::{Path, PathBuf},
  process::{Command, Stdio},
};

const FLAGS: &[&str] = &["build", "-mod=readonly", "-trimpath", "-buildvcs=false"];
fn command(cache: &Path) -> Command {
  let mut command = Command::new("go");
  // GOENV=off also ignores persistent `go env -w` values. CGO is deliberately
  // disabled: code tasks require Go alone, not a hidden C compiler dependency.
  // Clear inherited Go settings as a family, including external cache helpers
  // (GOCACHEPROG) and future toolchain flags. Apply only our controlled values.
  for (name, _) in std::env::vars_os() {
    if name.to_string_lossy().to_ascii_uppercase().starts_with("GO") {
      command.env_remove(name);
    }
  }
  command
    .env("GOENV", "off")
    .env("GOTOOLCHAIN", "local")
    .env("GOWORK", "off")
    .env("GO111MODULE", "on")
    .env("GOFLAGS", "")
    .env("CGO_ENABLED", "0")
    .env("GOPROXY", "off")
    .env("GOSUMDB", "off")
    .env("GOFIPS140", "off")
    .env("GOTELEMETRY", "off")
    .env("GOCACHE", cache.join("objects"))
    .env("GOMODCACHE", cache.join("modules"))
    .env(
      "GOOS",
      match std::env::consts::OS {
        "macos" => "darwin",
        value => value,
      },
    )
    .env(
      "GOARCH",
      match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "x86" => "386",
        value => value,
      },
    )
    .env_remove("DOTASK_EXECUTION_CONTEXT");
  command
}
fn supported(version: &str) -> bool {
  let Some(version) = version.strip_prefix("go") else {
    return false;
  };
  let parts: Vec<_> = version.split('.').collect();
  parts.len() == 3
    && parts[0] == "1"
    && parts[1].parse::<u32>().is_ok_and(|v| v >= 22)
    && parts[2].parse::<u32>().is_ok()
}
pub(crate) fn compile(target: &Target, root: &Path, project: &Path, snapshot: &Path) -> Result<PathBuf> {
  if !crate::configuration::tool_exists("go", project) {
    bail!("Go tasks require an installed Go 1.22+ compiler. Install/select Go explicitly and retry; dotask never downloads toolchains.");
  }
  let user = format!(
    "{:x}",
    Sha256::digest(std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default())
  );
  let cache = std::env::temp_dir().join("_go/dotask/tasks").join(&user[..16]);
  // A consumer can override TEMP/TMPDIR. Refuse source-local caches rather than
  // silently generating manifests or build output in the consumer checkout.
  let project = fs::canonicalize(project)?;
  let mut ancestor = cache.as_path();
  while !ancestor.exists() {
    ancestor = ancestor.parent().context("Go cache has no existing ancestor")?;
  }
  let resolved = fs::canonicalize(ancestor)?.join(cache.strip_prefix(ancestor)?);
  if resolved.starts_with(&project) {
    bail!("Go task cache must be outside the consumer source tree: {}", resolved.display());
  }
  fs::create_dir_all(&resolved)?;
  let cache = fs::canonicalize(resolved)?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&cache, fs::Permissions::from_mode(0o700))?;
  }
  let environment = tempfile::NamedTempFile::new_in(&cache)?;
  let code = process::run(
    command(&cache)
      .current_dir(&cache)
      // Probe the compiler without searching ancestor modules in the temp tree.
      .env("GO111MODULE", "off")
      .args([
        "env",
        "-json",
        "GOVERSION",
        "GOROOT",
        "GOTOOLDIR",
        "GOOS",
        "GOARCH",
        "GOAMD64",
        "GOARM",
        "GOARM64",
        "GO386",
      ])
      .stdout(Stdio::from(environment.reopen()?)),
  )?;
  if code != 0 {
    bail!("Cannot inspect the installed Go compiler (exit {code}); Go 1.22+ is required. No toolchain was downloaded.");
  }
  let compiler = fs::read(environment.path())?;
  let info: serde_json::Value =
    serde_json::from_slice(&compiler).context("Installed go returned invalid compiler metadata")?;
  let version = info["GOVERSION"].as_str().unwrap_or_default();
  if !supported(version) {
    bail!("Unsupported Go compiler '{version}'; Go tasks require Go 1.22+ (stable). Select a supported installed compiler; dotask never downloads toolchains.");
  }
  let executable = std::env::current_exe()?;
  let sdk = if crate::execution::source_sdk_stage()?.is_some() {
    PathBuf::from(std::env::var_os("DOTASK_SOURCE_ROOT").context("Missing source bootstrap root")?)
      .join("src/dotask-go")
  } else {
    executable.parent().unwrap().join("sdk/go")
  };
  if !sdk.join("go.mod").is_file() {
    bail!("Bundled Go SDK is missing. Restore the complete dotask distribution (sdk/go).");
  }
  let entry = target.file_path.strip_prefix(root)?;
  let parent = entry.parent().unwrap();
  let relative = PathBuf::from(entry.file_name().unwrap()).with_extension("go");
  let mut inputs = vec![(PathBuf::from("task").join(&relative), fs::read(&target.file_path)?)];
  for requirement in target.metadata["Requirements"]
    .as_array()
    .into_iter()
    .flatten()
    .filter(|r| r["Kind"] == "file")
  {
    let name = support_name(target, root, requirement["Value"].as_str().unwrap())?;
    if name
      .components()
      .any(|c| matches!(c.as_os_str().to_str(), Some("go.mod" | "go.sum" | "go.work" | "go.work.sum")))
    {
      bail!(
        "Go task support must not supply consumer module/workspace manifests: {}",
        name.display()
      );
    }
    let portable = name.to_string_lossy().replace('\\', "/");
    inputs.push((
      PathBuf::from("task").join(name.strip_prefix(parent).unwrap_or(&name)),
      fs::read(support(root, &portable)?)
        .with_context(|| format!("Required Go support file is missing: {portable}"))?,
    ));
  }
  sdk_sources(&sdk, &sdk, &mut inputs)?;
  inputs.push((
    PathBuf::from("task/go.mod"),
    b"module dotask.local/task\n\ngo 1.22.0\n\nrequire dotask.local/sdk v0.0.0\nreplace dotask.local/sdk => ../sdk\n"
      .to_vec(),
  ));
  inputs.sort_by(|a, b| a.0.cmp(&b.0));
  inputs.dedup();
  // Imports are relative to the entry directory, so shared Go packages survive
  // source/group relocation. Task-root support outside that directory retains
  // its root-relative path. Refuse aliases that would overwrite different files.
  let mut destinations = std::collections::BTreeSet::new();
  for (name, _) in &inputs {
    if !destinations.insert(name.to_string_lossy().to_ascii_lowercase()) {
      bail!("Go support files map to conflicting snapshot paths: {}", name.display());
    }
  }
  let mut hash = Sha256::new();
  hash.update(
    b"dotask-go-snapshot-v4;GoEnvironment=controlled;CGO=0;GOENV=off;GOWORK=off;GOTOOLCHAIN=local;GOFIPS140=off",
  );
  hash.update(&compiler);
  for flag in FLAGS {
    hash.update(flag.as_bytes());
  }
  hash.update(target.file_path.to_string_lossy().as_bytes());
  for (name, bytes) in &inputs {
    let name = name.to_string_lossy();
    hash.update((name.len() as u64).to_le_bytes());
    hash.update(name.as_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
  }
  let build = cache.join(format!("{:x}", hash.finalize()));
  crate::shared_files::check_link(&build)?;
  fs::create_dir_all(&build)?;
  // Lock one immutable snapshot, but keep builds of different tasks independent.
  let lock = fs::OpenOptions::new()
    .create(true)
    .truncate(false)
    .read(true)
    .write(true)
    .open(build.join("build.lock"))?;
  loop {
    process::check_cancelled()?;
    match lock.try_lock() {
      Ok(()) => break,
      Err(fs::TryLockError::WouldBlock) => std::thread::sleep(std::time::Duration::from_millis(20)),
      Err(error) => return Err(error.into()),
    }
  }
  for (name, bytes) in inputs {
    create_source(&build.join(name), &bytes)?;
  }
  let binary = cached_binary(&build, std::env::consts::EXE_SUFFIX);
  if !binary.is_file() {
    // Publish only a successful build. Cancellation/failure leaves no usable
    // executable at the cache key; the next invocation safely retries.
    let temporary = tempfile::Builder::new().prefix("compile-").tempdir_in(&build)?;
    let output = temporary.path().join(binary.file_name().unwrap());
    let code = process::run(
      command(&cache)
        .current_dir(build.join("task"))
        .args(FLAGS)
        .arg("-o")
        .arg(&output)
        .arg(&relative),
    )?;
    if code != 0 {
      bail!("Go compilation failed for '{}' (exit {code}).", target.name);
    }
    fs::rename(output, &binary)?;
  }
  let destination = snapshot.join(binary.file_name().unwrap());
  fs::copy(&binary, &destination)?;
  Ok(destination)
}
// The source module already occupies build/task. Unix has no .exe suffix,
// so executable names must use a distinct namespace on every platform.
fn cached_binary(build: &Path, suffix: &str) -> PathBuf {
  build.join(format!("task-bin{suffix}"))
}
fn sdk_sources(root: &Path, directory: &Path, inputs: &mut Vec<(PathBuf, Vec<u8>)>) -> Result<()> {
  let mut entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
  entries.sort_by_key(|e| e.file_name());
  for entry in entries {
    let metadata = fs::symlink_metadata(entry.path())?;
    if crate::initialization::is_link(&metadata) {
      bail!("Go SDK cannot contain links: {}", entry.path().display());
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
#[cfg(test)]
mod tests {
  #[test]
  fn cached_executables_do_not_collide_with_source_directories_on_any_platform() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("task");
    std::fs::create_dir(&source).unwrap();
    for suffix in ["", ".exe"] {
      let output = super::cached_binary(root.path(), suffix);
      std::fs::write(&output, b"compiled executable").unwrap();
      assert!(source.is_dir());
      assert!(output.is_file());
      assert!(!output.starts_with(&source));
    }
  }
  #[test]
  fn installed_compiler_versions() {
    for version in ["go1.22.0", "go1.26.6", "go1.30.1"] {
      assert!(super::supported(version));
    }
    for version in ["go1.21.9", "go1.26rc1", "devel go1.27", "garbage"] {
      assert!(!super::supported(version));
    }
  }
}

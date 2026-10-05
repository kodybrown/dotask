//! Rust task metadata is a leading YAML block in crate documentation. Reading
//! it never invokes Cargo, rustc, or task code; compilation uses private external
//! manifests and snapshots, isolated from a consumer's Cargo workspace.
use crate::{catalog, host::write_json, process, project::Target};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Metadata {
  description: String,
  remarks: Option<String>,
  examples: Vec<String>,
  capabilities: Vec<String>,
  options: Vec<OptionMetadata>,
  requires: Vec<Requirement>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Requirement {
  kind: String,
  value: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionMetadata {
  name: String,
  #[serde(default)]
  alias: Option<String>,
  #[serde(default = "string_type", rename = "type")]
  kind: String,
  #[serde(default)]
  description: String,
  #[serde(default)]
  default: Option<Value>,
  #[serde(default)]
  required: bool,
  #[serde(default)]
  choices: Vec<String>,
  #[serde(default)]
  completion: Option<String>,
}
fn string_type() -> String {
  "string".into()
}

pub(crate) fn metadata(file: &Path, root: &Path) -> Target {
  let mut target = Target {
    name: file.file_stem().unwrap().to_string_lossy().into_owned(),
    file_path: file.into(),
    description: "(no description)".into(),
    short_name: None,
    error: None,
    group: None,
    options: vec![],
    metadata: serde_json::Map::new(),
  };
  let parsed = (|| -> Result<()> {
    (target.name, target.short_name) = catalog::read_name(file, root)?;
    let source = fs::read_to_string(file)?;
    let mut lines = source
      .trim_start_matches('\u{feff}')
      .lines()
      .skip_while(|l| l.trim().is_empty());
    let metadata = if lines.next().is_some_and(|l| l.trim() == "//! ---") {
      let mut yaml = String::new();
      let mut ended = false;
      for line in lines {
        let doc = line
          .strip_prefix("//!")
          .context("Rust task metadata must remain inside leading //! documentation")?;
        if doc.trim() == "---" {
          ended = true;
          break;
        }
        yaml.push_str(doc.strip_prefix(' ').unwrap_or(doc));
        yaml.push('\n');
      }
      if !ended {
        bail!("Unterminated Rust task metadata block.");
      }
      serde_json::from_value::<Metadata>(crate::yaml::parse(&yaml, &file.display().to_string())?)?
    } else {
      Metadata::default()
    };
    if !metadata.description.is_empty() {
      target.description = metadata.description;
    }
    let mut names = std::collections::BTreeSet::new();
    for option in metadata.options {
      for name in std::iter::once(&option.name).chain(option.alias.iter()) {
        if !catalog::identifier(name)
          || !names.insert(name.to_lowercase())
          || ["help", "h", "verbose", "version", "use-dir"].contains(&name.to_ascii_lowercase().as_str())
        {
          bail!("Invalid, reserved, or duplicate option '{name}'.");
        }
      }
      if !["string", "path", "bool", "int", "number"].contains(&option.kind.as_str()) {
        bail!("Unknown option type '{}'.", option.kind);
      }
      if option
        .completion
        .as_deref()
        .is_some_and(|c| !["file", "directory", "none"].contains(&c))
      {
        bail!("Invalid completion kind.");
      }
      let default = option
        .default
        .map(|v| match v {
          Value::String(s) => Ok(s),
          Value::Bool(_) | Value::Number(_) => Ok(v.to_string()),
          _ => Err(anyhow::anyhow!("Option defaults must be scalars.")),
        })
        .transpose()?;
      let value = json!({"Name":option.name,"Alias":option.alias,"Type":option.kind,"Description":option.description,"Default":default,"Required":option.required,"Choices":option.choices,"Completion":option.completion});
      if let Some(value_text) = value["Default"].as_str() {
        crate::configuration::convert(&value, value_text, root)?;
      }
      target.options.push(value);
    }
    let mut requirements = vec![];
    for requirement in metadata.requires {
      if !["tool", "setting", "task", "file", "os"].contains(&requirement.kind.as_str())
        || requirement.value.trim().is_empty()
      {
        bail!("Invalid task requirement.");
      }
      if requirement.kind == "file" {
        support(root, &requirement.value)?;
      }
      requirements.push(json!({"Kind":requirement.kind,"Value":requirement.value}));
    }
    target.metadata.extend([
      ("Requirements".into(), json!(requirements)),
      ("Remarks".into(), json!(metadata.remarks)),
      ("Examples".into(), json!(metadata.examples)),
      ("Capabilities".into(), json!(metadata.capabilities)),
    ]);
    Ok(())
  })();
  if let Err(error) = parsed {
    target.error = Some(format!("Metadata error in {}: {error:#}", file.display()));
  }
  target
}

fn support(root: &Path, name: &str) -> Result<PathBuf> {
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
  let exe = std::env::current_exe()?;
  let sdk = exe.parent().context("CLI has no parent directory")?.join("sdk");
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
    inputs.push((PathBuf::from("task").join(name), fs::read(support(root, name)?)?));
  }
  for name in ["Cargo.toml", "src/lib.rs"] {
    inputs.push((PathBuf::from("sdk").join(name), fs::read(sdk.join(name))?));
  }
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
  let csharp: Vec<_> = files
    .iter()
    .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cs")))
    .collect();
  let mut targets: Vec<Target> = if csharp.is_empty() {
    vec![]
  } else {
    crate::host::Host::locate()?.request(
      request.parent().unwrap(),
      json!({"Operation":"metadata","TaskDirectory":root,"Files":csharp}),
    )?
  };
  targets.extend(
    files
      .iter()
      .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("rs")))
      .map(|p| metadata(p, root)),
  );
  write_json(response, &targets)
}

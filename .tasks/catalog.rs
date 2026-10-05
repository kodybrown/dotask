//! ---
//! description: Generate the shared-task catalog, or verify that it matches the sources.
//! options:
//!   - { name: verify, alias: v, type: bool, default: false, description: Check the existing catalog without writing files. }
//!   - { name: root, type: path, default: shared-tasks, completion: directory, description: Directory containing the shared task groups. }
//! examples: [dotask catalog, dotask catalog --verify]
//! ---
use dotask_sdk::{
  bail, json, serde_json,
  sha2::{Digest, Sha256},
  BuildContext, Result,
};
use std::{
  collections::BTreeSet,
  fs,
  path::{Path, PathBuf},
};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let root = Path::new(project.string("root")?);
  if !root.is_dir() {
    bail!("Shared task directory is missing: {}", root.display());
  }
  reject_link(root)?;
  let mut sources = vec![];
  let mut groups = fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
  groups.sort_by_key(|e| e.file_name());
  for group in groups {
    if hidden(&group.path()) || !group.file_type()?.is_dir() {
      continue;
    }
    reject_link(&group.path())?;
    for entry in fs::read_dir(group.path())? {
      let source = entry?.path();
      if hidden(&source) || !source.extension().is_some_and(|e| e == "cs") {
        continue;
      }
      reject_link(&source)?;
      let manifest = source.with_extension("task.json");
      if manifest.is_file() {
        bail!("Move '{}' into XML <requires task=\"...\" /> / <requires file=\"...\" /> comments in '{}', then remove the .task.json file.", manifest.display(), source.display());
      }
      sources.push(source);
    }
  }
  sources.sort();
  let metadata = project.metadata(root, &sources)?;
  let mut tasks = vec![];
  for source in &sources {
    let relative = source.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
    let id = relative.strip_suffix(".cs").unwrap();
    if id.split('/').count() != 2 || !id.split('/').all(identifier) {
      bail!("Invalid task ID: {id}");
    }
    let metadata = metadata
      .as_array()
      .unwrap()
      .iter()
      .find(|m| Path::new(m["FilePath"].as_str().unwrap()) == source)
      .unwrap();
    if let Some(error) = metadata["Error"].as_str() {
      bail!("{relative}: {error}");
    }
    let requirements = metadata["Requirements"].as_array().unwrap();
    let mut files: BTreeSet<String> = requirements
      .iter()
      .filter(|r| r["Kind"] == "file")
      .map(|r| r["Value"].as_str().unwrap().into())
      .collect();
    files.insert(relative.clone());
    let mut records = vec![];
    for name in files {
      let path = resolve_file(root, &name)?;
      records.push(json!({"path":name,"sha256":format!("{:x}",Sha256::digest(fs::read(path)?))}));
    }
    let requires: Vec<_> = requirements
      .iter()
      .filter(|r| r["Kind"] == "task")
      .map(|r| r["Value"].clone())
      .collect();
    tasks.push(json!({"id":id,"entryPoint":relative,"description":metadata["Description"],"runtime":"csharp","files":records,"requires":requires}));
  }
  let mut ids = BTreeSet::new();
  for task in &tasks {
    let id = task["id"].as_str().unwrap();
    if !ids.insert(id.to_lowercase()) {
      bail!("Duplicate task ID: {id}");
    }
  }
  for task in &tasks {
    for required in task["requires"].as_array().unwrap() {
      if !ids.contains(&required.as_str().unwrap().to_lowercase()) {
        bail!(
          "{} requires missing task: {}",
          task["id"].as_str().unwrap(),
          required.as_str().unwrap()
        );
      }
    }
  }
  let content = serde_json::to_string_pretty(&json!({"version":1,"tasks":tasks}))? + "\n";
  let destination = root.join("catalog.json");
  if project.boolean("verify")? {
    if !destination.is_file() || fs::read(&destination)? != content.as_bytes() {
      bail!("catalog.json is stale. Run ./build.sh catalog (Windows: build.cmd catalog).");
    }
    println!("Shared task catalog is current.");
  } else {
    fs::write(&destination, content)?;
    println!("Wrote {}", destination.display());
  }
  Ok(())
}
fn hidden(path: &Path) -> bool {
  path.file_name().unwrap().to_string_lossy().starts_with(['.', '_'])
}
fn identifier(value: &str) -> bool {
  value.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
    && value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn resolve_file(root: &Path, name: &str) -> Result<PathBuf> {
  if name.contains(['\\', ':']) || name.split('/').any(|p| matches!(p, "" | "." | "..")) {
    bail!("File must be a portable relative path inside the source: {name}");
  }
  let mut path = root.to_path_buf();
  for part in name.split('/') {
    path.push(part);
    reject_link(&path)?;
  }
  Ok(path)
}
fn reject_link(path: &Path) -> Result<()> {
  let metadata = fs::symlink_metadata(path)?;
  #[cfg(windows)]
  {
    use std::os::windows::fs::MetadataExt;
    if metadata.file_attributes() & 0x400 != 0 {
      bail!("Shared catalog sources cannot be symbolic links: {}", path.display());
    }
  }
  if metadata.file_type().is_symlink() {
    bail!("Shared catalog sources cannot be symbolic links: {}", path.display());
  }
  Ok(())
}

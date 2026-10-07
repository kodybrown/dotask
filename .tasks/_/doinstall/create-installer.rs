// dotask: 1
// description: Run configured build steps and gather files to create an application installer.
// options:
//   - { name: config, type: path, description: Source installer YAML; otherwise installer.config. }
//   - { name: output, type: path, description: Package parent directory; otherwise installer.output. }
//   - { name: app-version, description: Override the application version pattern. }
//   - { name: build-stamp, description: Reproducible UTC YYDDD-HHMM build stamp. }
// requires: [{ kind: file, value: doinstall/_support/BuildInfo.rs }]
// end-dotask
#[path = "_support/BuildInfo.rs"]
mod build_info;
use dotask_sdk::{json, serde_json, serde_saphyr, tempfile, BuildContext, Context, Result, Value};
use std::{
  collections::BTreeMap,
  fs,
  path::{Component, Path, PathBuf},
};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let path = |option: &str, setting: &str| -> Result<PathBuf> {
    match project.string(option) {
      Ok(value) => Ok(project.path(value)),
      Err(_) => project.setting_path(setting),
    }
  };
  let source = path("config", "installer.config")?;
  let output = path("output", "installer.output")?.join(format!("{}-{}", project.os(), project.architecture()));
  let mut config: Value = serde_saphyr::from_str(&fs::read_to_string(&source)?)?;
  let mapping = config.as_object().context("Installer YAML must be a mapping")?;
  if config["schema"] != 1 {
    dotask_sdk::bail!("Source installer YAML requires schema: 1");
  }
  for key in mapping.keys() {
    if !["schema", "application", "build-steps", "runtime"].contains(&key.as_str()) {
      dotask_sdk::bail!("Unknown source installer field: {key}. Destination options belong under runtime.");
    }
  }
  let steps = config
    .as_object_mut()
    .context("Installer YAML must be a mapping")?
    .remove("build-steps")
    .context("Installer YAML requires build-steps")?;
  config
    .get("runtime")
    .context("Installer YAML requires runtime")?
    .as_object()
    .context("runtime must be a mapping")?;
  let id = config["application"]["id"]
    .as_str()
    .context("application.id is required")?
    .to_owned();
  if id.is_empty()
    || !id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    || id == "."
    || id == ".."
  {
    dotask_sdk::bail!("application.id must be a portable directory name");
  }
  let stamp = project
    .string("build-stamp")
    .ok()
    .map(build_info::Stamp::parse)
    .transpose()
    .map_err(|e| dotask_sdk::anyhow!(e))?
    .unwrap_or_else(build_info::Stamp::now);
  let pattern = project
    .string("app-version")
    .ok()
    .or_else(|| project.setting("app.version").ok().and_then(Value::as_str))
    .or_else(|| config["application"]["version"].as_str())
    .context("app.version or application.version is required")?;
  let version = stamp.format_version(pattern).map_err(|e| dotask_sdk::anyhow!(e))?;
  let (commit, dirty) = build_info::git(project.root());
  let mode = project.setting("app.git-hash").ok().and_then(Value::as_str).unwrap_or("short");
  let revision = match mode {
    "none" => "",
    "short" => &commit[..commit.len().min(7)],
    "full" => &commit,
    _ => dotask_sdk::bail!("app.git-hash must be none, short, or full"),
  };
  let suffix = project
    .setting("app.dirty-suffix")
    .ok()
    .and_then(Value::as_bool)
    .unwrap_or(true);
  let name = format!(
    "{}{}{}",
    build_info::version_component(&version).map_err(|e| dotask_sdk::anyhow!(e))?,
    if revision.is_empty() {
      String::new()
    } else {
      format!("-{revision}")
    },
    if dirty && suffix { "-dirty" } else { "" }
  );
  let directory = output.join(format!("{id}-{name}"));
  // A second build never changes the clock or reuses an earlier deliverable.
  // The user decides whether to change the version or remove an old build.
  if directory.exists() {
    dotask_sdk::bail!(
      "Installer version already exists: {}. Change app.version or remove that version explicitly.",
      directory.display()
    );
  }
  fs::create_dir_all(&output)?;
  // Persistent locks stay outside deliverables and avoid unlink/reopen races.
  let locks = fs::canonicalize(std::env::temp_dir())?.join("doinstall-build-locks");
  fs::create_dir_all(&locks)?;
  use dotask_sdk::sha2::{Digest, Sha256};
  let lock_path = locks.join(format!(
    "{:x}",
    Sha256::digest(output.to_string_lossy().to_lowercase().as_bytes())
  ));
  let lock = fs::OpenOptions::new()
    .create(true)
    .truncate(false)
    .read(true)
    .write(true)
    .open(lock_path)?;
  lock
    .try_lock()
    .context("Another installer creation is using this output directory")?;
  if directory.exists() {
    dotask_sdk::bail!("Installer version already exists: {}", directory.display());
  }
  let temporary = tempfile::Builder::new().prefix("doinstall-").tempdir()?;
  let staging = fs::canonicalize(temporary.path())?;
  let payload = staging.join("payload");
  fs::create_dir(&payload)?;
  let resolved = staging.join("installer.yaml");
  let build = json!({"version":version,"stamp":stamp.text(),"commit":if commit.is_empty(){None}else{Some(&commit)},"dirty":dirty,
    "git-hash":mode,"dirty-suffix":suffix});
  config["schema"] = 1.into();
  config["application"]["version"] = version.clone().into();
  config["application"]["build"] = json!({"name":name,"stamp":stamp.text(),"commit":build["commit"],"dirty":dirty});
  config["platform"] = project.os().into();
  config["architecture"] = project.architecture().into();
  config["payload"] = payload.to_string_lossy().into_owned().into();
  let mut variables = BTreeMap::from([
    ("staging".into(), json!(staging)),
    ("payload".into(), json!(payload)),
    ("config".into(), json!(resolved)),
    ("package".into(), json!(directory)),
    ("version".into(), json!(version)),
    ("build-info".into(), json!(serde_json::to_string(&build)?)),
    ("exe-extension".into(), json!(if cfg!(windows) { ".exe" } else { "" })),
  ]);
  settings("settings", project.settings(), &mut variables);
  for key in ["commands", "shortcuts"] {
    if let Some(entries) = config["runtime"][key].as_array_mut() {
      for entry in entries {
        if let Some(executable) = entry.get_mut("executable") {
          *executable = expand(executable, &variables)?;
        }
      }
    }
  }
  let steps = steps.as_array().context("build-steps must be a sequence")?;
  if steps.is_empty() {
    dotask_sdk::bail!("build-steps cannot be empty");
  }
  for step in steps {
    let mapping = step.as_object().context("Each build step must be a mapping")?;
    for key in mapping.keys() {
      if !["run", "with", "enabled", "gather"].contains(&key.as_str()) {
        dotask_sdk::bail!("Unknown build-step field: {key}");
      }
    }
    let enabled = step
      .get("enabled")
      .map(|value| expand(value, &variables))
      .transpose()?
      .map(|value| value.as_bool().context("build-step enabled must be boolean"))
      .transpose()?
      .unwrap_or(true);
    if !enabled {
      continue;
    }
    let step = expand(step, &variables)?;
    if step.get("run").is_none() && step.get("gather").is_none() {
      dotask_sdk::bail!("Each enabled build step requires run or gather");
    }
    if step.get("run").is_some_and(|run| !run.is_string()) {
      dotask_sdk::bail!("build-step run must be a task name");
    }
    if step.get("with").is_some_and(|value| !value.is_object()) {
      dotask_sdk::bail!("build-step with must be a mapping");
    }
    if let Some(target) = step["run"].as_str() {
      // The resolved config is refreshed before each task so final assembly
      // sees the payload gathered by every successful preceding step.
      fs::write(&resolved, serde_saphyr::to_string(&config)?)?;
      project.exec_target(target, step.get("with").cloned().unwrap_or_else(|| json!({})))?;
    }
    if let Some(items) = step.get("gather") {
      for item in items.as_array().context("gather must be a sequence")? {
        let from = project.path(item["from"].as_str().context("gather.from is required")?);
        // Output policies can intentionally use an aliased root (for example
        // C:\tmp). Resolve that declared root, then reject links within it.
        let from = fs::canonicalize(&from)
          .with_context(|| format!("Expected installer output directory is missing: {}", from.display()))?;
        let to = item["to"].as_str().unwrap_or(".");
        let destination = if to == "." {
          payload.clone()
        } else {
          payload.join(relative(to)?)
        };
        for file in item["files"].as_array().context("gather.files must be a sequence")? {
          let file = file.as_str().context("Expected file must be a string")?;
          if let Some(directory) = file.strip_suffix("/**") {
            let relative = relative(directory)?;
            tree(&from.join(&relative), &destination.join(relative))?;
          } else {
            let relative = relative(file)?;
            copy(&from.join(&relative), &destination.join(relative))?;
          }
        }
      }
    }
  }
  let artifact = project.installer_artifact(&directory.join(if cfg!(windows) {
    "installer.exe"
  } else {
    "installer"
  }))?;
  project.set_installer_result(&artifact)
}
fn settings(prefix: &str, value: &Value, variables: &mut BTreeMap<String, Value>) {
  if let Some(values) = value.as_object() {
    for (key, value) in values {
      settings(&format!("{prefix}.{key}"), value, variables);
    }
  } else {
    variables.insert(prefix.into(), value.clone());
  }
}
fn expand(value: &Value, variables: &BTreeMap<String, Value>) -> Result<Value> {
  Ok(match value {
    Value::String(text) => {
      if text.starts_with("${") && text.ends_with('}') && text.matches("${").count() == 1 {
        if let Some(value) = variables.get(&text[2..text.len() - 1]) {
          return Ok(value.clone());
        }
      }
      let mut text = text.clone();
      for (key, value) in variables {
        text = text.replace(&format!("${{{key}}}"), value.as_str().unwrap_or(""));
      }
      if text.contains("${") {
        dotask_sdk::bail!("Unresolved installer build value: {text}");
      }
      text.into()
    }
    Value::Array(items) => Value::Array(items.iter().map(|item| expand(item, variables)).collect::<Result<_>>()?),
    Value::Object(items) => Value::Object(
      items
        .iter()
        .map(|(key, value)| Ok((key.clone(), expand(value, variables)?)))
        .collect::<Result<_>>()?,
    ),
    other => other.clone(),
  })
}
fn relative(name: &str) -> Result<PathBuf> {
  let path = Path::new(name);
  if name.is_empty() || name.contains('\\') || path.components().any(|c| !matches!(c, Component::Normal(_))) {
    dotask_sdk::bail!("Expected portable relative payload path: {name}");
  }
  Ok(path.to_path_buf())
}
fn copy(source: &Path, destination: &Path) -> Result<()> {
  no_links(source)?;
  no_links(destination)?;
  let metadata = fs::symlink_metadata(source)
    .with_context(|| format!("Expected installer file is missing: {}", source.display()))?;
  if metadata.file_type().is_symlink() || !metadata.is_file() {
    dotask_sdk::bail!("Expected ordinary installer file: {}", source.display());
  }
  if destination.exists() {
    dotask_sdk::bail!("Installer payload collision: {}", destination.display());
  }
  fs::create_dir_all(destination.parent().unwrap())?;
  fs::copy(source, destination)?;
  Ok(())
}
fn tree(source: &Path, destination: &Path) -> Result<()> {
  no_links(source)?;
  let mut entries = fs::read_dir(source)?.collect::<std::io::Result<Vec<_>>>()?;
  entries.sort_by_key(|e| e.file_name());
  if entries.is_empty() {
    dotask_sdk::bail!("Expected installer directory is empty: {}", source.display());
  }
  for entry in entries {
    if entry.file_type()?.is_dir() {
      tree(&entry.path(), &destination.join(entry.file_name()))?;
    } else {
      copy(&entry.path(), &destination.join(entry.file_name()))?;
    }
  }
  Ok(())
}
fn no_links(path: &Path) -> Result<()> {
  for parent in path.ancestors() {
    let metadata = match fs::symlink_metadata(parent) {
      Ok(metadata) => metadata,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
      Err(error) => return Err(error.into()),
    };
    let mut linked = metadata.file_type().is_symlink();
    #[cfg(windows)]
    {
      use std::os::windows::fs::MetadataExt;
      linked |= metadata.file_attributes() & 0x400 != 0;
    }
    if linked {
      dotask_sdk::bail!("Installer file paths cannot contain links: {}", parent.display());
    }
  }
  Ok(())
}

use crate::project::{Catalog, Group, Step, Target};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{
  fs,
  path::{Path, PathBuf},
};

pub(crate) fn sources(root: &Path, task_root: bool, groups: bool) -> Result<Vec<PathBuf>> {
  let mut result = vec![];
  if !root.is_dir() {
    return Ok(result);
  }
  let mut entries = fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
  entries.sort_by_key(|e| e.file_name());
  for entry in entries {
    let name = entry.file_name().to_string_lossy().into_owned();
    let metadata = fs::symlink_metadata(entry.path())?;
    if name.starts_with('.')
      || (name.starts_with('_') && !(task_root && metadata.is_dir() && name == "_"))
      || crate::initialization::is_link(&metadata)
    {
      continue;
    }
    if metadata.is_dir() {
      if !["bin", "obj", "node_modules"].contains(&name.as_str()) {
        result.extend(sources(&entry.path(), false, groups)?);
      }
    } else if entry.path().extension().and_then(|e| e.to_str()).is_some_and(|e| {
      e.eq_ignore_ascii_case("cs") || (groups && (e.eq_ignore_ascii_case("rs") || e.eq_ignore_ascii_case("task")))
    }) {
      result.push(entry.path());
    }
  }
  Ok(result)
}
pub(crate) fn load(root: &Path, _session: &Path) -> Result<Catalog> {
  let files = sources(root, true, true)?;
  let mut targets: Vec<_> = files
    .iter()
    .map(|p| {
      if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("task")) {
        group(p, root)
      } else {
        crate::task_metadata::metadata(p, root)
      }
    })
    .collect();
  targets.sort_by(|a, b| {
    a.name
      .to_lowercase()
      .cmp(&b.name.to_lowercase())
      .then(a.file_path.cmp(&b.file_path))
  });
  let mut start = 0;
  while start < targets.len() {
    let mut end = start + 1;
    while end < targets.len() && targets[end].name.eq_ignore_ascii_case(&targets[start].name) {
      end += 1;
    }
    if end - start > 1 {
      let error = format!(
        "Ambiguous full target name '{}': {}.",
        targets[start].name,
        targets[start..end]
          .iter()
          .map(|t| t.file_path.strip_prefix(root).unwrap_or(&t.file_path).display().to_string())
          .collect::<Vec<_>>()
          .join(", ")
      );
      for target in &mut targets[start..end] {
        target.error = Some(error.clone());
      }
    }
    start = end;
  }
  Ok(Catalog {
    configuration: Value::Null,
    targets,
  })
}
pub(crate) fn identifier(value: &str) -> bool {
  value.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
    && value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
pub(crate) fn read_name(file: &Path, root: &Path) -> Result<(String, Option<String>)> {
  let stem = file.file_stem().unwrap().to_string_lossy();
  let parts: Vec<_> = stem.split(' ').collect();
  if parts.len() > 2 || parts.iter().any(|s| !identifier(s)) {
    bail!(
      "Invalid target filename '{}'. Expected a target name or '<group> <target>' with one space.",
      file.file_name().unwrap().to_string_lossy()
    );
  }
  let mut short = (parts.len() == 2).then(|| parts[1].to_owned());
  let leaf = parts.join("-");
  let relative = file.strip_prefix(root)?;
  let parents: Vec<_> = relative
    .parent()
    .unwrap()
    .components()
    .map(|p| p.as_os_str().to_string_lossy().into_owned())
    .collect();
  if parents.iter().enumerate().any(|(i, p)| !(identifier(p) || i == 0 && p == "_")) {
    bail!(
      "Invalid target directory in '{}'. Use letters, digits, underscores, or hyphens, starting with a letter.",
      relative.display()
    );
  }
  let name = if parents.is_empty() {
    leaf.clone()
  } else {
    short.get_or_insert(leaf.clone());
    format!("{}/{leaf}", parents.join("/"))
  };
  if ["help", "completion", "__complete", "__exec"]
    .iter()
    .any(|r| name.eq_ignore_ascii_case(r))
  {
    bail!("Invalid or reserved target name '{name}'.");
  }
  Ok((name, short))
}
fn group(file: &Path, root: &Path) -> Target {
  let mut target = Target {
    name: file.file_stem().unwrap().to_string_lossy().into_owned(),
    file_path: file.into(),
    description: String::new(),
    short_name: None,
    error: None,
    group: None,
    options: vec![],
    metadata: serde_json::Map::new(),
  };
  let parsed = (|| -> Result<()> {
    (target.name, target.short_name) = read_name(file, root)?;
    let value = crate::yaml::parse(&fs::read_to_string(file)?, &file.display().to_string())?;
    keys(&value, &["description", "require_at_least_1_step", "steps"])?;
    target.description = match value.get("description") {
      None => "(no description)".into(),
      Some(v) => text(v, "description")?.into(),
    };
    let required = flag(&value, "require_at_least_1_step")?;
    let steps = value["steps"]
      .as_array()
      .ok_or_else(|| anyhow::anyhow!("steps must be a sequence (use steps: [] for an empty group)."))?
      .iter()
      .map(|step| {
        keys(step, &["run", "optional", "with"])?;
        let run = text(
          step.get("run").ok_or_else(|| anyhow::anyhow!("Every step requires run."))?,
          "run",
        )?;
        if run.trim().is_empty() {
          bail!("run must name a target.");
        }
        let parameters = step.get("with").cloned().unwrap_or(json!({}));
        let object = parameters
          .as_object()
          .ok_or_else(|| anyhow::anyhow!("with must be a mapping of parameter names to values."))?;
        for (name, v) in object {
          if v.is_null() || v.is_array() || v.is_object() {
            bail!("with.{name} must be a string, boolean, or number.");
          }
        }
        Ok(Step {
          run: run.into(),
          optional: flag(step, "optional")?,
          parameters,
        })
      })
      .collect::<Result<Vec<_>>>()?;
    target.group = Some(Group {
      require_at_least_one_step: required,
      steps,
    });
    Ok(())
  })();
  if let Err(error) = parsed {
    target.error = Some(format!(
      "Metadata error in {}: {error}",
      file.file_name().unwrap().to_string_lossy()
    ));
  }
  target
}
fn keys(value: &Value, keys: &[&str]) -> Result<()> {
  for key in value
    .as_object()
    .ok_or_else(|| anyhow::anyhow!("Expected a YAML mapping."))?
    .keys()
  {
    if !keys.contains(&key.as_str()) {
      bail!("Unknown .task key '{key}'. Expected {}.", keys.join(", "));
    }
  }
  Ok(())
}
fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
  value.as_str().ok_or_else(|| anyhow::anyhow!("{name} must be a string."))
}
fn flag(value: &Value, name: &str) -> Result<bool> {
  match value.get(name) {
    None => Ok(false),
    Some(v) => v.as_bool().ok_or_else(|| anyhow::anyhow!("{name} must be a boolean.")),
  }
}

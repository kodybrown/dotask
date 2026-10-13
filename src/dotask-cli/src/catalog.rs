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
      let extensions: std::collections::BTreeSet<_> = targets[start..end]
        .iter()
        .map(|t| t.file_path.extension().unwrap().to_string_lossy().to_ascii_lowercase())
        .collect();
      if extensions.len() == end - start {
        for target in &mut targets[start..end] {
          let extension = target.file_path.extension().unwrap().to_string_lossy();
          target.name = format!("{}.{extension}", target.name);
          target.short_name = target.short_name.as_ref().map(|s| format!("{s}.{extension}"));
        }
      } else {
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
pub(crate) fn group(file: &Path, root: &Path) -> Target {
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
    keys(
      &value,
      &[
        "description",
        "remarks",
        "examples",
        "capabilities",
        "options",
        "requires",
        "require_at_least_1_step",
        "steps",
      ],
    )?;
    let mut metadata = value.as_object().unwrap().clone();
    metadata.remove("steps");
    metadata.remove("require_at_least_1_step");
    crate::task_metadata::apply_value(&mut target, Value::Object(metadata), root)?;
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
        keys(step, &["run", "exec", "optional", "with", "when"])?;
        if step.get("run").is_some() == step.get("exec").is_some() {
          bail!("Every step requires exactly one run or exec action.");
        }
        let when = if let Some(condition) = step.get("when") {
          keys(condition, &["parameter"])?;
          let name = text(
            condition
              .get("parameter")
              .ok_or_else(|| anyhow::anyhow!("when needs a boolean parameter reference."))?,
            "parameter",
          )?;
          if crate::configuration::option(&target, name).is_none_or(|o| o["Type"] != "bool") {
            bail!("when must reference a declared boolean parameter: {name}");
          }
          Some(
            crate::configuration::option(&target, name).unwrap()["Name"]
              .as_str()
              .unwrap()
              .to_owned(),
          )
        } else {
          None
        };
        let run = if let Some(run) = step.get("run") {
          let run = text(run, "run")?;
          if run.trim().is_empty() {
            bail!("run must name a target.");
          }
          run
        } else {
          ""
        };
        let exec = if let Some(command) = step.get("exec") {
          if step.get("optional").is_some() || step.get("with").is_some() {
            bail!("exec steps cannot declare optional or with.");
          }
          keys(command, &["executable", "arguments", "cwd", "output-file"])?;
          let output_file = if let Some(output) = command.get("output-file") {
            keys(output, &["path", "outside-directory", "suffix"])?;
            let path = output
              .get("path")
              .ok_or_else(|| anyhow::anyhow!("output-file requires path."))?
              .clone();
            crate::process_steps::validate(&path, &target)?;
            if path.as_str().is_some_and(|s| s.trim().is_empty()) {
              bail!("output-file path must be nonempty.");
            }
            let outside_directory = output.get("outside-directory").cloned();
            if let Some(outside) = &outside_directory {
              crate::process_steps::validate(outside, &target)?;
              if outside.as_str().is_some_and(|s| s.trim().is_empty()) {
                bail!("outside-directory must be nonempty.");
              }
            }
            let suffix = output.get("suffix").map(|v| text(v, "suffix")).transpose()?.unwrap_or("");
            if !suffix
              .bytes()
              .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
            {
              bail!("output-file suffix must use portable filename characters: letters, digits, '.', '-' or '_'.");
            }
            Some(crate::project::ProcessOutputFile {
              path,
              outside_directory,
              suffix: suffix.into(),
            })
          } else {
            None
          };
          let executable = command
            .get("executable")
            .ok_or_else(|| anyhow::anyhow!("exec requires executable."))?
            .clone();
          crate::process_steps::validate(&executable, &target)?;
          if executable.as_str().is_some_and(|s| s.trim().is_empty()) {
            bail!("Executable must be nonempty.");
          }
          let arguments = command.get("arguments").cloned().unwrap_or(json!([]));
          let arguments = arguments
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("arguments must be a sequence."))?
            .clone();
          for argument in &arguments {
            crate::process_steps::validate_argument(argument, &target, output_file.is_some())?;
          }
          if output_file.is_some() && !arguments.iter().any(|v| v.get("output-file") == Some(&json!(true))) {
            bail!("An output-file process requires an argument containing {{output-file: true}}.");
          }
          let cwd = command.get("cwd").cloned().unwrap_or(json!("."));
          crate::process_steps::validate(&cwd, &target)?;
          if cwd.as_str().is_some_and(|s| s.trim().is_empty()) {
            bail!("cwd must be nonempty.");
          }
          Some(crate::project::ProcessStep {
            executable,
            arguments,
            cwd,
            output_file,
          })
        } else {
          None
        };
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
          exec,
          when,
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

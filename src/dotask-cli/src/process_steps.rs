//! Declarative values are literals, exact references, or bounded path operations,
//! never shell text or an expression language. One value remains one argument.
use crate::{
  configuration, process,
  project::{portable_path, Directory, ProcessStep, Target},
};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{ffi::OsString, path::PathBuf, process::Command};

pub(crate) fn validate(value: &Value, target: &Target) -> Result<()> {
  validate_value(value, target, false, 0)
}
pub(crate) fn validate_argument(value: &Value, target: &Target, output: bool) -> Result<()> {
  validate_value(value, target, output, 0)
}
fn validate_value(value: &Value, target: &Target, output: bool, depth: usize) -> Result<()> {
  if depth > 16 {
    bail!("Process path references cannot nest more than 16 levels.");
  }
  if let Some(text) = value.as_str() {
    if text.contains('\0') {
      bail!("Process values cannot contain NUL.");
    }
    return Ok(());
  }
  let reference = value
    .as_object()
    .ok_or_else(|| anyhow::anyhow!("Process values must be strings or exact references."))?;
  if reference.len() != 1 {
    bail!("A process reference must have exactly one key.");
  }
  let (kind, value) = reference.iter().next().unwrap();
  match kind.as_str() {
    "path-parent" | "path-name" => return validate_value(value, target, false, depth + 1),
    "output-file" if output && value == &Value::Bool(true) => return Ok(()),
    "output-file" => bail!("{{output-file: true}} is only valid as an argument of a process declaring output-file."),
    _ => {}
  }
  let name = value
    .as_str()
    .filter(|n| !n.trim().is_empty())
    .ok_or_else(|| anyhow::anyhow!("A reference needs a nonempty name."))?;
  match kind.as_str() {
    "parameter"
      if target
        .options
        .iter()
        .any(|o| o["Name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(name))) =>
    {
      Ok(())
    }
    "setting" | "result" | "environment" | "item" => Ok(()),
    _ => bail!("Unknown process reference {kind}: {name}."),
  }
}
pub(crate) fn resolve(value: &Value, directory: &Directory, parameters: &Value, settings: &Value) -> Result<OsString> {
  let resolved = if !value.is_object() {
    value
  } else {
    let (kind, value) = value.as_object().unwrap().iter().next().unwrap();
    if matches!(kind.as_str(), "path-parent" | "path-name") {
      let path = resolve_path(resolve(value, directory, parameters, settings)?, directory)?;
      // Canonicalize existing inputs so aliases use the same parent and name.
      // Keep OS strings intact, including Unix filenames not representable in UTF-8.
      let path =
        std::fs::canonicalize(&path).with_context(|| format!("Cannot resolve process path: {}", path.display()))?;
      let component = if kind == "path-parent" {
        path.parent()
      } else {
        path.file_name().map(std::path::Path::new)
      };
      return component
        .map(|p| p.as_os_str().to_owned())
        .ok_or_else(|| anyhow::anyhow!("Process path has no {kind}: {}", path.display()));
    }
    let name = value.as_str().unwrap();
    if kind == "parameter" {
      configuration::get(parameters, name)
    } else {
      configuration::find(settings, name)
    }
    .ok_or_else(|| anyhow::anyhow!("Missing process reference {kind}: {name}."))?
  };
  let text = match resolved {
    Value::String(s) => s.clone(),
    Value::Bool(_) | Value::Number(_) => resolved.to_string(),
    _ => bail!("A process reference must resolve to a string, boolean or number."),
  };
  if text.contains('\0') {
    bail!("Process values cannot contain NUL.");
  }
  Ok(text.into())
}
pub(crate) fn resolve_path(value: OsString, directory: &Directory) -> Result<PathBuf> {
  if value.is_empty() || value.to_str().is_some_and(|s| s.trim().is_empty()) {
    bail!("Process paths must be nonempty.");
  }
  match value.to_str() {
    Some(text) => portable_path(&directory.root_directory, text),
    // Non-UTF-8 values can only originate from canonical path references.
    None => Ok(std::path::absolute(directory.root_directory.join(value))?),
  }
}
pub(crate) fn execute(
  step: &ProcessStep,
  directory: &Directory,
  parameters: &Value,
  settings: &Value,
) -> Result<(i32, Value)> {
  let executable = resolve(&step.executable, directory, parameters, settings)?;
  if executable.is_empty() || executable.to_str().is_some_and(|s| s.trim().is_empty()) {
    bail!("Executable must be nonempty.");
  }
  let cwd = resolve_path(resolve(&step.cwd, directory, parameters, settings)?, directory)?;
  if !cwd.is_dir() {
    bail!("Process cwd is not a directory: {}", cwd.display());
  }
  // Reject invalid executable paths before preparing any output directories.
  let executable = if std::path::Path::new(&executable).is_absolute()
    || executable.to_str().is_some_and(|s| s.contains(['/', '\\']))
  {
    resolve_path(executable, directory)?.into_os_string()
  } else {
    executable
  };
  // Resolve all ordinary arguments before preparing output directories. A bad
  // setting/path must not leave staging or run a command with partial parameters.
  let mut arguments = step
    .arguments
    .iter()
    .map(|value| {
      if value.get("output-file") == Some(&Value::Bool(true)) {
        Ok(None)
      } else {
        resolve(value, directory, parameters, settings).map(Some)
      }
    })
    .collect::<Result<Vec<_>>>()?;
  let environment = step
    .environment
    .iter()
    .map(|(name, value)| Ok((name, resolve(value, directory, parameters, settings)?)))
    .collect::<Result<Vec<_>>>()?;
  let output = if let Some(output) = &step.output_file {
    let path = resolve_path(resolve(&output.path, directory, parameters, settings)?, directory)?;
    let outside = output
      .outside_directory
      .as_ref()
      .map(|v| resolve_path(resolve(v, directory, parameters, settings)?, directory))
      .transpose()?;
    Some(crate::process_output::FileOutput::prepare(path, outside, &output.suffix)?)
  } else {
    None
  };
  for argument in &mut arguments {
    if argument.is_none() {
      *argument = Some(output.as_ref().unwrap().staged.as_os_str().to_owned());
    }
  }
  // Only explicit executable paths, cwd and path references are normalized.
  // Literal arguments keep backslashes, empty strings and option-like values.
  let mut command = Command::new(executable);
  command.args(arguments.into_iter().map(Option::unwrap)).current_dir(cwd);
  command.envs(environment);
  let captured = if step.capture_output {
    let file = tempfile::NamedTempFile::new()?;
    command.stdout(file.reopen()?);
    Some(file)
  } else {
    None
  };
  let code = process::run_timeout(&mut command, step.timeout_seconds.map(std::time::Duration::from_secs))?;
  let mut result = serde_json::json!({"exit-code":code});
  if code == 0
    && let Some(captured) = captured
  {
    if captured.as_file().metadata()?.len() > 4 * 1024 * 1024 {
      bail!("Captured process output exceeds 4 MiB.");
    }
    result["output"] =
      Value::String(std::fs::read_to_string(captured.path()).context("Captured process output is not UTF-8.")?);
  }
  if code == 0
    && let Some(output) = &output
  {
    output.publish()?;
  }
  Ok((code, result))
}

#[cfg(all(test, unix))]
mod tests {
  use super::*;
  use serde_json::json;
  use std::os::unix::{ffi::OsStringExt, fs::symlink};

  #[test]
  fn path_references_preserve_non_utf8_canonical_names() {
    let root = tempfile::tempdir().unwrap();
    let name = OsString::from_vec(b"source-\xff".to_vec());
    let source = root.path().join(&name);
    std::fs::create_dir(&source).unwrap();
    symlink(&source, root.path().join("alias")).unwrap();
    let directory = Directory {
      root_directory: root.path().into(),
      task_directory: root.path().join(".tasks"),
      invocation_directory: root.path().into(),
    };
    let parameters = json!({"source":"alias"});
    let value = resolve(
      &json!({"path-name":{"parameter":"source"}}),
      &directory,
      &parameters,
      &json!({}),
    )
    .unwrap();
    assert_eq!(value, name);
  }
}

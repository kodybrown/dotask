//! Declarative values are literal strings or exact references, never shell text
//! or an expression language. One resolved value always remains one argument.
use crate::{
  configuration, process,
  project::{portable_path, Directory, ProcessStep, Target},
};
use anyhow::{bail, Result};
use serde_json::Value;
use std::{path::Path, process::Command};

pub(crate) fn validate(value: &Value, target: &Target) -> Result<()> {
  if let Some(text) = value.as_str() {
    if text.contains('\0') {
      bail!("Process values cannot contain NUL.");
    }
    return Ok(());
  }
  let reference = value
    .as_object()
    .ok_or_else(|| anyhow::anyhow!("Process values must be strings or parameter/setting references."))?;
  if reference.len() != 1 {
    bail!("A process reference must have exactly one parameter or setting key.");
  }
  let (kind, name) = reference.iter().next().unwrap();
  let name = name
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
    "setting" => Ok(()),
    _ => bail!("Unknown process reference {kind}: {name}."),
  }
}

pub(crate) fn resolve(value: &Value, parameters: &Value, settings: &Value) -> Result<String> {
  let resolved = if value.is_string() {
    value
  } else {
    let (kind, name) = value.as_object().unwrap().iter().next().unwrap();
    let name = name.as_str().unwrap();
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
  Ok(text)
}

pub(crate) fn execute(step: &ProcessStep, directory: &Directory, parameters: &Value, settings: &Value) -> Result<i32> {
  let executable = resolve(&step.executable, parameters, settings)?;
  if executable.trim().is_empty() {
    bail!("Executable must be nonempty.");
  }
  let cwd = portable_path(&directory.root_directory, &resolve(&step.cwd, parameters, settings)?)?;
  if !cwd.is_dir() {
    bail!("Process cwd is not a directory: {}", cwd.display());
  }
  let arguments = step
    .arguments
    .iter()
    .map(|a| resolve(a, parameters, settings))
    .collect::<Result<Vec<_>>>()?;
  // Only explicitly portable executable paths and cwd are normalized. Literal
  // arguments, including backslashes, empty strings and option-like values, are not.
  let executable = if Path::new(&executable).is_absolute() || executable.contains(['/', '\\']) {
    portable_path(&directory.root_directory, &executable)?
  } else {
    executable.into()
  };
  process::run(Command::new(executable).args(arguments).current_dir(cwd))
}

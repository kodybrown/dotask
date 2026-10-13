//! Strict parsing keeps misspelled deployment actions from becoming successful no-ops.
use crate::project::{ProcessOutputFile, ProcessStep, Step, Target};
use anyhow::{bail, Result};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub(crate) fn keys(value: &Value, allowed: &[&str]) -> Result<()> {
  for key in value
    .as_object()
    .ok_or_else(|| anyhow::anyhow!("Expected a YAML mapping."))?
    .keys()
  {
    if !allowed.contains(&key.as_str()) {
      bail!("Unknown .task key '{key}'. Expected {}.", allowed.join(", "));
    }
  }
  Ok(())
}
fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
  value
    .as_str()
    .filter(|s| !s.trim().is_empty())
    .ok_or_else(|| anyhow::anyhow!("{name} requires a nonempty string."))
}
fn flag(value: &Value, name: &str) -> Result<bool> {
  value
    .get(name)
    .map(|v| v.as_bool().ok_or_else(|| anyhow::anyhow!("{name} must be a boolean.")))
    .transpose()
    .map(|v| v.unwrap_or(false))
}

pub(crate) fn steps(value: &Value, target: &Target) -> Result<Vec<Step>> {
  let mut previous = BTreeSet::new();
  let mut parsed = Vec::new();
  for step in value
    .as_array()
    .ok_or_else(|| anyhow::anyhow!("steps must be a sequence (use steps: [] for an empty group)."))?
  {
    keys(step, &["id", "run", "exec", "file", "optional", "with", "when"])?;
    if ["run", "exec", "file"].iter().filter(|key| step.get(**key).is_some()).count() != 1 {
      bail!("Every step requires exactly one run, exec, or file action.");
    }
    crate::bindings::validate(step, &previous)?;
    let id = step.get("id").map(|v| text(v, "id").map(str::to_owned)).transpose()?;
    if let Some(id) = &id
      && (!crate::catalog::identifier(id) || !previous.insert(id.clone()))
    {
      bail!("Step id '{id}' must be a unique identifier.");
    }
    let when = step.get("when").cloned();
    if let Some(condition) = &when {
      keys(condition, &["parameter", "result"])?;
      if condition.as_object().unwrap().len() != 1 {
        bail!("when needs one boolean parameter or result reference.");
      }
      if let Some(name) = condition.get("parameter") {
        let name = text(name, "parameter")?;
        if crate::configuration::option(target, name).is_none_or(|o| o["Type"] != "bool") {
          bail!("when must reference a declared boolean parameter: {name}");
        }
      }
    }
    let run = step.get("run").map(|v| text(v, "run")).transpose()?.unwrap_or("").to_owned();
    let file = step.get("file").cloned();
    if let Some(file) = &file {
      crate::file_steps::validate(file, target)?;
    }
    if run.is_empty() && (step.get("optional").is_some() || step.get("with").is_some()) {
      bail!("exec and file steps cannot declare optional or with.");
    }
    let exec = step.get("exec").map(|v| process(v, target)).transpose()?;
    let parameters = step.get("with").cloned().unwrap_or(json!({}));
    for (name, value) in parameters
      .as_object()
      .ok_or_else(|| anyhow::anyhow!("with must be a mapping of parameter names to values."))?
    {
      if value.is_object() {
        crate::process_steps::validate(value, target)?;
      } else if value.is_array() || value.is_null() {
        bail!("with.{name} must be a scalar or exact reference.");
      }
    }
    parsed.push(Step {
      id,
      run,
      optional: flag(step, "optional")?,
      parameters,
      exec,
      file,
      when,
    });
  }
  Ok(parsed)
}

fn process(command: &Value, target: &Target) -> Result<ProcessStep> {
  keys(
    command,
    &[
      "executable",
      "arguments",
      "cwd",
      "output-file",
      "environment",
      "timeout-seconds",
      "capture-output",
    ],
  )?;
  let output_file = command
    .get("output-file")
    .map(|output| -> Result<_> {
      keys(output, &["path", "outside-directory", "suffix"])?;
      let path = output
        .get("path")
        .ok_or_else(|| anyhow::anyhow!("output-file requires path."))?
        .clone();
      crate::process_steps::validate(&path, target)?;
      if path.as_str().is_some_and(|s| s.trim().is_empty()) {
        bail!("output-file path must be nonempty.");
      }
      let outside_directory = output.get("outside-directory").cloned();
      if let Some(outside) = &outside_directory {
        crate::process_steps::validate(outside, target)?;
        if outside.as_str().is_some_and(|s| s.trim().is_empty()) {
          bail!("outside-directory must be nonempty.");
        }
      }
      let suffix = output
        .get("suffix")
        .map(|v| v.as_str().ok_or_else(|| anyhow::anyhow!("suffix must be a string.")))
        .transpose()?
        .unwrap_or("");
      if !suffix
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
      {
        bail!("output-file suffix must use portable filename characters: letters, digits, '.', '-' or '_'.");
      }
      Ok(ProcessOutputFile {
        path,
        outside_directory,
        suffix: suffix.into(),
      })
    })
    .transpose()?;
  let executable = command
    .get("executable")
    .ok_or_else(|| anyhow::anyhow!("exec requires executable."))?
    .clone();
  crate::process_steps::validate(&executable, target)?;
  if executable.as_str().is_some_and(|s| s.trim().is_empty()) {
    bail!("Executable must be nonempty.");
  }
  let arguments = command.get("arguments").cloned().unwrap_or(json!([]));
  let arguments = arguments
    .as_array()
    .ok_or_else(|| anyhow::anyhow!("arguments must be a sequence."))?
    .clone();
  for argument in &arguments {
    crate::process_steps::validate_argument(argument, target, output_file.is_some())?;
  }
  if output_file.is_some() && !arguments.iter().any(|v| v.get("output-file") == Some(&json!(true))) {
    bail!("An output-file process requires an argument containing {{output-file: true}}.");
  }
  let cwd = command.get("cwd").cloned().unwrap_or(json!("."));
  crate::process_steps::validate(&cwd, target)?;
  if cwd.as_str().is_some_and(|s| s.trim().is_empty()) {
    bail!("cwd must be nonempty.");
  }
  let environment = command.get("environment").cloned().unwrap_or(json!({}));
  let environment: Map<String, Value> = environment
    .as_object()
    .ok_or_else(|| anyhow::anyhow!("environment must be a mapping."))?
    .clone();
  for (name, value) in &environment {
    if name.is_empty() || name.contains(['=', '\0']) {
      bail!("Invalid environment variable name.");
    }
    crate::process_steps::validate(value, target)?;
  }
  let timeout_seconds = command
    .get("timeout-seconds")
    .map(|v| {
      v.as_u64()
        .filter(|n| *n > 0 && *n <= 86400)
        .ok_or_else(|| anyhow::anyhow!("timeout-seconds must be an integer from 1 to 86400."))
    })
    .transpose()?;
  Ok(ProcessStep {
    executable,
    arguments,
    cwd,
    output_file,
    environment,
    timeout_seconds,
    capture_output: flag(command, "capture-output")?,
  })
}

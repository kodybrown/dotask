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
  scoped_steps(value, target, BTreeSet::new(), &BTreeSet::new(), 0)
}
fn scoped_steps(
  value: &Value,
  target: &Target,
  mut previous: BTreeSet<String>,
  items: &BTreeSet<String>,
  depth: usize,
) -> Result<Vec<Step>> {
  if depth > 16 {
    bail!("foreach nesting exceeds 16 levels.");
  }
  let mut parsed = Vec::new();
  for step in value
    .as_array()
    .ok_or_else(|| anyhow::anyhow!("steps must be a sequence (use steps: [] for an empty group)."))?
  {
    keys(
      step,
      &[
        "id", "run", "exec", "file", "http", "json", "text", "foreach", "steps", "optional", "with", "when",
      ],
    )?;
    if ["run", "exec", "file", "http", "json", "text", "foreach"]
      .iter()
      .filter(|key| step.get(**key).is_some())
      .count()
      != 1
    {
      bail!("Every step requires exactly one run, exec, file, http, json, text, or foreach action.");
    }
    let mut bindings = step.clone();
    bindings.as_object_mut().unwrap().remove("steps");
    // These mappings declare names, not references themselves. Validate their
    // values so fields/placeholders named 'result' or 'item' remain ordinary names.
    for (action, names) in [
      ("http", &["headers", "query"][..]),
      ("text", &["values"][..]),
      ("file", &["values", "initial", "fill-empty", "set", "match-existing"][..]),
    ] {
      if let Some(action) = bindings.get_mut(action) {
        for name in names {
          if let Some(value) = action.get_mut(*name)
            && let Some(map) = value.as_object()
          {
            *value = Value::Array(map.values().cloned().collect());
          }
        }
      }
    }
    if let Some(action) = bindings.get_mut("json") {
      action.as_object_mut().map(|m| m.remove("fields"));
    }
    if let Some(values) = bindings.get_mut("with")
      && let Some(map) = values.as_object()
    {
      *values = Value::Array(map.values().cloned().collect());
    }
    crate::bindings::validate_scoped(&bindings, &previous, items)?;
    let iteration = if let Some(iteration) = step.get("foreach") {
      keys(iteration, &["items", "as"])?;
      let input = iteration
        .get("items")
        .ok_or_else(|| anyhow::anyhow!("foreach requires items."))?;
      crate::workflow_values::validate(input, target)?;
      if !input.is_array() {
        crate::process_steps::validate(input, target)?;
      }
      let name = text(&iteration["as"], "foreach.as")?;
      if !crate::catalog::identifier(name) || items.contains(name) {
        bail!("foreach.as must be a unique enclosing item name.");
      }
      let mut nested_items = items.clone();
      nested_items.insert(name.into());
      Some(crate::project::Iteration {
        items: input.clone(),
        name: name.into(),
        steps: scoped_steps(
          step.get("steps").ok_or_else(|| anyhow::anyhow!("foreach requires steps."))?,
          target,
          previous.clone(),
          &nested_items,
          depth + 1,
        )?,
      })
    } else {
      if step.get("steps").is_some() {
        bail!("Only foreach may declare nested steps.");
      }
      None
    };
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
      bail!("Only run steps may declare optional or with.");
    }
    let http = step.get("http").cloned();
    if let Some(action) = &http {
      crate::http_steps::validate(action, target)?;
    }
    let json = step.get("json").cloned();
    if let Some(action) = &json {
      crate::workflow_values::validate_json(action, target)?;
    }
    let text_action = step.get("text").cloned();
    if let Some(action) = &text_action {
      crate::workflow_values::validate_text(action, target)?;
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
      http,
      json,
      text: text_action,
      iteration,
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

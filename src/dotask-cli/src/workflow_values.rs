//! Small, typed data operations deliberately avoid an embedded expression language.
use crate::{declarative::keys, project::Target};
use anyhow::{bail, ensure, Result};
use serde_json::{json, Value};

pub(crate) fn field<'a>(value: &'a Value, path: &str) -> Result<&'a Value> {
  let mut value = value;
  if !path.is_empty() {
    for part in path.split('.') {
      value = if let Some(array) = value.as_array() {
        part.parse::<usize>().ok().and_then(|i| array.get(i))
      } else {
        value.get(part)
      }
      .ok_or_else(|| anyhow::anyhow!("Missing data field '{path}'."))?;
    }
  }
  Ok(value)
}

pub(crate) fn validate(value: &Value, target: &Target) -> Result<()> {
  match value {
    Value::Object(map)
      if map
        .keys()
        .any(|k| matches!(k.as_str(), "result" | "item" | "setting" | "parameter" | "environment")) =>
    {
      crate::process_steps::validate(value, target)
    }
    Value::Object(map) => {
      for value in map.values() {
        validate(value, target)?;
      }
      Ok(())
    }
    Value::Array(values) => {
      for value in values {
        validate(value, target)?;
      }
      Ok(())
    }
    _ => Ok(()),
  }
}

pub(crate) fn resolve(value: &Value, parameters: &Value, settings: &Value) -> Result<Value> {
  if let Some(map) = value.as_object() {
    if map.len() == 1 {
      for (kind, source) in [("parameter", parameters), ("setting", settings)] {
        if let Some(name) = map.get(kind).and_then(Value::as_str) {
          return crate::configuration::find(source, name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Missing {kind} '{name}'."));
        }
      }
    }
    return Ok(Value::Object(
      map
        .iter()
        .map(|(k, v)| Ok((k.clone(), resolve(v, parameters, settings)?)))
        .collect::<Result<_>>()?,
    ));
  }
  if let Some(array) = value.as_array() {
    return Ok(Value::Array(
      array.iter().map(|v| resolve(v, parameters, settings)).collect::<Result<_>>()?,
    ));
  }
  Ok(value.clone())
}

pub(crate) fn bind(
  value: &Value,
  results: &serde_json::Map<String, Value>,
  items: &serde_json::Map<String, Value>,
  parameters: &Value,
  settings: &Value,
) -> Result<Value> {
  if let Some(map) = value.as_object() {
    if map.len() == 1 {
      if map.keys().any(|k| matches!(k.as_str(), "result" | "item" | "environment")) {
        return crate::bindings::resolve_scoped(value, results, items);
      }
      if map.keys().any(|k| matches!(k.as_str(), "setting" | "parameter")) {
        return resolve(value, parameters, settings);
      }
    }
    return Ok(Value::Object(
      map
        .iter()
        .map(|(k, v)| Ok((k.clone(), bind(v, results, items, parameters, settings)?)))
        .collect::<Result<_>>()?,
    ));
  }
  if let Some(array) = value.as_array() {
    return Ok(Value::Array(
      array
        .iter()
        .map(|v| bind(v, results, items, parameters, settings))
        .collect::<Result<_>>()?,
    ));
  }
  Ok(value.clone())
}

pub(crate) fn scalar(value: &Value) -> Result<String> {
  match value {
    Value::String(s) => {
      ensure!(!s.contains('\0'), "Text cannot contain NUL.");
      Ok(s.clone())
    }
    Value::Number(_) | Value::Bool(_) => Ok(value.to_string()),
    _ => bail!("Text requires a string, number, or boolean; use a JSON operation for structured data."),
  }
}

pub(crate) fn validate_text(action: &Value, target: &Target) -> Result<()> {
  keys(action, &["template", "values"])?;
  ensure!(action["template"].is_string(), "text requires a string template.");
  ensure!(action["values"].is_object(), "text requires a values mapping.");
  for value in action["values"].as_object().unwrap().values() {
    if value.is_object() {
      crate::process_steps::validate(value, target)?;
    } else {
      scalar(value)?;
    }
  }
  crate::file_steps::render(
    action["template"].as_str().unwrap(),
    action["values"].as_object().unwrap(),
    |_| Ok(String::new()),
  )?;
  Ok(())
}
pub(crate) fn text(action: &Value) -> Result<Value> {
  let values = action["values"].as_object().unwrap();
  Ok(json!({"text": crate::file_steps::render(action["template"].as_str().unwrap(), values, scalar)?}))
}

pub(crate) fn validate_json(action: &Value, target: &Target) -> Result<()> {
  keys(action, &["operation", "input", "where", "fields"])?;
  ensure!(action.get("input").is_some(), "json requires input.");
  match action["operation"].as_str() {
    Some("filter") => {
      if !action["input"].is_array() {
        crate::process_steps::validate(&action["input"], target)?;
      }
      ensure!(action.get("fields").is_none(), "filter cannot declare fields.");
      keys(&action["where"], &["field", "equals"])?;
      ensure!(
        action["where"]["field"].as_str().is_some_and(|s| !s.is_empty()) && action["where"].get("equals").is_some(),
        "filter requires where.field and where.equals."
      );
    }
    Some("select") => {
      ensure!(action.get("where").is_none(), "select cannot declare where.");
      ensure!(
        action["fields"].as_object().is_some_and(|m| m.values().all(Value::is_string)),
        "select requires fields mapping output names to input field paths."
      );
    }
    _ => bail!("json operation must be filter or select."),
  }
  validate(&action["input"], target)?;
  if let Some(value) = action.get("where").and_then(|v| v.get("equals")) {
    validate(value, target)?;
  }
  Ok(())
}
pub(crate) fn json(action: &Value) -> Result<Value> {
  let input = &action["input"];
  let data = match action["operation"].as_str().unwrap() {
    "filter" => {
      let values = input
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("json filter input must be an array."))?;
      let path = action["where"]["field"].as_str().unwrap();
      let mut output = Vec::new();
      for value in values {
        if field(value, path)? == &action["where"]["equals"] {
          output.push(value.clone());
        }
      }
      Value::Array(output)
    }
    "select" => {
      let select = |value: &Value| -> Result<Value> {
        Ok(Value::Object(
          action["fields"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, path)| Ok((name.clone(), field(value, path.as_str().unwrap())?.clone())))
            .collect::<Result<_>>()?,
        ))
      };
      if let Some(values) = input.as_array() {
        Value::Array(values.iter().map(select).collect::<Result<_>>()?)
      } else {
        select(input)?
      }
    }
    _ => unreachable!(),
  };
  Ok(json!({"data":data}))
}

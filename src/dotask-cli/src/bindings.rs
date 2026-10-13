//! Execution-local results never alter project settings or the caller's parameters.
use anyhow::{bail, Result};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub(crate) fn validate(value: &Value, previous: &BTreeSet<String>) -> Result<()> {
  match value {
    Value::Object(object) => {
      if let Some(reference) = object.get("result") {
        let name = reference
          .as_str()
          .ok_or_else(|| anyhow::anyhow!("result requires step.field."))?;
        let (id, field) = name
          .split_once('.')
          .ok_or_else(|| anyhow::anyhow!("result requires step.field."))?;
        if object.len() != 1 || field.is_empty() || !previous.contains(id) {
          bail!("Result '{name}' must reference a previously named step.");
        }
      } else {
        for value in object.values() {
          validate(value, previous)?;
        }
      }
    }
    Value::Array(values) => {
      for value in values {
        validate(value, previous)?;
      }
    }
    _ => {}
  }
  Ok(())
}

pub(crate) fn resolve(value: &Value, results: &Map<String, Value>) -> Result<Value> {
  match value {
    Value::Object(object) if object.len() == 1 && object.contains_key("result") => {
      let reference = object["result"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid result reference."))?;
      let mut parts = reference.split('.');
      let id = parts.next().unwrap();
      let mut value = results
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("Step result '{id}' is unavailable (was it skipped?)."))?;
      for field in parts {
        value = value
          .get(field)
          .ok_or_else(|| anyhow::anyhow!("Missing result field '{reference}'."))?;
      }
      Ok(value.clone())
    }
    Value::Object(object) if object.len() == 1 && object.contains_key("environment") => {
      let name = object["environment"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid environment reference."))?;
      Ok(Value::String(
        std::env::var(name).map_err(|_| anyhow::anyhow!("Missing UTF-8 environment variable '{name}'."))?,
      ))
    }
    Value::Object(object) => Ok(Value::Object(
      object
        .iter()
        .map(|(key, value)| Ok((key.clone(), resolve(value, results)?)))
        .collect::<Result<_>>()?,
    )),
    Value::Array(values) => Ok(Value::Array(values.iter().map(|v| resolve(v, results)).collect::<Result<_>>()?)),
    _ => Ok(value.clone()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn result_references_are_explicit_ordered_and_local() {
    let ids = BTreeSet::from(["read".into()]);
    assert!(validate(&json!({"result":"future.content"}), &ids).is_err());
    let results = json!({"read":{"changed":false,"data":{"port":8080}}});
    assert_eq!(
      resolve(&json!({"result":"read.data.port"}), results.as_object().unwrap()).unwrap(),
      json!(8080)
    );
    assert_eq!(
      resolve(&json!({"result":"read.changed"}), results.as_object().unwrap()).unwrap(),
      json!(false)
    );
    assert!(resolve(&json!({"result":"skipped.changed"}), results.as_object().unwrap()).is_err());
  }
}

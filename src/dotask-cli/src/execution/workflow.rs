//! Loop results are lexical: each iteration starts from the outer snapshot.
use super::Executor;
use crate::{
  bindings, configuration, process,
  project::{arguments, Step},
  workflow_values,
};
use anyhow::{bail, Result};
use serde_json::{json, Map, Value};
use std::path::Path;

pub(super) struct Workflow<'a> {
  pub executor: &'a Executor,
  pub parameters: &'a Value,
  pub settings: &'a Value,
  pub chain: &'a [String],
  pub session: &'a Path,
}
impl Workflow<'_> {
  fn action(
    &self,
    kind: &str,
    action: &Value,
    results: &Map<String, Value>,
    items: &Map<String, Value>,
  ) -> Result<Value> {
    let typed = |value: &Value| workflow_values::bind(value, results, items, self.parameters, self.settings);
    let mut action = action.clone();
    let mappings: &[&str] = match kind {
      "http" => &["query", "headers"],
      "text" => &["values"],
      "file" => &[
        "initial",
        "fill-empty",
        "set",
        "match-existing",
        "body-if-empty",
      ],
      _ => &[],
    };
    for name in mappings {
      if let Some(values) = action.get_mut(*name).and_then(Value::as_object_mut) {
        for value in values.values_mut() {
          *value = typed(value)?;
        }
      }
    }
    for name in match kind {
      "http" => &["url"][..],
      "json" => &["input"][..],
      "file" => &["path", "expected-sha256"][..],
      _ => &[],
    } {
      if let Some(value) = action.get_mut(*name) {
        *value = typed(value)?;
      }
    }
    if kind == "json"
      && let Some(value) = action.get_mut("where").and_then(|v| v.get_mut("equals"))
    {
      *value = typed(value)?;
    }
    Ok(action)
  }
  pub fn steps(
    &self,
    steps: &[Step],
    results: &mut Map<String, Value>,
    items: &Map<String, Value>,
  ) -> Result<(i32, usize)> {
    let mut executed = 0;
    for step in steps {
      process::check_cancelled()?;
      let bind = |value: &Value| bindings::resolve_scoped(value, results, items);
      let typed = |value: &Value| workflow_values::bind(value, results, items, self.parameters, self.settings);
      if let Some(condition) = &step.when {
        let condition = bind(condition)?;
        let enabled = if let Some(name) = condition.get("parameter").and_then(Value::as_str) {
          configuration::get(self.parameters, name).and_then(Value::as_bool)
        } else {
          condition.as_bool()
        }
        .ok_or_else(|| anyhow::anyhow!("Condition requires a boolean parameter or result."))?;
        if !enabled {
          continue;
        }
      }
      let (code, result) = if let Some(iteration) = &step.iteration {
        self.executor.trace("Executing declarative foreach step");
        let values = typed(&iteration.items)?;
        let values = values
          .as_array()
          .ok_or_else(|| anyhow::anyhow!("foreach items must be an array."))?;
        if values.len() > 10_000 {
          bail!("foreach exceeds 10000 items.");
        }
        let mut count = 0;
        for value in values {
          let mut local_items = items.clone();
          local_items.insert(iteration.name.clone(), value.clone());
          // Metadata rejects shadowing outer result IDs. Remove body-local IDs
          // after each iteration instead of copying the complete HTTP collection
          // once per item (which would make large refreshes quadratic).
          let outcome = self.steps(&iteration.steps, results, &local_items);
          for local in &iteration.steps {
            if let Some(id) = &local.id {
              results.remove(id);
            }
          }
          let (code, nested) = outcome?;
          if code != 0 {
            return Ok((code, executed + count));
          }
          count += nested;
        }
        // An empty loop does not satisfy require_at_least_1_step.
        executed += count;
        if let Some(id) = &step.id {
          results.insert(id.clone(), json!({"iterations":values.len()}));
        }
        continue;
      } else if let Some(action) = &step.http {
        self.executor.trace("Executing declarative HTTP step");
        (0, crate::http_steps::execute(self.action("http", action, results, items)?)?)
      } else if let Some(action) = &step.json {
        self.executor.trace("Executing declarative JSON step");
        (0, workflow_values::json(&self.action("json", action, results, items)?)?)
      } else if let Some(action) = &step.text {
        self.executor.trace("Executing declarative text step");
        // Keep the template literal: inserted data is never evaluated again.
        let action = self.action("text", action, results, items)?;
        (0, workflow_values::text(&action)?)
      } else if let Some(file) = &step.file {
        self.executor.trace("Executing declarative file step");
        let file = if file["operation"] == "frontmatter" {
          self.action("file", file, results, items)?
        } else {
          bind(file)?
        };
        (
          0,
          crate::file_steps::execute(&file, &self.executor.directory, self.parameters, self.settings)?,
        )
      } else if let Some(command) = &step.exec {
        self.executor.trace("Executing declarative process step");
        let command = serde_json::from_value(bind(&serde_json::to_value(command)?)?)?;
        crate::process_steps::execute(&command, &self.executor.directory, self.parameters, self.settings)?
      } else {
        let catalog = self.executor.catalog(self.session)?;
        let child = catalog.find(&step.run)?;
        if child.is_none() && step.optional {
          self.executor.trace(&format!("Skipping absent optional target: {}", step.run));
          continue;
        }
        let child = child.ok_or_else(|| anyhow::anyhow!("Unknown target '{}'.", step.run))?;
        let mut values = step.parameters.clone();
        for value in values.as_object_mut().unwrap().values_mut() {
          *value = bind(value)?;
          if value.is_object() {
            *value = Value::String(
              crate::process_steps::resolve(value, &self.executor.directory, self.parameters, self.settings)?
                .into_string()
                .map_err(|_| anyhow::anyhow!("Nested parameter requires UTF-8."))?,
            );
          }
        }
        let code = self.executor.execute(child, &arguments(&values)?, self.chain, self.session)?;
        (code, json!({"exit-code":code}))
      };
      if let Some(id) = &step.id {
        results.insert(id.clone(), result);
      }
      if code != 0 {
        return Ok((code, executed));
      }
      executed += 1;
    }
    Ok((0, executed))
  }
}

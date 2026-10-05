//! Task headers are declarative YAML in ordinary line comments. Reading a
//! header requires no language toolchain, compilation, restore, or task execution.
use crate::{catalog, project::Target};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{fs, path::Path};

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Metadata {
  description: String,
  remarks: Option<String>,
  examples: Vec<String>,
  capabilities: Vec<String>,
  options: Vec<OptionMetadata>,
  requires: Vec<Requirement>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Requirement {
  kind: String,
  value: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionMetadata {
  name: String,
  #[serde(default)]
  alias: Option<String>,
  #[serde(default = "string_type", rename = "type")]
  kind: String,
  #[serde(default)]
  description: String,
  #[serde(default)]
  default: Option<Value>,
  #[serde(default)]
  required: bool,
  #[serde(default)]
  choices: Vec<String>,
  #[serde(default)]
  completion: Option<String>,
}
fn string_type() -> String {
  "string".into()
}

pub(crate) fn metadata(file: &Path, root: &Path) -> Target {
  let mut target = Target {
    name: file.file_stem().unwrap().to_string_lossy().into_owned(),
    file_path: file.into(),
    description: "(no description)".into(),
    short_name: None,
    error: None,
    group: None,
    options: vec![],
    metadata: serde_json::Map::new(),
  };
  let parsed = (|| -> Result<()> {
    (target.name, target.short_name) = catalog::read_name(file, root)?;
    let source = fs::read_to_string(file)?;
    let prefix = if file.extension().is_some_and(|e| e == "py") {
      "#"
    } else {
      "//"
    };
    let marker = format!("{prefix} dotask: 1");
    let mut lines = source.trim_start_matches('\u{feff}').lines().skip_while(|l| {
      l.trim().is_empty()
        || l.starts_with("#!")
        || (prefix == "#" && (l.starts_with("# coding:") || l.starts_with("# -*- coding:")))
    });
    let first = lines.next().unwrap_or_default().trim();
    if first.starts_with(&format!("{prefix} dotask:")) && first != marker {
      bail!("Unsupported dotask metadata version.");
    }
    let metadata = if first == marker {
      let mut yaml = String::new();
      let mut ended = false;
      for line in lines {
        let doc = line
          .trim_start()
          .strip_prefix(prefix)
          .context("Task metadata must remain inside the leading comment header")?;
        if doc.trim() == "end-dotask" {
          ended = true;
          break;
        }
        yaml.push_str(doc.strip_prefix(' ').unwrap_or(doc));
        yaml.push('\n');
      }
      if !ended {
        bail!("Unterminated dotask metadata header.");
      }
      if yaml.trim().is_empty() {
        Metadata::default()
      } else {
        let value = crate::yaml::parse(&yaml, &file.display().to_string())?;
        // Absence has a default; an explicitly declared null is not a scalar.
        // Serde's Option would otherwise silently treat null as an absent value.
        for option in value["options"].as_array().into_iter().flatten() {
          if option.get("default").is_some_and(Value::is_null) {
            bail!("Option defaults must be scalars.");
          }
        }
        serde_json::from_value::<Metadata>(value)?
      }
    } else {
      Metadata::default()
    };
    if !metadata.description.is_empty() {
      target.description = metadata.description;
    }
    let mut names = std::collections::BTreeSet::new();
    for option in metadata.options {
      if option
        .alias
        .as_ref()
        .is_some_and(|a| a.len() != 1 || !a.as_bytes()[0].is_ascii_alphabetic())
      {
        bail!("Option aliases must be single-letter names.");
      }
      for name in std::iter::once(&option.name).chain(option.alias.iter()) {
        if !catalog::identifier(name)
          || !names.insert(name.to_lowercase())
          || ["help", "h", "verbose", "version", "use-dir"].contains(&name.to_ascii_lowercase().as_str())
        {
          bail!("Invalid, reserved, or duplicate option '{name}'.");
        }
      }
      if !["string", "path", "bool", "int", "number"].contains(&option.kind.as_str()) {
        bail!("Unknown option type '{}'.", option.kind);
      }
      if option
        .completion
        .as_deref()
        .is_some_and(|c| !["file", "directory", "none"].contains(&c))
      {
        bail!("Invalid completion kind.");
      }
      let default = option
        .default
        .map(|v| match v {
          Value::String(s) => Ok(s),
          Value::Bool(_) | Value::Number(_) => Ok(v.to_string()),
          _ => Err(anyhow::anyhow!("Option defaults must be scalars.")),
        })
        .transpose()?;
      let value = json!({"Name":option.name,"Alias":option.alias,"Type":option.kind,"Description":option.description,"Default":default,"Required":option.required,"Choices":option.choices,"Completion":option.completion});
      if let Some(value_text) = value["Default"].as_str() {
        crate::configuration::convert(&value, value_text, root)?;
      }
      target.options.push(value);
    }
    let mut requirements = vec![];
    for requirement in metadata.requires {
      if !["tool", "setting", "task", "file", "os"].contains(&requirement.kind.as_str())
        || requirement.value.trim().is_empty()
      {
        bail!("Invalid task requirement.");
      }
      if requirement.kind == "file" {
        crate::rust_tasks::support(root, &requirement.value)?;
      }
      requirements.push(json!({"Kind":requirement.kind,"Value":requirement.value}));
    }
    target.metadata.extend([
      ("Requirements".into(), json!(requirements)),
      ("Remarks".into(), json!(metadata.remarks)),
      ("Examples".into(), json!(metadata.examples)),
      ("Capabilities".into(), json!(metadata.capabilities)),
    ]);
    Ok(())
  })();
  if let Err(error) = parsed {
    target.error = Some(format!("Metadata error in {}: {error:#}", file.display()));
  }
  target
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn one_header_contract_reads_each_comment_prefix_and_ignores_code() {
    let root = tempfile::tempdir().unwrap();
    for extension in ["cs", "rs", "py", "c", "cpp", "ts"] {
      let prefix = if extension == "py" { "#" } else { "//" };
      let file = root.path().join(format!("task.{extension}"));
      let preamble = if extension == "py" {
        "#!/usr/bin/env python\n# coding: utf-8\n"
      } else {
        ""
      };
      fs::write(&file, format!("\u{feff}{preamble}\n{prefix} dotask: 1\n{prefix} description: '日本語 & metadata'\n{prefix} options: [{{name: flag, type: bool, default: true}}]\n{prefix} end-dotask\ninvalid language body\n")).unwrap();
      let target = metadata(&file, root.path());
      assert!(target.error.is_none(), "{:?}", target.error);
      assert_eq!(target.description, "日本語 & metadata");
      assert_eq!(target.options[0]["Default"], "true");
    }
  }
  #[test]
  fn malformed_versions_headers_and_fields_are_errors_before_execution() {
    let root = tempfile::tempdir().unwrap();
    for source in [
      "// dotask: 2\n// end-dotask",
      "// dotask: 1\n// description: x",
      "// dotask: 1\nlet value = 1;\n// end-dotask",
      "// dotask: 1\n// description: x\n// DESCRIPTION: y\n// end-dotask",
      "// dotask: 1\n// options: [{name: name, alias: long}]\n// end-dotask",
      "// dotask: 1\n// options: [{name: n, default: []}]\n// end-dotask",
    ] {
      let file = root.path().join("task.cs");
      fs::write(&file, source).unwrap();
      assert!(metadata(&file, root.path()).error.is_some(), "{source}");
    }
  }
}

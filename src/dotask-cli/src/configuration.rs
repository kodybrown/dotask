use crate::project::{portable_path, Directory, Target};
use anyhow::{bail, Result};
use serde_json::{json, Map, Value};
use std::path::Path;

// Ordinal identifiers fold individual characters, never whole strings. Avoid
// expansion (for example sharp-s into two letters) or culture-sensitive casing.
pub(crate) fn same_key(a: &str, b: &str) -> bool {
  fn upper(c: char) -> char {
    let mut chars = c.to_uppercase();
    let first = chars.next().unwrap();
    if chars.next().is_none() {
      first
    } else {
      c
    }
  }
  let mut left = a.chars();
  let mut right = b.chars();
  loop {
    match (left.next(), right.next()) {
      (None, None) => return true,
      (Some(a), Some(b)) if a == b || a.is_ascii() == b.is_ascii() && upper(a) == upper(b) => {}
      _ => return false,
    }
  }
}

pub(crate) fn get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
  value
    .as_object()?
    .iter()
    .find(|(k, _)| same_key(k, key))
    .map(|(_, v)| v)
    .filter(|v| !v.is_null())
}
pub(crate) fn find<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
  let mut value = value;
  for part in key.split('.') {
    value = get(value, part)?;
  }
  Some(value)
}
pub(crate) fn argument(value: &Value) -> Result<String> {
  match value {
    Value::String(s) => Ok(s.clone()),
    Value::Bool(_) | Value::Number(_) => Ok(value.to_string()),
    _ => bail!("Target arguments must be strings, numbers, or booleans."),
  }
}
pub(crate) fn load(directory: &Directory) -> Result<Value> {
  let legacy = directory.task_directory.join("config.yaml");
  let current = directory.root_directory.join(".dotasks.yaml");
  if legacy.is_file() && current.is_file() {
    bail!("Both .dotasks.yaml and the legacy task-directory config.yaml exist. Keep the project configuration in .dotasks.yaml and remove the duplicate after reviewing it.");
  }
  let file = if current.is_file() { current } else { legacy };
  if !file.is_file() {
    return Ok(json!({"Settings": {}, "TargetDefaults": {}, "Name": null, "Description": null}));
  }
  let source = file.file_name().unwrap().to_string_lossy();
  let value = crate::yaml::parse(&std::fs::read_to_string(&file)?, &source)?;
  let object = value
    .as_object()
    .ok_or_else(|| anyhow::anyhow!("{source} must contain one mapping document."))?;
  for key in object.keys() {
    if !["version", "name", "description", "settings", "targets"]
      .iter()
      .any(|k| key.eq_ignore_ascii_case(k))
    {
      bail!("Unknown {source} key '{key}'. Expected version, name, description, settings, or targets.");
    }
  }
  if get(&value, "version").is_some_and(|v| v.as_i64() != Some(1)) {
    bail!("Unsupported {source} version. Expected version: 1.");
  }
  let empty = json!({});
  let settings = get(&value, "settings").unwrap_or(&empty);
  let targets = get(&value, "targets").unwrap_or(&empty);
  if !settings.is_object() || !targets.is_object() {
    bail!("{source} settings and targets must be mappings.");
  }
  for (name, target) in targets.as_object().unwrap() {
    if target
      .as_object()
      .is_none_or(|o| o.iter().any(|(k, v)| !k.eq_ignore_ascii_case("defaults") || !v.is_object()))
    {
      bail!("{source} targets.{name} only supports a defaults mapping.");
    }
  }
  let text = |key: &str| -> Result<Option<String>> {
    match get(&value, key) {
      None => Ok(None),
      Some(Value::String(s)) => Ok((!s.trim().is_empty()).then(|| s.trim().to_owned())),
      _ => bail!("{source} {key} must be a string."),
    }
  };
  Ok(
    json!({"Settings": settings, "TargetDefaults": targets, "Name": text("name")?, "Description": text("description")?}),
  )
}

pub(crate) fn defaults(config: &Value, name: &str) -> Result<Map<String, Value>> {
  let key = format!("{name}.defaults");
  let Some(value) = find(&config["TargetDefaults"], &key) else {
    return Ok(Map::new());
  };
  value
    .as_object()
    .ok_or_else(|| anyhow::anyhow!("Target defaults must be a mapping."))?
    .iter()
    .map(|(k, v)| Ok((k.clone(), Value::String(argument(v)?))))
    .collect()
}
pub(crate) fn option<'a>(target: &'a Target, name: &str) -> Option<&'a Value> {
  target.options.iter().find(|o| {
    ["Name", "Alias"]
      .iter()
      .any(|key| o[key].as_str().is_some_and(|s| s.eq_ignore_ascii_case(name)))
  })
}
pub(crate) fn boolean(value: &str) -> Option<bool> {
  if value.trim().eq_ignore_ascii_case("true") {
    Some(true)
  } else if value.trim().eq_ignore_ascii_case("false") {
    Some(false)
  } else {
    None
  }
}
pub(crate) fn bind(target: &Target, config: &Value, args: &[String], root: &Path, help: bool) -> Result<Value> {
  if let Some(error) = &target.error {
    bail!("{error}");
  }
  let mut supplied = Map::new();
  let mut index = 0;
  while index < args.len() {
    let arg = &args[index];
    let token = arg.strip_prefix("--").or_else(|| arg.strip_prefix('-')).unwrap_or(arg);
    let (name, inline) = token.split_once('=').map(|(a, b)| (a, Some(b))).unwrap_or((token, None));
    let option =
      option(target, name).ok_or_else(|| anyhow::anyhow!("Unknown option '{name}' for target '{}'.", target.name))?;
    let canonical = option["Name"].as_str().unwrap();
    let value = if let Some(value) = inline {
      value.to_owned()
    } else if !arg.starts_with('-') {
      bail!("Expected --{name} VALUE or {name}=VALUE.");
    } else if option["Type"] == "bool" && args.get(index + 1).and_then(|s| boolean(s)).is_none() {
      "true".to_owned()
    } else {
      index += 1;
      args
        .get(index)
        .filter(|v| !v.starts_with("--"))
        .ok_or_else(|| anyhow::anyhow!("Missing value for --{canonical}."))?
        .clone()
    };
    if supplied.insert(canonical.into(), Value::String(value)).is_some() {
      bail!("Option '{canonical}' was supplied more than once.");
    }
    index += 1;
  }
  let defaults = Value::Object(defaults(config, &target.name)?);
  for key in defaults.as_object().unwrap().keys() {
    if !target
      .options
      .iter()
      .any(|o| o["Name"].as_str().is_some_and(|s| s.eq_ignore_ascii_case(key)))
    {
      bail!("Target '{}' has no option '{key}' configured in config.yaml.", target.name);
    }
  }
  let mut values = Map::new();
  for option in &target.options {
    let name = option["Name"].as_str().unwrap();
    let kind = option["Type"].as_str().unwrap();
    let value = supplied
      .get(name)
      .or_else(|| get(&defaults, name))
      .or_else(|| get(option, "Default"));
    let Some(value) = value else {
      if !help && option["Required"] == true {
        bail!("Target '{}' requires --{name}.", target.name);
      }
      if kind == "bool" {
        values.insert(name.into(), Value::Bool(false));
      }
      continue;
    };
    values.insert(name.into(), convert(option, value.as_str().unwrap(), root)?);
  }
  if !help {
    requirements(target, config, root)?;
  }
  Ok(Value::Object(values))
}
pub(crate) fn convert(option: &Value, value: &str, root: &Path) -> Result<Value> {
  let choices = option["Choices"].as_array().unwrap();
  let name = option["Name"].as_str().unwrap();
  let value = if choices.is_empty() {
    value
  } else {
    choices
      .iter()
      .filter_map(Value::as_str)
      .find(|s| same_key(s, value))
      .ok_or_else(|| {
        anyhow::anyhow!(
          "Invalid value for --{name}: '{value}'. Choose {}.",
          choices.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")
        )
      })?
  };
  let kind = option["Type"].as_str().unwrap();
  let converted = match kind {
    "string" => Some(Value::String(value.into())),
    "path" => Some(Value::String(portable_path(root, value)?.to_string_lossy().into_owned())),
    "bool" => boolean(value).map(Value::Bool),
    "int" => value.trim().parse::<i32>().ok().map(Value::from),
    "number" => value.trim().parse::<f64>().ok().filter(|v| v.is_finite()).map(Value::from),
    _ => None,
  };
  converted.ok_or_else(|| anyhow::anyhow!("Invalid {kind} value for --{name}: '{value}'."))
}
fn requirements(target: &Target, config: &Value, root: &Path) -> Result<()> {
  for r in target
    .metadata
    .get("Requirements")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
  {
    let kind = r["Kind"].as_str().unwrap();
    let value = r["Value"].as_str().unwrap();
    let valid = match kind {
      "setting" => find(&config["Settings"], value).is_some(),
      "tool" => tool_exists(value, root),
      "os" => value.split(',').any(|v| {
        v.trim().eq_ignore_ascii_case(if cfg!(windows) {
          "windows"
        } else if cfg!(target_os = "macos") {
          "macos"
        } else {
          "linux"
        })
      }),
      "task" | "file" => true,
      _ => false,
    };
    if !valid {
      bail!("Target '{}' requires {kind} '{value}'.", target.name);
    }
  }
  Ok(())
}
pub(crate) fn tool_exists(tool: &str, root: &Path) -> bool {
  let mut names = vec![tool.to_owned()];
  if cfg!(windows) && Path::new(tool).extension().is_none() {
    names.extend(
      std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".EXE;.COM;.BAT;.CMD".into())
        .split(';')
        .map(|e| format!("{tool}{e}")),
    );
  }
  let directories = if tool.contains(['/', '\\']) {
    vec![root.to_path_buf()]
  } else {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect()
  };
  directories.iter().any(|d| {
    names.iter().any(|n| {
      let p = root.join(d).join(n);
      if !p.is_file() {
        return false;
      }
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        p.metadata().is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
      }
      #[cfg(windows)]
      {
        true
      }
    })
  })
}

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Directory {
  pub task_directory: PathBuf,
  pub root_directory: PathBuf,
  pub invocation_directory: PathBuf,
}

impl Directory {
  pub fn locate(invocation: PathBuf, selected: Option<&str>) -> Result<Self> {
    Self::locate_optional(invocation, selected, false)
  }
  pub fn locate_optional(invocation: PathBuf, selected: Option<&str>, allow_missing: bool) -> Result<Self> {
    let (task_directory, root_directory) = if let Some(selected) = selected {
      let task = portable_path(&invocation, selected)?;
      if !task.is_dir() && !allow_missing {
        bail!("Task directory does not exist: {}", task.display());
      }
      let anchor = if task.join("config.yaml").is_file() {
        None
      } else {
        task.ancestors().find(|p| p.join(".dotasks.yaml").is_file())
      };
      let root = anchor.unwrap_or_else(|| task.parent().unwrap_or(&task)).to_path_buf();
      (task, root)
    } else {
      let root = invocation
        .ancestors()
        .find(|p| p.join(".dotasks.yaml").is_file() || p.join(".tasks").is_dir());
      let root = root.or_else(|| allow_missing.then_some(invocation.as_path()));
      let Some(root) = root else {
        bail!("No .dotasks.yaml or .tasks directory found. Run dotask --init here or use dotask --add TASK.");
      };
      (root.join(".tasks"), root.to_path_buf())
    };
    Ok(Self {
      task_directory,
      root_directory,
      invocation_directory: invocation,
    })
  }
}

pub(crate) fn portable_path(root: &Path, value: &str) -> Result<PathBuf> {
  if value.contains('\0') {
    bail!("A path cannot contain a NUL character.");
  }
  #[cfg(unix)]
  if value.starts_with("\\\\")
    || (value.len() >= 2 && value.as_bytes()[0].is_ascii_alphabetic() && value.as_bytes()[1] == b':')
  {
    bail!("Windows rooted path '{value}' cannot be resolved on this host.");
  }
  let path = std::path::absolute(root.join(value.replace(['/', '\\'], std::path::MAIN_SEPARATOR_STR)))?;
  // Lexical normalization preserves the invocation spelling and does not
  // require directories to exist (configuration alone can anchor a project).
  let mut normalized = PathBuf::new();
  for component in path.components() {
    match component {
      std::path::Component::ParentDir => {
        normalized.pop();
      }
      std::path::Component::CurDir => {}
      other => normalized.push(other.as_os_str()),
    }
  }
  Ok(normalized)
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Target {
  pub name: String,
  pub file_path: PathBuf,
  pub description: String,
  pub short_name: Option<String>,
  pub error: Option<String>,
  pub group: Option<Group>,
  pub options: Vec<Value>,
  #[serde(flatten)]
  pub metadata: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Group {
  pub require_at_least_one_step: bool,
  pub steps: Vec<Step>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Step {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,
  pub run: String,
  pub optional: bool,
  pub parameters: Value,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub exec: Option<ProcessStep>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub when: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub file: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub http: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub json: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub text: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub iteration: Option<Iteration>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Iteration {
  pub items: Value,
  pub name: String,
  pub steps: Vec<Step>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct ProcessStep {
  pub executable: Value,
  pub arguments: Vec<Value>,
  pub cwd: Value,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub output_file: Option<ProcessOutputFile>,
  #[serde(default, skip_serializing_if = "Map::is_empty")]
  pub environment: Map<String, Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub timeout_seconds: Option<u64>,
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub capture_output: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct ProcessOutputFile {
  pub path: Value,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub outside_directory: Option<Value>,
  pub suffix: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Catalog {
  pub configuration: Value,
  pub targets: Vec<Target>,
}

impl Catalog {
  pub fn names(&self, target: &Target) -> Vec<String> {
    let mut names = vec![target.name.clone()];
    for alias in target
      .aliases()
      .into_iter()
      .filter(|a| !a.contains('.') || target.name.contains('.'))
    {
      if !self.targets.iter().any(|t| t.name.eq_ignore_ascii_case(&alias))
        && self.find(&alias).is_ok_and(|t| t.is_some())
        && !names.iter().any(|n| n.eq_ignore_ascii_case(&alias))
      {
        names.push(alias);
      }
    }
    names
  }
  pub fn explicit_names(&self, target: &Target) -> Vec<String> {
    std::iter::once(target.qualified_name())
      .chain(target.aliases().into_iter().filter(|s| s.contains('.')))
      .filter(|name| {
        self
          .find(name)
          .is_ok_and(|t| t.is_some_and(|t| t.file_path == target.file_path))
      })
      .collect()
  }
  pub fn display_name(&self, target: &Target) -> String {
    self.names(target).get(1).cloned().unwrap_or_else(|| target.name.clone())
  }
  pub fn find(&self, name: &str) -> Result<Option<&Target>> {
    self.find_with_language(name, None)
  }
  pub fn find_with_language(&self, name: &str, language: Option<&str>) -> Result<Option<&Target>> {
    // Selection belongs to this invocation. The returned target keeps its
    // canonical identity; nested calls resolve independently across languages.
    if let Some(language) = language {
      let extension = Path::new(name).extension().and_then(|e| e.to_str());
      if let Some(extension) = extension {
        let expected = if language == "rust" { "rs" } else { "cs" };
        if !extension.eq_ignore_ascii_case(expected) {
          bail!("Target '{name}' conflicts with --lang {language}.");
        }
      }
    }
    let exact: Vec<_> = self
      .targets
      .iter()
      .filter(|t| language.is_none_or(|l| t.language() == Some(l)))
      .filter(|t| t.name.eq_ignore_ascii_case(name) || t.qualified_name().eq_ignore_ascii_case(name))
      .collect();
    if exact.len() > 1 {
      // No full-name spelling can disambiguate colliding files. Report the
      // catalog's file-level diagnostic instead of suggesting the same command
      // twice, including for existence queries and optional nested calls.
      bail!("{}", exact[0].error.as_deref().unwrap_or("Ambiguous full target name."));
    }
    let matches = if exact.is_empty() {
      self
        .targets
        .iter()
        .filter(|t| language.is_none_or(|l| t.language() == Some(l)))
        .filter(|t| t.aliases().iter().any(|a| a.eq_ignore_ascii_case(name)))
        .collect()
    } else {
      exact
    };
    match matches.as_slice() {
      [] => Ok(None),
      [target] => Ok(Some(target)),
      _ => bail!(
        "Ambiguous target '{name}'. Use {}.",
        matches
          .iter()
          .map(|t| format!("'dotask {}' ({})", t.name, t.file_path.display()))
          .collect::<Vec<_>>()
          .join(" or ")
      ),
    }
  }
}

impl Target {
  pub fn language(&self) -> Option<&'static str> {
    match self.file_path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
      "cs" => Some("csharp"),
      "rs" => Some("rust"),
      _ => None,
    }
  }
  pub fn base_name(&self) -> &str {
    let suffix = format!(".{}", self.file_path.extension().unwrap().to_string_lossy());
    if self.name.to_ascii_lowercase().ends_with(&suffix.to_ascii_lowercase()) {
      &self.name[..self.name.len() - suffix.len()]
    } else {
      &self.name
    }
  }
  pub fn qualified_name(&self) -> String {
    let extension = self.file_path.extension().unwrap().to_string_lossy();
    if self.name.to_ascii_lowercase().ends_with(&format!(".{extension}")) {
      self.name.clone()
    } else {
      format!("{}.{extension}", self.name)
    }
  }
  fn aliases(&self) -> Vec<String> {
    let mut aliases = Vec::new();
    if let Some(name) = &self.short_name
      && !["help", "completion", "__complete", "__exec"]
        .iter()
        .any(|r| name.eq_ignore_ascii_case(r))
    {
      aliases.push(name.clone());
    }
    if let Some(short) = &self.short_name
      && !short.contains('.')
    {
      aliases.push(format!("{short}.{}", self.file_path.extension().unwrap().to_string_lossy()));
    }
    let qualified = self.qualified_name();
    for name in [self.name.as_str(), qualified.as_str()] {
      let parts: Vec<_> = name.split('/').collect();
      if parts.len() > 2 {
        aliases.push(parts[parts.len() - 2..].join("/"));
      }
      if parts.len() > 1 {
        aliases.push(parts[parts.len() - 2..].join("-"));
      }
      if parts.len() > 1 {
        aliases.push(parts[parts.len() - 1].into());
      }
      let base = if name == qualified {
        self.base_name()
      } else {
        name
      };
      if base != name {
        aliases.push(base.into());
        let base_parts: Vec<_> = base.split('/').collect();
        if base_parts.len() > 2 {
          aliases.push(base_parts[base_parts.len() - 2..].join("/"));
        }
        if base_parts.len() > 1 {
          aliases.push(base_parts[base_parts.len() - 2..].join("-"));
          aliases.push(base_parts[base_parts.len() - 1].into());
        }
        if let Some(short) = &self.short_name {
          aliases.push(crate::shared_files::stem(short).into());
        }
      }
    }
    aliases.retain(|s| {
      !["help", "completion", "__complete", "__exec"]
        .iter()
        .any(|r| s.eq_ignore_ascii_case(r))
    });
    aliases
  }
}

pub(crate) fn arguments(parameters: &Value) -> Result<Vec<String>> {
  let Some(parameters) = parameters.as_object() else {
    bail!("ExecTargetAsync parameters must be an object with named properties.");
  };
  if parameters.values().any(|v| v.is_null() || v.is_array() || v.is_object()) {
    bail!("Target arguments must be strings, numbers, or booleans.");
  }
  Ok(
    parameters
      .iter()
      .map(|(key, value)| {
        format!(
          "{key}={}",
          match value {
            Value::String(s) => s.clone(),
            _ => value.to_string(),
          }
        )
      })
      .collect(),
  )
}

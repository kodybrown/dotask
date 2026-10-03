use anyhow::{Result, bail};
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
        let (task_directory, root_directory) = if let Some(selected) = selected {
            let task = portable_path(&invocation, selected)?;
            if !task.is_dir() {
                bail!("Task directory does not exist: {}", task.display());
            }
            let anchor = if task.join("config.yaml").is_file() {
                None
            } else {
                task.ancestors().find(|p| p.join(".dotasks.yaml").is_file())
            };
            let root = anchor
                .unwrap_or_else(|| task.parent().unwrap_or(&task))
                .to_path_buf();
            (task, root)
        } else {
            let root = invocation
                .ancestors()
                .find(|p| p.join(".dotasks.yaml").is_file() || p.join(".tasks").is_dir());
            let Some(root) = root else {
                bail!(
                    "No .dotasks.yaml or .tasks directory found. Use the existing C# CLI to initialize a project."
                );
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

fn portable_path(root: &Path, value: &str) -> Result<PathBuf> {
    if value.contains('\0') {
        bail!("A path cannot contain a NUL character.");
    }
    #[cfg(unix)]
    if value.starts_with("\\\\")
        || (value.len() >= 2
            && value.as_bytes()[0].is_ascii_alphabetic()
            && value.as_bytes()[1] == b':')
    {
        bail!("Windows rooted path '{value}' cannot be resolved on this host.");
    }
    let path = root.join(value.replace(['/', '\\'], std::path::MAIN_SEPARATOR_STR));
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
    pub run: String,
    pub optional: bool,
    pub parameters: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Catalog {
    pub configuration: Value,
    pub targets: Vec<Target>,
}

impl Catalog {
    pub fn find(&self, name: &str) -> Result<Option<&Target>> {
        let exact: Vec<_> = self
            .targets
            .iter()
            .filter(|t| t.name.eq_ignore_ascii_case(name))
            .collect();
        let matches = if exact.is_empty() {
            self.targets
                .iter()
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
    fn aliases(&self) -> Vec<String> {
        let mut aliases = Vec::new();
        if let Some(name) = &self.short_name
            && !["help", "completion", "__complete", "__exec"]
                .iter()
                .any(|r| name.eq_ignore_ascii_case(r))
        {
            aliases.push(name.clone());
        }
        let parts: Vec<_> = self.name.split('/').collect();
        if parts.len() > 2 {
            aliases.push(parts[parts.len() - 2..].join("/"));
        }
        if parts.len() > 1 {
            aliases.push(parts[parts.len() - 2..].join("-"));
        }
        aliases
    }
}

pub(crate) fn arguments(parameters: &Value) -> Result<Vec<String>> {
    let Some(parameters) = parameters.as_object() else {
        bail!("ExecTargetAsync parameters must be an object with named properties.");
    };
    if parameters
        .values()
        .any(|v| v.is_null() || v.is_array() || v.is_object())
    {
        bail!("Target arguments must be strings, numbers, or booleans.");
    }
    Ok(parameters
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
        .collect())
}

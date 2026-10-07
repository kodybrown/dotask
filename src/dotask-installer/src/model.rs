use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Package {
  pub schema: u32,
  pub application: Application,
  pub platform: String,
  pub architecture: String,
  pub payload: String,
  pub runtime: Runtime,
}

/// Options used on the destination machine. The creator preserves this layout
/// when writing the package, so source and distributed YAML share one schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Runtime {
  #[serde(default = "interactive_default")]
  pub interactive: bool,
  #[serde(default)]
  pub inputs: BTreeMap<String, Input>,
  #[serde(default)]
  pub values: BTreeMap<String, serde_json::Value>,
  #[serde(default)]
  pub defaults: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
  #[serde(default)]
  pub profiles: BTreeMap<String, Profile>,
  #[serde(default)]
  pub commands: Vec<Command>,
  #[serde(default)]
  pub shortcuts: Vec<Shortcut>,
  #[serde(default)]
  pub settings: Vec<String>,
}

fn interactive_default() -> bool {
  true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Application {
  pub id: String,
  pub name: String,
  pub version: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub build: Option<Build>,
  #[serde(default)]
  pub author: String,
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub copyright: String,
  #[serde(default)]
  pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  pub stamp: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub commit: Option<String>,
  #[serde(default)]
  pub dirty: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
  #[serde(rename = "type")]
  pub kind: String,
  #[serde(default)]
  pub required: bool,
  #[serde(default)]
  pub prompt: Option<String>,
  #[serde(default)]
  pub default: Option<serde_json::Value>,
  #[serde(default)]
  pub choices: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
  #[serde(default)]
  pub detect: Vec<String>,
  pub values: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
  pub name: String,
  pub executable: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Shortcut {
  pub name: String,
  pub executable: String,
  #[serde(default)]
  pub arguments: Vec<String>,
  #[serde(default)]
  pub icon: Option<String>,
  #[serde(default)]
  pub working_directory: Option<String>,
  #[serde(default)]
  pub terminal: bool,
  #[serde(default)]
  pub desktop: bool,
  #[serde(default)]
  pub start_menu: bool,
  #[serde(default)]
  pub local: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRecord {
  pub sha256: String,
  pub mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedFile {
  pub path: String,
  pub hash: Option<String>,
  pub link: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct BuildReceipt {
  pub schema: u32,
  pub app_id: String,
  pub build: String,
  pub files: BTreeMap<String, FileRecord>,
  pub config_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Receipt {
  pub schema: u32,
  pub app_id: String,
  pub root: String,
  pub active: String,
  pub previous: Option<String>,
  pub builds: BTreeMap<String, String>,
  pub launchers: Vec<OwnedFile>,
  #[serde(default)]
  pub shortcut_directories: Vec<String>,
  pub settings: Vec<String>,
  pub values: BTreeMap<String, serde_json::Value>,
  pub engine_hash: String,
  pub config_hash: String,
  #[serde(default)]
  pub uninstalling: bool,
}

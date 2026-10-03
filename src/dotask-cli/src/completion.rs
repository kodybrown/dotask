use crate::{
  configuration,
  project::{Catalog, Directory},
  shared::{Store, Tracking},
};
use anyhow::{bail, Result};
use serde_json::Value;
use std::path::Path;

pub(crate) fn script(shell: &str) -> Result<i32> {
  // Both runners serve the same shell protocol. Keep one canonical script per
  // shell until the maintained C# reference CLI is retired.
  let script = match shell.to_ascii_lowercase().as_str() {
    "bash" => include_str!("../../Dotask.Cli/Completion/Scripts/bash"),
    "zsh" => include_str!("../../Dotask.Cli/Completion/Scripts/zsh"),
    "fish" => include_str!("../../Dotask.Cli/Completion/Scripts/fish"),
    "powershell" => include_str!("../../Dotask.Cli/Completion/Scripts/powershell"),
    _ => bail!("Choose a completion shell: bash, zsh, fish, or powershell."),
  };
  print!("{script}");
  Ok(0)
}
#[derive(Clone)]
struct Candidate {
  value: String,
  kind: &'static str,
  description: String,
}
fn candidate(value: impl Into<String>, kind: &'static str, description: impl Into<String>) -> Candidate {
  Candidate {
    value: value.into(),
    kind,
    description: description.into(),
  }
}
fn globals() -> Vec<Candidate> {
  [
    ("--use-dir", "Select a task directory"),
    ("--help", "Show help"),
    ("--verbose", "Show full summaries and execution diagnostics"),
    ("--version", "Show version"),
    ("--init", "Initialize a project in the current directory"),
    ("--create-task", "Interactively create a YAML task group"),
    ("--list", "List shared tasks"),
    ("--save", "Download shared tasks"),
    ("--add", "Add shared project tasks"),
    ("--sync", "Sync shared project tasks"),
    ("--remove", "Remove unchanged shared project tasks"),
  ]
  .into_iter()
  .map(|(v, d)| candidate(v, "option", d))
  .collect()
}
pub(crate) fn query(args: &[String]) -> Result<i32> {
  let mut line = String::new();
  let mut shell = "bash";
  let mut position: Option<i64> = None;
  if !args.len().is_multiple_of(2) {
    return Ok(0);
  }
  for pair in args.chunks(2) {
    match pair[0].as_str() {
      "--line" => line = pair[1].clone(),
      "--shell" => shell = &pair[1],
      "--position" => {
        let Ok(p) = pair[1].parse() else {
          return Ok(0);
        };
        position = Some(p);
      }
      _ => return Ok(0),
    }
  }
  if let Some(p) = position {
    let p = p.max(0) as usize;
    line = match shell {
      "bash" => String::from_utf8_lossy(&line.as_bytes()[..p.min(line.len())]).into_owned(),
      "zsh" => line.chars().take(p).collect(),
      _ => String::from_utf16_lossy(&line.encode_utf16().take(p).collect::<Vec<_>>()),
    };
  }
  let candidates = complete(&line, &std::env::current_dir()?, shell)?;
  for c in candidates {
    println!("{}\t{}\t{}", c.value, c.kind, c.description.replace(['\t', '\r', '\n'], " "));
  }
  Ok(0)
}
fn words(line: &str, shell: &str) -> Vec<String> {
  // This is tokenization only. Never expand variables, substitutions or globs.
  let ps = shell.eq_ignore_ascii_case("powershell");
  let escape = if ps { '`' } else { '\\' };
  let chars: Vec<_> = line.chars().collect();
  let mut index = 0;
  let mut quote = '\0';
  let mut started = false;
  let mut word = String::new();
  let mut result = vec![];
  while index < chars.len() {
    let c = chars[index];
    let next = chars.get(index + 1).copied();
    if c == escape
      && quote != '\''
      && let Some(next) = next
      && (quote != '"' || ps || ['"', '\\', '$', '`', '\n'].contains(&next))
    {
      word.push(next);
      index += 2;
      started = true;
      continue;
    }
    if quote != '\0' {
      if c == quote {
        if ps && quote == '\'' && next == Some('\'') {
          word.push('\'');
          index += 1;
        } else {
          quote = '\0';
        }
      } else {
        word.push(c);
      }
    } else if c == '\'' || c == '"' {
      quote = c;
      started = true;
    } else if [';', '|', '&'].contains(&c) {
      result.clear();
      word.clear();
      started = false;
    } else if c.is_whitespace() {
      if started || !word.is_empty() {
        result.push(std::mem::take(&mut word));
        started = false;
      }
    } else {
      word.push(c);
      started = true;
    }
    index += 1;
  }
  result.push(word);
  result
}
fn filter(mut candidates: Vec<Candidate>, prefix: &str) -> Vec<Candidate> {
  candidates
    .retain(|c| c.value.to_lowercase().starts_with(&prefix.to_lowercase()) && !c.value.chars().any(char::is_control));
  candidates.sort_by_key(|c| c.value.to_lowercase());
  candidates.dedup_by(|a, b| a.value.eq_ignore_ascii_case(&b.value));
  candidates
}
fn prefix(mut candidates: Vec<Candidate>, prefix: &str) -> Vec<Candidate> {
  for c in &mut candidates {
    c.value = format!("{prefix}{}", c.value);
  }
  candidates
}
fn paths(prefix: &str, root: &Path, directories: bool) -> Vec<Candidate> {
  let result = (|| -> Result<Vec<Candidate>> {
    let normalized = prefix.replace('\\', "/");
    let (parent, name) = normalized
      .rsplit_once('/')
      .map(|(p, n)| (format!("{p}/"), n))
      .unwrap_or((String::new(), normalized.as_str()));
    let path = if let Some(rest) = parent.strip_prefix("~/") {
      let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .ok_or_else(|| anyhow::anyhow!("Home unavailable"))?;
      crate::project::portable_path(Path::new(&home), rest)?
    } else {
      crate::project::portable_path(root, &parent)?
    };
    let mut out = vec![];
    for entry in std::fs::read_dir(path)? {
      let entry = entry?;
      let n = entry.file_name().to_string_lossy().into_owned();
      let dir = entry.path().is_dir();
      if (if cfg!(windows) {
        n.to_lowercase().starts_with(&name.to_lowercase())
      } else {
        n.starts_with(name)
      }) && (!directories || dir)
        && !n.chars().any(char::is_control)
      {
        out.push(candidate(
          format!("{parent}{n}{}", if dir { "/" } else { "" }),
          if dir { "directory" } else { "file" },
          if dir { "Directory" } else { "File" },
        ));
      }
    }
    out.sort_by(|a, b| a.value.cmp(&b.value));
    Ok(out)
  })();
  result.unwrap_or_default()
}
fn values(option: &Value, current: &str, root: &Path) -> Vec<Candidate> {
  let choices = option["Choices"].as_array().unwrap();
  let description = option["Description"].as_str().unwrap_or_default();
  if !choices.is_empty() {
    return filter(
      choices
        .iter()
        .filter_map(Value::as_str)
        .map(|s| candidate(s, "value", description))
        .collect(),
      current,
    );
  }
  if option["Type"] == "bool" {
    return filter(
      ["true", "false"]
        .into_iter()
        .map(|s| candidate(s, "value", description))
        .collect(),
      current,
    );
  }
  if !option["Completion"].is_null() || option["Type"] == "path" {
    paths(current, root, option["Completion"] == "directory")
  } else {
    vec![]
  }
}
fn complete(line: &str, invocation: &Path, shell: &str) -> Result<Vec<Candidate>> {
  let mut words: Vec<_> = words(line, shell).into_iter().skip(1).collect();
  let current = words.pop().unwrap_or_default();
  let mut selected = None;
  let mut index = 0;
  while index < words.len() {
    if words[index].eq_ignore_ascii_case("--use-dir") {
      if index + 1 == words.len() {
        return Ok(paths(&current, invocation, true));
      }
      selected = Some(words.remove(index + 1));
      words.remove(index);
    } else if words[index].to_lowercase().starts_with("--use-dir=") {
      selected = Some(words.remove(index)[10..].to_owned());
    } else {
      index += 1;
    }
  }
  words.retain(|s| !s.eq_ignore_ascii_case("--verbose"));
  if current.to_lowercase().starts_with("--use-dir=") {
    return Ok(prefix(paths(&current[10..], invocation, true), &current[..10]));
  }
  let action = words.first().map(|s| s.to_lowercase()).unwrap_or_default();
  if action == "completion" {
    return Ok(filter(
      ["bash", "zsh", "fish", "powershell"]
        .into_iter()
        .map(|s| candidate(s, "value", "Shell integration"))
        .collect(),
      &current,
    ));
  }
  if action == "--init" || action == "--create-task" {
    return Ok(filter(
      globals()
        .into_iter()
        .filter(|c| {
          ["--help", "--version", "--verbose"].contains(&c.value.as_str())
            || c.value == "--use-dir" && selected.is_none()
        })
        .collect(),
      &current,
    ));
  }
  let directory = Directory::locate(invocation.into(), selected.as_deref()).ok();
  // Management completion does not inspect project code or configuration.
  if crate::shared::COMMANDS.contains(&action.as_str()) {
    let mut candidates = vec![];
    if action == "--remove" || action == "--sync" {
      if let Some(d) = &directory {
        let read = (|| -> Result<Tracking> {
          let path = crate::shared_files::resolve(&d.root_directory, ".dotasks-lock.yaml")?;
          let bytes = if path.is_file() {
            Some(std::fs::read(path)?)
          } else {
            None
          };
          Tracking::read(
            bytes.as_deref(),
            &d.task_directory
              .strip_prefix(&d.root_directory)?
              .to_string_lossy()
              .replace('\\', "/"),
          )
        })();
        if let Ok(t) = read {
          candidates.extend(t.ids().map(|id| candidate(id, "target", "Installed shared task")));
        }
      }
    } else if let Ok(entries) = Store::environment().and_then(|s| s.cached()) {
      candidates.extend(entries.into_iter().map(|(id, d)| candidate(id, "target", d)));
    }
    if ["--add", "--sync", "--remove"].contains(&action.as_str()) {
      candidates.push(candidate("--dry-run", "option", "Preview without changing project files"));
    }
    if action == "--sync" {
      candidates.push(candidate("--accept-merge", "option", "Record an already reviewed manual merge"));
    }
    candidates.push(candidate(
      "--verbose",
      "option",
      "Show full summaries and execution diagnostics",
    ));
    return Ok(filter(candidates, &current));
  }
  let session = tempfile::tempdir()?;
  let catalog = directory
    .as_ref()
    .and_then(|d| crate::catalog::load(&d.task_directory, session.path()).ok())
    .unwrap_or(Catalog {
      configuration: Value::Null,
      targets: vec![],
    });
  if words.is_empty() || action == "help" {
    let mut candidates: Vec<_> = catalog
      .targets
      .iter()
      .flat_map(|t| {
        catalog
          .names(t)
          .into_iter()
          .map(|n| candidate(n, "target", t.error.as_ref().unwrap_or(&t.description)))
      })
      .collect();
    if words.is_empty() {
      candidates.extend(globals());
      candidates.push(candidate("help", "command", "Show target help"));
      candidates.push(candidate("completion", "command", "Print shell integration"));
    }
    candidates.push(candidate(
      "--verbose",
      "option",
      "Show full summaries and execution diagnostics",
    ));
    return Ok(filter(candidates, &current));
  }
  let Some(target) = catalog.find(&words[0]).ok().flatten() else {
    return Ok(filter(globals(), &current));
  };
  let root = &directory.as_ref().unwrap().root_directory;
  if let Some((name, value)) = current.split_once('=') {
    return Ok(
      configuration::option(target, name.trim_start_matches('-'))
        .map(|o| prefix(values(o, value, root), &current[..name.len() + 1]))
        .unwrap_or_default(),
    );
  }
  let mut used = std::collections::BTreeSet::new();
  let mut pending: Option<&Value> = None;
  for word in &words[1..] {
    if let Some(p) = pending.take()
      && (p["Type"] != "bool" || configuration::boolean(word).is_some())
    {
      continue;
    }
    let key = word.split('=').next().unwrap().trim_start_matches('-');
    if let Some(option) = configuration::option(target, key) {
      used.insert(option["Name"].as_str().unwrap());
      if !word.contains('=') && word.starts_with('-') {
        pending = Some(option);
      }
    }
  }
  if let Some(p) = pending
    && (p["Type"] != "bool" || !current.starts_with('-'))
  {
    return Ok(values(p, &current, root));
  }
  let mut candidates = globals();
  for o in &target.options {
    let name = o["Name"].as_str().unwrap();
    if used.contains(name) {
      continue;
    }
    let description = o["Description"].as_str().unwrap_or_default();
    candidates.push(candidate(format!("--{name}"), "option", description));
    candidates.push(candidate(format!("{name}="), "option", description));
    if let Some(alias) = o["Alias"].as_str() {
      candidates.push(candidate(format!("-{alias}"), "option", description));
    }
  }
  Ok(filter(candidates, &current))
}

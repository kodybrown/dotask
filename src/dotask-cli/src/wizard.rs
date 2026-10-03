use crate::{
  catalog,
  cli::Arguments,
  configuration, process,
  project::{Catalog, Directory, Step},
  shared_files as files,
};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{
  io::{self, BufRead, IsTerminal, Write},
  path::{Path, PathBuf},
  sync::mpsc::{self, Receiver, RecvTimeoutError},
  time::Duration,
};

#[derive(Debug)]
struct Back;
impl std::fmt::Display for Back {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("Unfinished step discarded; previously added steps kept.")
  }
}
impl std::error::Error for Back {}
struct Input {
  lines: Receiver<io::Result<String>>,
  adding: bool,
}
impl Input {
  fn ask(&self, prompt: &str) -> Result<String> {
    process::check_cancelled()?;
    if self.adding {
      print!("{} [:back discards this step] ", prompt.trim_end());
    } else {
      print!("{prompt}");
    }
    io::stdout().flush()?;
    loop {
      process::check_cancelled()?;
      match self.lines.recv_timeout(Duration::from_millis(50)) {
        Ok(line) => {
          let line = line?;
          if line == ":cancel" {
            bail!(process::Cancelled);
          }
          if self.adding && line.trim().eq_ignore_ascii_case(":back") {
            bail!(Back);
          }
          return Ok(line);
        }
        Err(RecvTimeoutError::Timeout) => {}
        Err(_) => bail!(process::Cancelled),
      }
    }
  }
  fn yes_no(&self, prompt: &str, default: bool) -> Result<bool> {
    loop {
      match self.ask(prompt)?.trim().to_lowercase().as_str() {
        "" => return Ok(default),
        "y" | "yes" => return Ok(true),
        "n" | "no" => return Ok(false),
        _ => println!("Enter yes or no."),
      }
    }
  }
  fn parameter(&self, option: &Value, root: &Path, required: bool) -> Result<Option<Value>> {
    loop {
      let name = option["Name"].as_str().unwrap();
      let value = self.ask(&format!(
        "  {name} value ({}; :empty for empty string): ",
        if required {
          "required"
        } else {
          "Enter keeps default"
        }
      ))?;
      if value.is_empty() {
        if required {
          continue;
        }
        return Ok(None);
      }
      let value = if value == ":empty" { "" } else { &value };
      match configuration::convert(option, value, root) {
        Ok(converted) => {
          return Ok(Some(if option["Type"] == "path" {
            Value::String(value.into())
          } else {
            converted
          }))
        }
        Err(e) => println!("{e}"),
      }
    }
  }
}
pub(crate) fn run(command: &Arguments) -> Result<i32> {
  if command.help {
    println!("Usage: dotask --create-task [--use-dir PATH]\nInteractively create a YAML .task group: choose tasks and parameters, reorder steps, preview, and save.\nRequires a terminal. Enter :cancel or press Ctrl+C to cancel. Existing files are never overwritten.");
    return Ok(0);
  }
  if command.remaining.len() != 1 {
    bail!("Usage: dotask --create-task [--use-dir PATH]");
  }
  if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
    bail!("--create-task requires an interactive terminal. Create a .task YAML file directly in scripts; run --create-task --help for usage.");
  }
  let directory = Directory::locate(std::env::current_dir()?, command.use_directory.as_deref())?;
  // Only this thread reads stdin. The foreground polls cancellation while an
  // input read blocks, keeping Ctrl+C responsive without consuming task input.
  let (sender, lines) = mpsc::channel();
  std::thread::spawn(move || {
    for line in io::stdin().lock().lines() {
      if sender.send(line).is_err() {
        break;
      }
    }
  });
  interact(
    &mut Input {
      lines,
      adding: false,
    },
    &directory,
  )?;
  Ok(0)
}
fn destination(directory: &Directory, name: &str) -> Result<PathBuf> {
  if !catalog::identifier(name) {
    bail!("Use a name starting with a letter, followed by letters, digits, underscores, or hyphens.");
  }
  if ["help", "completion", "__complete", "__exec"]
    .iter()
    .any(|s| name.eq_ignore_ascii_case(s))
  {
    bail!("Invalid or reserved target name '{name}'.");
  }
  let path = files::resolve(&directory.task_directory, &format!("{name}.task"))?;
  let session = tempfile::tempdir()?;
  if path.exists()
    || catalog::load(&directory.task_directory, session.path())?
      .targets
      .iter()
      .any(|t| t.name.eq_ignore_ascii_case(name))
  {
    bail!("A file or target named '{name}' already exists. Choose another name.");
  }
  Ok(path)
}
fn interact(input: &mut Input, directory: &Directory) -> Result<()> {
  let session = tempfile::tempdir()?;
  let catalog = catalog::load(&directory.task_directory, session.path())?;
  let config = configuration::load(directory)?;
  println!("Create a YAML task group. Enter :cancel or press Ctrl+C to cancel. Nothing is written until save.");
  let (name, path) = loop {
    let name = input.ask("Task name (without extension): ")?.trim().to_owned();
    match destination(directory, &name) {
      Ok(path) => break (name, path),
      Err(e) => println!("{e}"),
    }
  };
  println!("Destination: {}", path.display());
  let description = input.ask("Description (Enter to omit): ")?;
  let mut steps = vec![];
  while input.yes_no("Add a step? [Y/n]: ", true)? {
    input.adding = true;
    let result = step(input, &catalog, &config, &directory.root_directory, &name);
    input.adding = false;
    match result {
      Ok(s) => steps.push(s),
      Err(e) if e.is::<Back>() => println!("{e}"),
      Err(e) => return Err(e),
    }
  }
  for (i, s) in steps.iter().enumerate() {
    println!("  {}. {}{}", i + 1, s.run, if s.optional { " (optional)" } else { "" });
  }
  if steps.len() > 1 {
    loop {
      let order = input.ask("Step order (comma-separated numbers; Enter keeps this order): ")?;
      if order.is_empty() {
        break;
      }
      let indices: Option<Vec<_>> = order
        .split(',')
        .map(|s| s.trim().parse::<usize>().ok().and_then(|i| i.checked_sub(1)))
        .collect();
      if let Some(indices) = indices
        && indices.len() == steps.len()
        && indices.iter().collect::<std::collections::BTreeSet<_>>().len() == steps.len()
        && indices.iter().all(|i| *i < steps.len())
      {
        steps = indices.iter().map(|i| steps[*i].clone()).collect();
        break;
      }
      println!("List every step number exactly once.");
    }
  }
  let required = input.yes_no("Require at least one step to execute? [y/N]: ", false)?;
  let yaml = serialize(&description, required, &steps)?;
  println!("\nYAML preview:\n{yaml}");
  if !input.yes_no("Save this file? [y/N]: ", false)? {
    println!("Cancelled; no file created.");
    return Ok(());
  }
  process::check_cancelled()?;
  let path = destination(directory, &name)?;
  files::atomic(&path, yaml.as_bytes(), false)?;
  println!(
    "Created {}. Run dotask {name} (include --use-dir when using a custom task directory).",
    path.display()
  );
  Ok(())
}
fn step(input: &Input, catalog: &Catalog, config: &Value, root: &Path, group: &str) -> Result<Step> {
  let (run, target) = loop {
    let search = input.ask("Search task names/descriptions (Enter lists all; :manual enters an absent task): ")?;
    if search == ":manual" {
      let run = input.ask("Task name: ")?.trim().to_owned();
      if run.is_empty() || run.eq_ignore_ascii_case(group) {
        println!("Enter a nonempty task name other than this group.");
        continue;
      }
      match catalog.find(&run) {
        Ok(Some(t)) if t.error.is_some() => println!("{}", t.error.as_ref().unwrap()),
        Ok(Some(t)) => break (t.name.clone(), Some(t)),
        Ok(None) => break (run, None),
        Err(e) => println!("{e}"),
      }
    } else {
      let matches: Vec<_> = catalog
        .targets
        .iter()
        .filter(|t| {
          t.error.is_none()
            && (t.name.to_lowercase().contains(&search.to_lowercase())
              || t.description.to_lowercase().contains(&search.to_lowercase()))
        })
        .collect();
      for (i, t) in matches.iter().enumerate() {
        println!("  {}. {} — {}", i + 1, t.name, t.description);
      }
      if matches.is_empty() {
        println!("No matching tasks. Search again or use :manual.");
        continue;
      }
      let selection = input.ask("Task number (Enter to search again): ")?;
      if let Some(t) = selection
        .parse::<usize>()
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|i| matches.get(i))
      {
        break (t.name.clone(), Some(*t));
      }
    }
  };
  let optional = input.yes_no("Optional (skip only when absent)? [y/N]: ", false)?;
  let mut parameters = serde_json::Map::new();
  if let Some(target) = target {
    let defaults = Value::Object(configuration::defaults(config, &target.name)?);
    for option in &target.options {
      let name = option["Name"].as_str().unwrap();
      println!(
        "  {name} ({}) — {}",
        option["Type"].as_str().unwrap(),
        option["Description"].as_str().unwrap_or_default()
      );
      let choices = option["Choices"].as_array().unwrap();
      if !choices.is_empty() {
        println!(
          "  Choices: {}",
          choices.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")
        );
      }
      let effective = configuration::get(&defaults, name).or_else(|| configuration::get(option, "Default"));
      println!(
        "  Default: {}",
        effective.and_then(Value::as_str).unwrap_or(if option["Type"] == "bool" {
          "false"
        } else {
          "none"
        })
      );
      if let Some(value) = input.parameter(option, root, option["Required"] == true && effective.is_none())? {
        parameters.insert(name.into(), value);
      }
    }
    // Validate metadata/defaults, but never check tools or execute requirements
    // while authoring. Required values were checked by each parameter prompt.
    configuration::bind(
      target,
      config,
      &crate::project::arguments(&Value::Object(parameters.clone()))?,
      root,
      true,
    )?;
  } else {
    println!("Task is not installed; its parameter names and types cannot be checked.");
    while input.yes_no("Add a parameter? [y/N]: ", false)? {
      let key = input.ask("Parameter name: ")?.trim().to_owned();
      if !crate::catalog::identifier(&key) || parameters.keys().any(|k| k.eq_ignore_ascii_case(&key)) {
        println!("Use a unique parameter name starting with a letter.");
        continue;
      }
      let kind = input.ask("Type [string/bool/int/number] (Enter for string): ")?;
      let kind = if kind.is_empty() { "string" } else { &kind };
      if !["string", "bool", "int", "number"].contains(&kind) {
        println!("Unknown type.");
        continue;
      }
      let option = json!({"Name":key,"Type":kind,"Choices":[]});
      parameters.insert(key, input.parameter(&option, root, true)?.unwrap());
    }
  }
  Ok(Step {
    run,
    optional,
    parameters: Value::Object(parameters),
  })
}
fn serialize(description: &str, required: bool, steps: &[Step]) -> Result<String> {
  let mut yaml = String::new();
  if !description.is_empty() {
    yaml += &format!("description: {}\n", serde_json::to_string(description)?);
  }
  if required {
    yaml += "require_at_least_1_step: true\n";
  }
  yaml += if steps.is_empty() {
    "steps: []\n"
  } else {
    "steps:\n"
  };
  for step in steps {
    yaml += &format!("  - run: {}\n", serde_json::to_string(&step.run)?);
    if step.optional {
      yaml += "    optional: true\n";
    }
    if !step.parameters.as_object().unwrap().is_empty() {
      yaml += "    with:\n";
      for (k, v) in step.parameters.as_object().unwrap() {
        yaml += &format!("      {}: {v}\n", serde_json::to_string(k)?);
      }
    }
  }
  Ok(yaml)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn fixture() -> (tempfile::TempDir, Directory) {
    let root = tempfile::tempdir().unwrap();
    let tasks = root.path().join(".tasks");
    std::fs::create_dir(&tasks).unwrap();
    std::fs::write(tasks.join("existing.task"), "steps: []\n").unwrap();
    let directory = Directory {
      root_directory: root.path().into(),
      task_directory: tasks,
      invocation_directory: root.path().into(),
    };
    (root, directory)
  }

  fn input(lines: &[&str]) -> Input {
    let (sender, receiver) = mpsc::channel();
    for line in lines {
      sender.send(Ok((*line).into())).unwrap();
    }
    Input {
      lines: receiver,
      adding: false,
    }
  }

  #[test]
  fn wizard_keeps_completed_steps_when_back_discards_a_partial_step() {
    let (_root, directory) = fixture();
    let mut input = input(&[
      "existing", "all", "Example", "y", ":manual", "missing", "y", "y", "message", "string", "hello", "n", "y",
      ":back", "n", "y", "y",
    ]);
    interact(&mut input, &directory).unwrap();
    let source = std::fs::read_to_string(directory.task_directory.join("all.task")).unwrap();
    let value = crate::yaml::parse(&source, "all.task").unwrap();
    assert_eq!(value["steps"].as_array().unwrap().len(), 1);
    assert_eq!(value["steps"][0]["run"], "missing");
    assert_eq!(value["steps"][0]["with"]["message"], "hello");
    assert_eq!(value["require_at_least_1_step"], true);
    assert_eq!(
      std::fs::read_to_string(directory.task_directory.join("existing.task")).unwrap(),
      "steps: []\n"
    );
  }

  #[test]
  fn wizard_decline_cancel_and_eof_do_not_write_files() {
    for lines in [
      &["all", "", "n", "n", "n"][..],
      &["all", ":cancel"],
      &["all"],
    ] {
      let (_root, directory) = fixture();
      let result = interact(&mut input(lines), &directory);
      if lines.len() != 5 {
        assert!(result.unwrap_err().is::<process::Cancelled>());
      } else {
        result.unwrap();
      }
      assert_eq!(std::fs::read_dir(directory.task_directory).unwrap().count(), 1);
    }
  }
}

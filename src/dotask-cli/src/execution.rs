use crate::host::{write_json, Host};
use crate::process::{self, Cancelled};
use crate::project::{arguments, Catalog, Directory, Target};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Context {
  #[serde(flatten)]
  pub directory: Directory,
  target_file: PathBuf,
  target_name: String,
  cli_executable: PathBuf,
  cli_arguments: Vec<String>,
  session_directory: PathBuf,
  settings: Value,
  parameters: Value,
  target_defaults: Value,
  verbose: bool,
  call_chain: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Call {
  context: Context,
  target: String,
  parameters: Value,
  operation: u8,
}

pub(crate) struct Executor {
  pub directory: Directory,
  pub configuration: Value,
  pub verbose: bool,
}

impl Executor {
  pub fn catalog(&self, session: &Path) -> Result<Catalog> {
    let mut catalog = crate::catalog::load(&self.directory.task_directory, session)?;
    catalog.configuration = if self.configuration.is_null() {
      crate::configuration::load(&self.directory)?
    } else {
      self.configuration.clone()
    };
    Ok(catalog)
  }

  pub fn bind(&self, target: &Target, args: &[String], _session: &Path, help: bool) -> Result<Value> {
    crate::configuration::bind(target, &self.configuration, args, &self.directory.root_directory, help)
  }

  pub fn execute(&self, target: &Target, args: &[String], chain: &[String], session: &Path) -> Result<i32> {
    process::check_cancelled()?;
    if self.directory.task_directory.join(".dotask/transaction/journal.json").is_file() {
      bail!(
                "A shared-task update is incomplete. Run the C# CLI's dotask --sync to recover it before executing project tasks."
            );
    }
    if let Some(error) = &target.error {
      bail!("{error}");
    }
    if chain.iter().any(|name| name.eq_ignore_ascii_case(&target.name)) {
      bail!("Target cycle: {} -> {}", chain.join(" -> "), target.name);
    }
    let mut chain = chain.to_vec();
    chain.push(target.name.clone());
    self.trace(&format!("Target: {}; source: {}", target.name, target.file_path.display()));
    let parameters = self.bind(target, args, session, false)?;
    if let Some(group) = &target.group {
      let mut executed = 0;
      for step in &group.steps {
        let catalog = self.catalog(session)?;
        let child = catalog.find(&step.run)?;
        if child.is_none() && step.optional {
          self.trace(&format!("Skipping absent optional target: {}", step.run));
          continue;
        }
        let child = child.ok_or_else(|| anyhow::anyhow!("Unknown target '{}'.", step.run))?;
        let code = self.execute(child, &arguments(&step.parameters)?, &chain, session)?;
        if code != 0 {
          return Ok(code);
        }
        executed += 1;
      }
      if executed == 0 && group.require_at_least_one_step {
        bail!(
          "Target '{}' requires at least one step to execute; no steps were executed.",
          target.name
        );
      }
      self.trace(&format!("Completed group: {}; executed steps: {executed}", target.name));
      return Ok(0);
    }

    let snapshot = tempfile::Builder::new().prefix("execution-").tempdir_in(session)?;
    self.trace(&format!("Compiling: {}", target.name));
    let host = Host::locate()?;
    let compiled: Value = host.request(
      session,
      json!({
          "Operation": "compile", "TaskDirectory": self.directory.task_directory,
          "RootDirectory": self.directory.root_directory, "Target": target,
          "SnapshotDirectory": snapshot.path(),
      }),
    )?;
    let assembly = compiled["AssemblyPath"]
      .as_str()
      .ok_or_else(|| anyhow::anyhow!("Missing compiler output path."))?;
    let context = Context {
      directory: self.directory.clone(),
      target_file: target.file_path.clone(),
      target_name: target.name.clone(),
      cli_executable: std::env::current_exe()?,
      cli_arguments: Vec::new(),
      session_directory: session.to_path_buf(),
      settings: self.configuration["Settings"].clone(),
      parameters,
      target_defaults: self.configuration["TargetDefaults"].clone(),
      verbose: self.verbose,
      call_chain: chain,
    };
    let context_file = snapshot.path().join("context.json");
    write_json(&context_file, &context)?;
    self.trace(&format!("Executing: {}", target.name));
    let code = process::run(
      Command::new(&host.dotnet)
        .arg(assembly)
        .current_dir(&self.directory.root_directory)
        .env("DOTASK_EXECUTION_CONTEXT", &context_file),
    )?;
    if code != 0 {
      eprintln!("Target '{}' failed with exit code {code}.", target.name);
    }
    self.trace(&format!("Completed: {}; exit code: {code}", target.name));
    Ok(code)
  }

  fn trace(&self, message: &str) {
    if self.verbose {
      eprintln!("[dotask] {message}");
    }
  }
}

pub(crate) fn execute_call(path: &Path) -> Result<i32> {
  let call: Call = serde_json::from_reader(File::open(path)?)?;
  if call.operation > 3 {
    bail!("Invalid target-call operation.");
  }
  if !call.parameters.is_object() {
    bail!("ExecTargetAsync parameters must be an object with named properties.");
  }
  let context = call.context;
  if !context.directory.task_directory.is_dir() {
    bail!(
      "The task directory disappeared during execution: {}",
      context.directory.task_directory.display()
    );
  }
  let executor = Executor {
    directory: context.directory,
    configuration: json!({ "Settings": context.settings, "TargetDefaults": context.target_defaults }),
    verbose: context.verbose,
  };
  let session = &context.session_directory;
  let catalog = executor.catalog(session)?;
  let reply_path = path.with_file_name(format!("{}.result", path.file_name().unwrap().to_string_lossy()));
  if call.operation == 1 {
    write_json(&reply_path, &json!({ "Exists": catalog.find(&call.target)?.is_some() }))?;
    return Ok(0);
  }
  let run = || -> Result<Option<i32>> {
    let Some(target) = catalog.find(&call.target)? else {
      return Ok(None);
    };
    Ok(Some(executor.execute(
      target,
      &arguments(&call.parameters)?,
      &context.call_chain,
      session,
    )?))
  };
  if call.operation == 2 {
    // Only an absent target is NotFound. Ambiguity, bad metadata, compilation
    // and execution failures are Failed; cancellation must still propagate.
    let reply = match run() {
      Ok(None) => json!({ "Exists": false }),
      Ok(Some(code)) => json!({ "Exists": true, "ExitCode": code,
                "Error": if code == 0 { None } else { Some(format!("Target '{}' failed with exit code {code}.", call.target)) } }),
      Err(error) if error.is::<Cancelled>() => return Err(error),
      Err(error) => json!({ "Exists": true, "Error": format!("{error:#}") }),
    };
    write_json(&reply_path, &reply)?;
    return Ok(0);
  }
  // CreateInstaller uses the same execution path; the task library owns
  // validation and the invocation-local installer-result.json transport.
  run()?.ok_or_else(|| anyhow::anyhow!("Unknown target '{}'.", call.target))
}

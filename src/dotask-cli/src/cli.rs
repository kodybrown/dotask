use crate::execution::{execute_call, Executor};
use crate::{help, process, project::Directory};
use anyhow::{bail, Result};
use serde_json::Value;
use std::ffi::OsString;
use std::path::Path;

pub(crate) fn run(arguments: Vec<OsString>) -> i32 {
  let completion = arguments.first().is_some_and(|s| s == "__complete");
  match run_inner(arguments) {
    Ok(code) => code,
    Err(_) if completion => 0,
    Err(error) => {
      eprintln!("dotask: {error:#}");
      if error.is::<process::Cancelled>() {
        130
      } else {
        1
      }
    }
  }
}
fn run_inner(arguments: Vec<OsString>) -> Result<i32> {
  let args = arguments
    .into_iter()
    .map(|s| {
      s.into_string().map_err(|_| {
        anyhow::anyhow!("Arguments must be valid Unicode; no replacement characters were passed to the task.")
      })
    })
    .collect::<Result<Vec<_>>>()?;
  if args.first().is_some_and(|s| s == "__metadata") {
    if args.len() != 3 {
      bail!("__metadata requires request and response files.");
    }
    process::initialize()?;
    crate::rust_tasks::batch(Path::new(&args[1]), Path::new(&args[2]))?;
    return Ok(0);
  }
  if args.first().is_some_and(|s| s == "__exec") {
    if args.len() != 2 {
      bail!("__exec requires a request file.");
    }
    process::initialize()?;
    return execute_call(Path::new(&args[1]));
  }
  if args.first().is_some_and(|s| s == "__complete") {
    return crate::completion::query(&args[1..]);
  }
  if args == ["__build-info"] {
    println!(
      "{}",
      serde_json::json!({"version":env!("DOTASK_APP_VERSION"),"stamp":env!("DOTASK_BUILD_STAMP"),
      "commit": if env!("DOTASK_BUILD_COMMIT").is_empty() {None} else {Some(env!("DOTASK_BUILD_COMMIT"))},
      "dirty":env!("DOTASK_BUILD_DIRTY") == "true"})
    );
    return Ok(0);
  }
  let command = Arguments::parse(args)?;
  if command.version {
    let revision = if env!("DOTASK_BUILD_COMMIT").is_empty() {
      String::new()
    } else {
      format!(
        ", commit {}{}",
        &env!("DOTASK_BUILD_COMMIT")[..7],
        if env!("DOTASK_BUILD_DIRTY") == "true" {
          "-dirty"
        } else {
          ""
        }
      )
    };
    println!(
      "dotask {} (build {}{})",
      env!("DOTASK_APP_VERSION"),
      env!("DOTASK_BUILD_STAMP"),
      revision
    );
    return Ok(0);
  }
  let remaining = &command.remaining;
  let action = remaining.first().map(|s| s.to_ascii_lowercase()).unwrap_or_default();
  if action == "help" && remaining.len() > 2 {
    bail!("Usage: dotask help [TARGET]");
  }
  if action == "--init" {
    if command.help {
      help::initialization();
      return Ok(0);
    }
    process::initialize()?;
    return crate::initialization::run(&std::env::current_dir()?, command.use_directory.as_deref(), &remaining[1..]);
  }
  if command.help
    && (remaining.is_empty()
      || crate::shared::COMMANDS.contains(&action.as_str())
      || action == "help" && remaining.len() == 1)
  {
    help::Text::new().line(include_str!("usage.txt").trim_end(), 0);
    return Ok(0);
  }
  if action == "completion" {
    if remaining.len() != 2 {
      bail!("Usage: dotask completion <bash|zsh|fish|powershell>");
    }
    return crate::completion::script(&remaining[1]);
  }
  process::initialize()?;
  if crate::shared::COMMANDS.contains(&action.as_str()) {
    return crate::shared::run(&command);
  }
  if action == "--create-task" {
    return crate::wizard::run(&command);
  }
  let directory = Directory::locate(std::env::current_dir()?, command.use_directory.as_deref())?;
  let session = tempfile::Builder::new().prefix("dotask-session-").tempdir()?;
  let mut executor = Executor {
    directory,
    configuration: Value::Null,
    verbose: command.verbose,
  };
  let catalog = executor.catalog(session.path())?;
  executor.configuration = catalog.configuration.clone();
  let name = remaining.get(usize::from(action == "help"));
  let Some(name) = name else {
    help::project(&catalog, &executor.directory, command.verbose);
    return Ok(0);
  };
  let target = catalog
    .find(name)?
    .ok_or_else(|| anyhow::anyhow!("Unknown target '{name}'. Run dotask help to list targets."))?;
  if let Some(error) = &target.error {
    bail!("{error}");
  }
  if command.help || action == "help" {
    help::target(&catalog, target, &executor.directory);
    return Ok(0);
  }
  if command.verbose {
    eprintln!(
      "[dotask] Project: {}; tasks: {}",
      executor.directory.root_directory.display(),
      executor.directory.task_directory.display()
    );
  }
  executor.execute(target, &remaining[1..], &[], session.path())
}

#[derive(Default)]
pub(crate) struct Arguments {
  pub use_directory: Option<String>,
  pub help: bool,
  pub version: bool,
  pub verbose: bool,
  pub remaining: Vec<String>,
}
impl Arguments {
  fn parse(args: Vec<String>) -> Result<Self> {
    let mut result = Self::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
      let lower = arg.to_ascii_lowercase();
      match lower.as_str() {
        "--help" | "-h" => result.help = true,
        "--version" => result.version = true,
        "--verbose" => result.verbose = true,
        _ if lower == "--use-dir" || lower.starts_with("--use-dir=") => {
          if result.use_directory.is_some() {
            bail!("--use-dir may only be specified once.");
          }
          let directory = if let Some((_, value)) = arg.split_once('=') {
            value.to_owned()
          } else {
            args.next().ok_or_else(|| anyhow::anyhow!("--use-dir requires a directory."))?
          };
          if directory.trim().is_empty() {
            bail!("--use-dir requires a nonempty directory.");
          }
          result.use_directory = Some(directory);
        }
        _ => result.remaining.push(arg),
      }
    }
    Ok(result)
  }
}

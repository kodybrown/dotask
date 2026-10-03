use crate::execution::{Executor, execute_call};
use crate::host::Host;
use crate::process;
use crate::project::Directory;
use anyhow::{Result, bail};
use serde_json::Value;
use std::ffi::OsString;
use std::path::Path;

const HELP: &str = "dotask - Rust CLI development preview

Usage: dotask [--use-dir PATH] [TARGET [OPTIONS]]
       dotask help [TARGET]

  --help, -h    Show CLI help, or metadata-only target help
  --version     Show the version
  --verbose     Show task execution diagnostics
  --use-dir     Select an exact task directory

Runs installed C# tasks and YAML task groups using .NET 10.
Management, shell completion, and Rust tasks still require further implementation.
Use the existing C# CLI to install tasks or manage a project.";

pub(crate) fn run(arguments: Vec<OsString>) -> i32 {
    match run_inner(arguments) {
        Ok(code) => code,
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
    let args: Vec<String> = arguments.into_iter().map(|s| s.into_string()
        .map_err(|_| anyhow::anyhow!("Arguments must be valid Unicode; no replacement characters were passed to the task.")))
        .collect::<Result<_>>()?;
    if args.first().is_some_and(|s| s == "__exec") {
        if args.len() != 2 {
            bail!("__exec requires a request file.");
        }
        process::initialize()?;
        return execute_call(Path::new(&args[1]));
    }
    let command = Arguments::parse(args)?;
    if command.version {
        println!(
            "dotask {} (Rust CLI development preview)",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(0);
    }
    if command.help && command.remaining.is_empty() {
        println!("{HELP}");
        return Ok(0);
    }
    process::initialize()?;
    let directory = Directory::locate(std::env::current_dir()?, command.use_directory.as_deref())?;
    let session = tempfile::Builder::new()
        .prefix("dotask-session-")
        .tempdir()?;
    let mut executor = Executor {
        host: Host::locate()?,
        directory,
        configuration: Value::Null,
        verbose: command.verbose,
    };
    let catalog = executor.catalog(session.path())?;
    executor.configuration = catalog.configuration.clone();
    let mut remaining = command.remaining.as_slice();
    let help = command.help
        || remaining
            .first()
            .is_some_and(|s| s.eq_ignore_ascii_case("help"));
    if remaining
        .first()
        .is_some_and(|s| s.eq_ignore_ascii_case("help"))
    {
        remaining = &remaining[1..];
    }
    let Some(name) = remaining.first() else {
        let mut invalid = false;
        for target in &catalog.targets {
            println!("  {}  {}", target.name, target.description);
            if let Err(error) = executor.bind(target, &[], session.path(), true) {
                if error.is::<process::Cancelled>() {
                    return Err(error);
                }
                eprintln!("dotask: {error:#}");
                invalid = true;
            }
        }
        println!("\nRun dotask help TARGET for details.");
        return Ok(i32::from(invalid));
    };
    let target = catalog.find(name)?.ok_or_else(|| {
        anyhow::anyhow!("Unknown target '{name}'. Run dotask help to list targets.")
    })?;
    if help {
        let parameters = executor.bind(target, &remaining[1..], session.path(), true)?;
        println!("{}\n{}", target.name, target.description);
        for option in &target.options {
            let name = option["Name"].as_str().unwrap_or_default();
            let alias = option["Alias"]
                .as_str()
                .map(|a| format!(", -{a}"))
                .unwrap_or_default();
            println!(
                "  --{name}{alias}  {}{}{}",
                option["Description"].as_str().unwrap_or_default(),
                if option["Required"] == true {
                    " (required)"
                } else {
                    ""
                },
                parameters
                    .get(name)
                    .map(|v| format!(" (default: {v})"))
                    .unwrap_or_default()
            );
        }
        if let Some(remarks) = target.metadata.get("Remarks").and_then(Value::as_str) {
            println!("\n{remarks}");
        }
        if let Some(examples) = target.metadata.get("Examples").and_then(Value::as_array) {
            for example in examples {
                if let Some(text) = example.as_str() {
                    println!("  {text}");
                }
            }
        }
        if let Some(group) = &target.group {
            for step in &group.steps {
                println!(
                    "  {}{}",
                    step.run,
                    if step.optional { " (optional)" } else { "" }
                );
            }
        }
        return Ok(0);
    }
    executor.execute(target, &remaining[1..], &[], session.path())
}

#[derive(Default)]
struct Arguments {
    use_directory: Option<String>,
    help: bool,
    version: bool,
    verbose: bool,
    remaining: Vec<String>,
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
                        args.next()
                            .ok_or_else(|| anyhow::anyhow!("--use-dir requires a directory."))?
                    };
                    if directory.trim().is_empty() {
                        bail!("--use-dir requires a nonempty directory.");
                    }
                    result.use_directory = Some(directory);
                }
                _ => result.remaining.push(arg),
            }
        }
        if let Some(first) = result.remaining.first()
            && (first.starts_with('-')
                || first.eq_ignore_ascii_case("completion")
                || first == "__complete")
        {
            bail!(
                "This Rust CLI preview does not support '{first}'. Use the existing C# CLI for project management and completion."
            );
        }
        Ok(result)
    }
}

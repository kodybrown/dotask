#[path = "../../../.tasks/_support/BuildInfo.rs"]
mod build_info;
mod config;
mod console;
mod engine;
mod files;
mod launchers;
mod model;
mod user_path;

use anyhow::{ensure, Context, Result};
use std::path::PathBuf;
fn main() {
  // Keep Ctrl+C's existing immediate exit behavior. Activation journals make
  // interruption recoverable; printing here also works while stdin is blocked.
  if let Err(error) = ctrlc::set_handler(|| {
    console::canceled();
    std::process::exit(130);
  }) {
    console::error(&error.into());
    std::process::exit(1);
  }
  if let Err(error) = run() {
    if error.is::<console::Canceled>() {
      console::canceled();
      std::process::exit(130);
    }
    console::error(&error);
    std::process::exit(1);
  }
}
fn run() -> Result<()> {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  if arguments.first().is_some_and(|arg| arg == "__config") {
    ensure!(arguments.len() == 2, "__config requires one YAML file");
    // Creator-only transport: decode source YAML (including partial templates)
    // without prompting or installing. The completed package is validated by
    // the normal package operation after the creator supplies build metadata.
    let value: serde_json::Value = files::read_yaml(std::path::Path::new(&arguments[1]))?;
    ensure!(value.is_object(), "Installer configuration must be a YAML mapping");
    println!("{}", serde_json::to_string(&value)?);
    return Ok(());
  }
  let options = config::Options::parse(arguments)?;
  if options.help {
    println!(
            "dotask-installer [install|uninstall] [--config FILE]\n  --interactive | --non-interactive  Override installer.yaml prompt mode\n  --install-dir ABSOLUTE_PATH   Installation root\n  --bin-dir ABSOLUTE_PATH       Enable an additional command in this directory\n  --additional-command         Enable the additional command (default: false)\n  --add-to-path                Add the selected command directory to user PATH (Windows)\n  --set NAME=VALUE              Override a YAML input\n  --profile NAME               Select environment defaults\n  --prune-old-versions          Keep new and previously active builds\n  --desktop-shortcuts --start-menu-shortcuts --local-shortcuts\n  --shortcut-name NAME         Name for shortcuts using ${{shortcut-name}}, without extension\n  --start-menu-nested          Nest Windows shortcuts under name/name.lnk\n  --leave-settings | --remove-settings  Uninstall settings policy\n  --validate                   Validate package without installing or prompting\n  package --config FILE --output NEW_DIRECTORY\n  --output-parent DIRECTORY    Create an app-version/build-named package\n  --result-file FILE           Write the package FilePath as JSON\nInteractive by default; use --non-interactive for automation. Rollback and version switching are not supported."
        );
    return Ok(());
  }
  let executable = std::env::current_exe()?;
  let config_file = match &options.config {
    Some(path) => files::absolute(path, &std::env::current_dir()?)?,
    None => executable.with_extension("yaml"),
  };
  ensure!(!(options.package && options.uninstall), "Choose package or uninstall");
  if options.package {
    ensure!(
      options.output.is_none() || options.output_parent.is_none(),
      "Choose --output or --output-parent"
    );
    let output = options
      .output
      .as_ref()
      .or(options.output_parent.as_ref())
      .context("package requires --output or --output-parent")?;
    let output = files::absolute(output, &std::env::current_dir()?)?;
    let result_file = options
      .result_file
      .as_ref()
      .map(|path| files::absolute(path, &std::env::current_dir()?))
      .transpose()?;
    if let Some(path) = &result_file {
      ensure!(!files::overlaps(path, &output), "Result file must be outside package output");
      files::safe_path(path)?;
    }
    let artifact = engine::package(&config_file, &output, options.output_parent.is_some())?;
    if let Some(path) = result_file {
      files::write_atomic(&path, &serde_json::to_vec(&serde_json::json!({"FilePath":artifact}))?)?;
    }
    Ok(())
  } else if options.uninstall {
    ensure!(!options.validate, "--validate is for package creation, not uninstall");
    let root = if let Some(value) = options.values.get("install-dir") {
      PathBuf::from(config::text(value)?)
    } else {
      ensure!(
        executable
          .parent()
          .and_then(|p| p.file_name())
          .is_some_and(|n| n == "installer"),
        "Package uninstall requires --install-dir"
      );
      executable
        .parent()
        .and_then(|p| p.parent())
        .context("Cannot locate installation root")?
        .to_path_buf()
    };
    let root = files::absolute(&root, &std::env::current_dir()?)?;
    engine::uninstall(&root, &options)
  } else {
    ensure!(!options.settings_explicit, "Settings flags are only valid with uninstall");
    let package = files::read_yaml(&config_file)?;
    engine::install(&config_file, package, &options)
  }
}

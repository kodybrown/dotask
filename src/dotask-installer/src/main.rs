mod config;
mod engine;
mod files;
mod launchers;
mod model;

use anyhow::{ensure, Context, Result};
use std::path::PathBuf;
fn main() {
  if let Err(error) = run() {
    eprintln!("Installer error: {error:#}");
    std::process::exit(1);
  }
}
fn run() -> Result<()> {
  let options = config::Options::parse(std::env::args().skip(1))?;
  if options.help {
    println!(
            "dotask-installer [install|uninstall] [--config FILE] [--interactive]\n  --install-dir ABSOLUTE_PATH   Installation root\n  --bin-dir ABSOLUTE_PATH       Command directory\n  --set NAME=VALUE              Override a YAML input\n  --profile NAME               Select environment defaults\n  --prune-old-versions          Keep new and previously active builds\n  --desktop-shortcuts --start-menu-shortcuts --local-shortcuts\n  --leave-settings | --remove-settings  Uninstall settings policy\n  --validate                   Validate package without installing\n  package --config FILE --output NEW_DIRECTORY\nUnattended by default. Rollback and version switching are not supported."
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
    let output = options.output.as_ref().context("package requires --output")?;
    let output = files::absolute(output, &std::env::current_dir()?)?;
    engine::package(&config_file, &output)
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

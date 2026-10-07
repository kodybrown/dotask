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
  let options = config::Options::parse(arguments)?;
  if options.help {
    println!(
            "dotask-installer [install|uninstall] [--config FILE]\n  --interactive | --non-interactive  Override installer.yaml prompt mode\n  --install-dir ABSOLUTE_PATH   Installation root\n  --bin-dir ABSOLUTE_PATH       Enable an additional command in this directory\n  --additional-command         Enable the additional command (default: false)\n  --add-to-path                Add the selected command directory to user PATH (Windows)\n  --set NAME=VALUE              Override a YAML input\n  --profile NAME               Select environment defaults\n  --prune-old-versions          Keep new and previously active builds\n  --desktop-shortcuts --start-menu-shortcuts --local-shortcuts\n  --shortcut-name NAME         Name for shortcuts using ${{shortcut-name}}, without extension\n  --start-menu-nested          Nest Windows shortcuts under name/name.lnk\n  --leave-settings | --remove-settings  Uninstall settings policy\n  --validate                   Validate package without installing or prompting\nInteractive by default; use --non-interactive for automation. Rollback and version switching are not supported."
        );
    return Ok(());
  }
  let executable = std::env::current_exe()?;
  let config_file = match &options.config {
    Some(path) => files::absolute(path, &std::env::current_dir()?)?,
    None => executable.with_extension("yaml"),
  };
  if options.uninstall {
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

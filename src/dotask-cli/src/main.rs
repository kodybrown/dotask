mod catalog;
mod cli;
mod completion;
mod configuration;
mod execution;
mod help;
mod host;
mod initialization;
mod process;
mod project;
mod rust_tasks;
mod shared;
mod shared_files;
mod transaction;
mod wizard;
mod yaml;

fn main() {
  // Preserve the full Windows exit code; ExitCode::from(u8) would truncate it.
  std::process::exit(cli::run(std::env::args_os().skip(1).collect()));
}

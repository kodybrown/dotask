use std::ffi::OsString;
use std::process::ExitCode;

const HELP: &str = "dotask - Rust CLI development preview

Usage: dotask [--help | --version]

  --help, -h    Show this help
  --version     Show the version

Task execution is not implemented in this preview.
Use the existing C# CLI to run tasks.";

pub(crate) fn run(arguments: Vec<OsString>) -> ExitCode {
    match arguments.as_slice() {
        [] => println!("{HELP}"),
        [argument] if argument == "--help" || argument == "-h" || argument == "help" => {
            println!("{HELP}");
        }
        [argument] if argument == "--version" => {
            println!(
                "dotask {} (Rust CLI development preview)",
                env!("CARGO_PKG_VERSION")
            );
        }
        _ => {
            // Until execution is ported, never acknowledge a task or management
            // request with a successful exit code or silently use a second CLI.
            eprintln!(
                "This Rust CLI preview supports only --help and --version. \
                 Use the existing C# CLI to run tasks or manage a project."
            );
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

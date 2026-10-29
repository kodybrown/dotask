// dotask: 1
// description: Fast Rust core, native CLI smoke and SDK checks; use test-all for language/installer/release integration.
// requires: [{kind: tool, value: cargo}, {kind: task, value: verify-docs}, {kind: task, value: catalog}]
// remarks: Uses isolated unoptimized bootstrap output. Does not prepare SDK distributions, run .NET/Go toolchains or inspect release binaries.
// examples: [dotask test, dotask test-all --doinstall-tools PATH]
// end-dotask
use dotask_sdk::{json, BuildContext, Result};
use std::time::Instant;
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let started = Instant::now();
  println!("Fast checks: Rust core, native CLI metadata/smoke, Rust SDK, docs and catalog.");
  println!("Use test-all for complete language, process, installer and release acceptance.");
  cargo(
    project,
    "Rust core and native CLI smoke",
    &[
      "test",
      "--locked",
      "--profile",
      "bootstrap",
      "--package",
      "dotask-cli",
      "--bin",
      "dotask",
      "--test",
      "cli",
      "--",
      "--skip",
      "integration_tests",
    ],
  )?;
  // The C# compiler probe lives in integration_tests and remains part of the
  // unfiltered complete gate. Routine CLI feedback needs no .NET/Go toolchain.
  cargo(
    project,
    "Rust SDK",
    &[
      "test",
      "--locked",
      "--profile",
      "bootstrap",
      "--package",
      "dotask-sdk",
    ],
  )?;
  project.exec_target("verify-docs", json!({}))?;
  project.exec_target("catalog", json!({"verify":true}))?;
  println!("Fast checks passed ({:.1}s).", started.elapsed().as_secs_f64());
  Ok(())
}
fn cargo(project: &BuildContext, label: &str, arguments: &[&str]) -> Result<()> {
  println!("==> {label}");
  let started = Instant::now();
  let mut command = project.command("cargo");
  command.args(arguments);
  // Match the Windows launcher's CRT setting so the runner and fast tests reuse
  // Cargo's compiled dependencies instead of building the same profile twice.
  if cfg!(windows) {
    command.env("RUSTFLAGS", "-C target-feature=+crt-static");
  }
  project.execute(&mut command)?;
  println!("{label} passed ({:.1}s).", started.elapsed().as_secs_f64());
  Ok(())
}

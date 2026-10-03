use std::process::{Command, Output};

fn run(arguments: &[&str]) -> Output {
  // No language host or installed dotask may satisfy these smoke tests.
  Command::new(env!("CARGO_BIN_EXE_dotask"))
    .args(arguments)
    .env("PATH", "")
    .output()
    .expect("launch the native CLI")
}

#[test]
fn help_runs_without_an_external_language_host() {
  for arguments in [
    &["--help"][..],
    &["-h"],
    &["--use-dir", "missing", "--help"],
  ] {
    let output = run(arguments);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Usage: dotask"));
    assert!(stdout.contains("Runs installed C# tasks"));
    assert!(output.stderr.is_empty());
  }
}

#[test]
fn version_identifies_the_development_preview() {
  let output = run(&["--version"]);
  assert!(output.status.success(), "{output:?}");
  assert_eq!(
    String::from_utf8(output.stdout).unwrap().trim(),
    format!("dotask {} (Rust CLI development preview)", env!("CARGO_PKG_VERSION"))
  );
  assert!(output.stderr.is_empty());
}

#[test]
fn unimplemented_requests_fail_instead_of_reporting_success() {
  for arguments in [
    &["--init", "unexpected"][..],
    &["--add", "git", "--lang", "rust"],
    &["completion", "unknown"],
    &["--create-task"],
  ] {
    let output = run(arguments);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
  }
}

fn in_project(root: &std::path::Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_dotask"))
    .args(args)
    .current_dir(root)
    .env("PATH", "")
    .env_remove("DOTNET_HOST_PATH")
    .env_remove("DOTNET_ROOT")
    .output()
    .unwrap()
}

#[test]
fn initialization_preserves_existing_state_and_uses_invocation_directory() {
  let parent = tempfile::tempdir().unwrap();
  std::fs::write(parent.path().join(".dotasks.yaml"), "invalid: [").unwrap();
  let root = parent.path().join("true 日本語");
  std::fs::create_dir(&root).unwrap();
  for args in [&["--init"][..], &["--INIT"]] {
    let result = in_project(&root, args);
    assert!(result.status.success(), "{result:?}");
  }
  let config = root.join(".dotasks.yaml");
  let bytes = std::fs::read(&config).unwrap();
  let time = config.metadata().unwrap().modified().unwrap();
  assert_eq!(2, std::fs::read_dir(&root).unwrap().count());
  let result = in_project(&root, &["--init"]);
  assert!(result.status.success(), "{result:?}");
  assert_eq!(bytes, std::fs::read(&config).unwrap());
  assert_eq!(time, config.metadata().unwrap().modified().unwrap());
  let result = in_project(&root, &["--verbose"]);
  assert!(result.status.success(), "{result:?}");
  assert!(String::from_utf8_lossy(&result.stdout).contains("true 日本語"));
}

#[test]
fn init_rejects_conflicts_and_help_never_creates_anything() {
  for config in [
    "unknown: 1",
    "settings: []",
    "settings: { x: 1, X: 2 }",
    "version: 2",
    "name: true",
    "settings: &x { x: *x }",
    "settings: {}\n---\nsettings: {}",
    "settings: [",
  ] {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(".dotasks.yaml");
    std::fs::write(&path, config).unwrap();
    let result = in_project(root.path(), &["--init"]);
    assert!(!result.status.success(), "{config}: {result:?}");
    assert_eq!(config, std::fs::read_to_string(path).unwrap());
    assert!(!root.path().join(".tasks").exists());
  }
  let root = tempfile::tempdir().unwrap();
  for args in [
    &["--init", "--help"][..],
    &["--use-dir", "missing", "--help"],
    &["--create-task", "--help"],
  ] {
    assert!(in_project(root.path(), args).status.success());
  }
  assert_eq!(0, std::fs::read_dir(root.path()).unwrap().count());
  for path in ["..", ".", ".dotasks.yaml/tasks", ".dotasks-lock.yaml"] {
    assert!(!in_project(root.path(), &["--init", "--use-dir", path]).status.success());
  }
  assert_eq!(0, std::fs::read_dir(root.path()).unwrap().count());
  std::fs::write(root.path().join(".tasks"), "user file").unwrap();
  assert!(!in_project(root.path(), &["--init"]).status.success());
  assert!(!root.path().join(".dotasks.yaml").exists());
}

#[test]
fn groups_configuration_and_completion_run_without_dotnet() {
  let root = tempfile::tempdir().unwrap();
  std::fs::create_dir(root.path().join(".tasks")).unwrap();
  std::fs::write(
    root.path().join(".dotasks.yaml"),
    "name: Example\nsettings: { x: 0x10, number: 12, text: '12', flag: false, label: !!str true }\n",
  )
  .unwrap();
  std::fs::write(root.path().join(".tasks/empty.task"), "description: Native group\nsteps: []").unwrap();
  let output = in_project(root.path(), &["--verbose"]);
  assert!(output.status.success(), "{output:?}");
  let text = String::from_utf8(output.stdout).unwrap();
  for s in [
    "Example",
    "x: 0x10",
    "flag: false",
    "Native group",
    "tasks: ./.tasks",
  ] {
    assert!(text.contains(s), "{text}");
  }
  assert!(in_project(root.path(), &["empty"]).status.success());
  std::fs::write(root.path().join(".dotasks.yaml"), "invalid: [").unwrap();
  let output = in_project(root.path(), &["__complete", "--line", "dotask em"]);
  assert!(output.status.success());
  assert!(String::from_utf8_lossy(&output.stdout).contains("empty\ttarget\tNative group"));
  assert!(output.stderr.is_empty());
  for shell in ["bash", "zsh", "fish", "powershell"] {
    assert!(in_project(root.path(), &["completion", shell]).status.success());
  }
}

#[test]
fn malformed_completion_stays_silent_and_successful() {
  for args in [
    &["__complete", "bad"][..],
    &["__complete", "--position", "invalid"],
    &["__complete", "--unknown", "x"],
  ] {
    let output = run(args);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
  }
}

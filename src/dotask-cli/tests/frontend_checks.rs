use serde_json::json;
use std::{fs, path::Path, process::Command};

const NPM_CHECK: &str = include_str!("../../../shared-tasks/npm/check.task");
const NPM_INSTALL: &str = include_str!("../../../shared-tasks/npm/install.task");
const ANGULAR_CHECK: &str = include_str!("../../../shared-tasks/angular/check.task");

struct Fixture {
  root: tempfile::TempDir,
  tasks: String,
}
impl Fixture {
  fn new(custom: bool) -> Self {
    let root = tempfile::tempdir().unwrap();
    let tasks = if custom { "automation tasks" } else { ".tasks" }.to_owned();
    for (name, text) in [
      ("npm/check.task", NPM_CHECK),
      ("npm/install.task", NPM_INSTALL),
      ("angular/check.task", ANGULAR_CHECK),
    ] {
      let path = root.path().join(&tasks).join("_").join(name);
      fs::create_dir_all(path.parent().unwrap()).unwrap();
      fs::write(path, text).unwrap();
    }
    fs::create_dir_all(root.path().join("website with spaces/nested")).unwrap();
    fs::create_dir_all(root.path().join("tools with spaces")).unwrap();
    fs::write(
      root.path().join(".dotasks.yaml"),
      "settings: {angular: 'website with spaces'}\n",
    )
    .unwrap();
    fs::write(
      root.path().join("website with spaces/angular.json"),
      "// Angular accepts JSON comments\n{}\n",
    )
    .unwrap();
    let fixture = Self { root, tasks };
    fixture.probe("node");
    fixture.probe("npm");
    fixture
  }
  fn probe(&self, name: &str) {
    let tools = self.root.path().join("tools with spaces");
    #[cfg(windows)]
    {
      // A Windows npm installation contains both a Unix script and a cmd shim.
      // Choosing the extensionless companion must not make the check unusable.
      fs::write(tools.join(name), "not a native executable\n").unwrap();
      fs::write(
        tools.join(format!("{name}.cmd")),
        format!(
          "@echo off\r\n>>\"%FRONTEND_LOG%\" echo {name}:%CD%:%*\r\nif defined FRONTEND_FAIL exit /b %FRONTEND_FAIL%\r\nexit /b 0\r\n"
        ),
      )
      .unwrap();
    }
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let path = tools.join(name);
      fs::write(
        &path,
        format!("#!/bin/sh\nprintf '%s\\n' \"{name}:$PWD:$*\" >> \"$FRONTEND_LOG\"\nexit \"${{FRONTEND_FAIL:-0}}\"\n"),
      )
      .unwrap();
      fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
  }
  fn command(&self, arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dotask"));
    if self.tasks != ".tasks" {
      command.arg("--use-dir").arg(self.root.path().join(&self.tasks));
    }
    command
      .args(arguments)
      .current_dir(self.root.path().join("website with spaces/nested"))
      .env("PATH", self.root.path().join("tools with spaces"))
      .env("PATHEXT", ".PS1;.EXE;.COM;.BAT;.CMD")
      .env("FRONTEND_LOG", self.root.path().join("calls.txt"))
      .env("DOTASK_CACHE_HOME", self.root.path().join("cache"))
      .env_remove("FRONTEND_FAIL");
    command
  }
  fn calls(&self) -> String {
    fs::read_to_string(self.root.path().join("calls.txt")).unwrap_or_default()
  }
}

#[test]
fn npm_checks_resolve_native_launchers_and_report_both_versions_without_installing() {
  let fixture = Fixture::new(false);
  let output = fixture.command(&["_/npm/check.task"]).output().unwrap();
  assert!(output.status.success(), "{output:?}");
  let calls = fixture.calls();
  assert_eq!(calls.lines().count(), 2, "{calls}");
  assert!(
    calls.contains(&format!("node:{}:--version", fixture.root.path().display())),
    "{calls}"
  );
  assert!(
    calls.contains(&format!("npm:{}:--version", fixture.root.path().display())),
    "{calls}"
  );
}

#[test]
fn angular_uses_configured_workspace_and_local_cli_with_custom_task_directories() {
  let fixture = Fixture::new(true);
  let output = fixture.command(&["_/angular/check.task"]).output().unwrap();
  assert!(output.status.success(), "{output:?}");
  let calls = fixture.calls();
  assert_eq!(calls.lines().count(), 1, "{calls}");
  assert!(
    calls.contains(&format!(
      "node:{}:node_modules/@angular/cli/bin/ng.js version",
      fixture.root.path().join("website with spaces").display()
    )),
    "{calls}"
  );
}

#[test]
fn angular_missing_workspace_fails_before_starting_the_cli() {
  let fixture = Fixture::new(false);
  fs::remove_file(fixture.root.path().join("website with spaces/angular.json")).unwrap();
  let output = fixture.command(&["_/angular/check.task"]).output().unwrap();
  assert!(!output.status.success(), "{output:?}");
  assert!(fixture.calls().is_empty());
  assert!(String::from_utf8_lossy(&output.stderr).contains("angular.json"), "{output:?}");
}

#[test]
fn npm_install_runs_only_install_in_the_selected_directory_and_preserves_failure() {
  for failure in ["0", "23"] {
    let fixture = Fixture::new(true);
    let output = fixture
      .command(&["_/npm/install.task", "--directory", "website with spaces"])
      .env("FRONTEND_FAIL", failure)
      .output()
      .unwrap();
    assert_eq!(output.status.code(), Some(failure.parse().unwrap()), "{output:?}");
    assert_eq!(
      fixture.calls().trim(),
      format!("npm:{}:install", fixture.root.path().join("website with spaces").display())
    );
  }
}

#[test]
fn check_failure_stops_later_probes_and_missing_tools_fail_clearly() {
  let fixture = Fixture::new(false);
  let output = fixture
    .command(&["_/npm/check.task"])
    .env("FRONTEND_FAIL", "17")
    .output()
    .unwrap();
  assert_eq!(output.status.code(), Some(17), "{output:?}");
  assert_eq!(fixture.calls().lines().count(), 1);

  let root = tempfile::tempdir().unwrap();
  fs::create_dir(root.path().join(".tasks")).unwrap();
  fs::write(root.path().join(".dotasks.yaml"), "settings: {}\n").unwrap();
  fs::write(
    root.path().join(".tasks/probe.task"),
    "steps: [{exec: {executable: {tool: absent-dotask-tool}, arguments: []}}]",
  )
  .unwrap();
  let output = Command::new(env!("CARGO_BIN_EXE_dotask"))
    .arg("probe")
    .current_dir(root.path())
    .env("PATH", "")
    .output()
    .unwrap();
  assert!(!output.status.success(), "{output:?}");
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("absent-dotask-tool"),
    "{output:?}"
  );
}

#[test]
fn tool_reference_accepts_configured_explicit_paths_and_rejects_bad_metadata_without_running() {
  let fixture = Fixture::new(false);
  let extension = if cfg!(windows) { ".cmd" } else { "" };
  let path = Path::new("tools with spaces").join(format!("node{extension}"));
  fs::write(
    fixture.root.path().join(".dotasks.yaml"),
    format!("settings: {{launcher: {}}}\n", json!(path.to_str().unwrap())),
  )
  .unwrap();
  let task = fixture.root.path().join(".tasks/probe.task");
  fs::write(
    &task,
    "steps: [{exec: {executable: {tool: {setting: launcher}}, arguments: [--version]}}]",
  )
  .unwrap();
  let output = fixture.command(&["probe"]).output().unwrap();
  assert!(output.status.success(), "{output:?}");
  let before = fixture.calls();
  fs::write(&task, "steps: [{exec: {executable: {tool: [bad]}, arguments: []}}]").unwrap();
  let output = fixture.command(&["probe"]).output().unwrap();
  assert!(!output.status.success(), "{output:?}");
  assert_eq!(fixture.calls(), before);
}

#[test]
fn help_reads_shared_check_metadata_without_running_or_requiring_node() {
  let fixture = Fixture::new(false);
  let output = fixture
    .command(&["help", "_/angular/check.task"])
    .env("PATH", "")
    .output()
    .unwrap();
  assert!(output.status.success(), "{output:?}");
  assert!(fixture.calls().is_empty());
  assert!(!String::from_utf8_lossy(&output.stderr).contains("Unknown process reference"));
}

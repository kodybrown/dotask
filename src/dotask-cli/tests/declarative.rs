use serde_json::json;
use std::{fs, process::Command};

fn task(root: &std::path::Path, text: &str) {
  fs::create_dir_all(root.join(".tasks")).unwrap();
  fs::write(root.join(".dotasks.yaml"), "settings: {}\n").unwrap();
  fs::write(root.join(".tasks/install.task"), text).unwrap();
}
fn run(root: &std::path::Path) -> std::process::Output {
  Command::new(env!("CARGO_BIN_EXE_dotask"))
    .arg("install")
    .current_dir(root)
    // These probes model a fresh outer workflow, including its own timeout.
    // Cargo itself can be running inside dotask's repository verification tree.
    .env_remove("DOTASK_PROCESS_GROUP_OWNER")
    .env_remove("DOINSTALL_PROCESS_GROUP_OWNER")
    .env("PATH", "")
    .env("DOTASK_CACHE_HOME", root.join("cache"))
    .output()
    .unwrap()
}

#[test]
fn yaml_steps_use_target_settings_and_nested_calls_keep_the_original_shared_snapshot() {
  let root = tempfile::tempdir().unwrap();
  task(
    root.path(),
    r#"
requires: [{kind: setting, value: marker}]
steps:
  - id: config
    file: {operation: read, path: .dotasks.yaml}
  - file: {operation: write, path: parent.txt, content: {setting: marker}}
  - file: {operation: write, path: .dotasks.yaml, content: 'settings: {marker: changed}', expected-sha256: {result: config.sha256}}
  - run: child
    with: {forwarded: {setting: marker}}
  - run: tail
"#,
  );
  fs::write(
    root.path().join(".dotasks.yaml"),
    r#"
settings: {marker: shared}
targets:
  install:
    settings: {marker: parent}
  child:
    settings: {marker: child, output: child.txt}
  child.task:
    settings: {output: qualified.txt}
"#,
  )
  .unwrap();
  fs::write(
    root.path().join(".tasks/child.task"),
    r#"
options: [{name: forwarded, required: true}]
requires: [{kind: setting, value: output}]
steps:
  - file: {operation: write, path: {setting: output}, content: {setting: marker}}
  - file: {operation: write, path: forwarded.txt, content: {parameter: forwarded}}
"#,
  )
  .unwrap();
  fs::write(
    root.path().join(".tasks/tail.task"),
    "steps: [{file: {operation: write, path: tail.txt, content: {setting: marker}}}]",
  )
  .unwrap();
  let output = run(root.path());
  assert!(output.status.success(), "{output:?}");
  for (path, expected) in [
    ("parent.txt", "parent"),
    ("qualified.txt", "child"),
    ("forwarded.txt", "parent"),
    ("tail.txt", "shared"),
  ] {
    assert_eq!(fs::read_to_string(root.path().join(path)).unwrap(), expected);
  }
  assert!(!root.path().join("child.txt").exists());
}

#[test]
fn native_yaml_reads_renders_guards_updates_and_calls_tasks_without_a_language_host() {
  let root = tempfile::tempdir().unwrap();
  fs::write(root.path().join("template"), "port={{port}}\n").unwrap();
  task(
    root.path(),
    r#"
description: Render configuration
steps:
  - id: previous
    file: {operation: read, path: service.conf, optional: true}
  - id: config
    file:
      operation: render
      source: template
      path: service.conf
      values: {port: "8080"}
      expected-sha256: {result: previous.sha256}
  - run: changed
    when: {result: config.changed}
    with: {content: {result: config.sha256}}
"#,
  );
  fs::write(
    root.path().join(".tasks/changed.task"),
    r#"
options:
  - name: content
    type: string
    required: true
steps:
  - file: {operation: write, path: changed.txt, content: {parameter: content}}
"#,
  )
  .unwrap();
  let output = run(root.path());
  assert!(output.status.success(), "{output:?}");
  assert_eq!(fs::read_to_string(root.path().join("service.conf")).unwrap(), "port=8080\n");
  fs::remove_file(root.path().join("changed.txt")).unwrap();
  let modified = fs::metadata(root.path().join("service.conf")).unwrap().modified().unwrap();
  assert!(run(root.path()).status.success());
  assert!(!root.path().join("changed.txt").exists());
  assert_eq!(
    modified,
    fs::metadata(root.path().join("service.conf")).unwrap().modified().unwrap()
  );
}

#[test]
fn failed_guards_and_metadata_stop_before_later_actions() {
  for malformed in [
    "steps:\n  - file: {operation: write, path: config, content: new, expected-sha256: incorrect}\n",
    "steps:\n  - id: now\n    file: {operation: write, path: created, content: {result: later.content}}\n",
    "steps:\n  - exec: {executable: missing, timeout-seconds: 0}\n",
    "steps:\n  - file: {operation: erase, path: config}\n",
  ] {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("config"), "original").unwrap();
    task(root.path(), malformed);
    let output = run(root.path());
    assert!(!output.status.success(), "{output:?}");
    assert_eq!(fs::read_to_string(root.path().join("config")).unwrap(), "original");
    assert!(!root.path().join("created").exists());
  }
}

#[test]
fn process_capture_and_json_reads_supply_typed_results() {
  let root = tempfile::tempdir().unwrap();
  fs::write(root.path().join("context.json"), "{\"port\":8080,\"enabled\":true}").unwrap();
  let executable = serde_json::to_string(env!("CARGO_BIN_EXE_dotask")).unwrap();
  task(
    root.path(),
    r#"
steps:
  - id: context
    file: {operation: read, path: context.json, format: json}
  - id: probe
    exec:
      executable: {setting: tool}
      arguments: ["__build-info"]
      capture-output: true
      environment: {INSTALL_PORT: {result: context.data.port}}
      timeout-seconds: 10
  - file:
      operation: write
      path: result.json
      content: {result: probe.output}
    when: {result: context.data.enabled}
"#,
  );
  fs::write(
    root.path().join(".dotasks.yaml"),
    format!("targets: {{install: {{settings: {{tool: {executable}}}}}}}\n"),
  )
  .unwrap();
  let output = run(root.path());
  assert!(output.status.success(), "{output:?}");
  let result: serde_json::Value = serde_json::from_slice(&fs::read(root.path().join("result.json")).unwrap()).unwrap();
  assert!(result["version"].is_string());
  assert_eq!(json!(8080).as_i64(), Some(8080));
}

#[test]
fn timed_out_process_stops_the_group_without_running_following_steps() {
  let root = tempfile::tempdir().unwrap();
  #[cfg(windows)]
  let command = format!(
    "executable: {}\n      arguments: [\"127.0.0.1\", \"-n\", \"15\"]",
    serde_json::to_string(&std::path::Path::new(&std::env::var("SystemRoot").unwrap()).join("System32/ping.exe"))
      .unwrap()
  );
  #[cfg(unix)]
  let command = "executable: /bin/sleep\n      arguments: [\"15\"]".to_owned();
  task(root.path(), &format!("steps:\n  - exec:\n      {command}\n      timeout-seconds: 1\n  - file: {{operation: write, path: later, content: bad}}\n"));
  let start = std::time::Instant::now();
  let output = run(root.path());
  assert_eq!(output.status.code(), Some(124), "{output:?}");
  assert!(start.elapsed() < std::time::Duration::from_secs(5));
  assert!(!root.path().join("later").exists());
}

#[test]
fn conditional_results_require_boolean_values_and_skipped_results_fail_clearly() {
  let root = tempfile::tempdir().unwrap();
  task(root.path(), "steps:\n  - id: read\n    file: {operation: read, path: missing, optional: true}\n  - file: {operation: write, path: created, content: bad}\n    when: {result: read.content}\n");
  let output = run(root.path());
  assert!(!output.status.success());
  assert!(!root.path().join("created").exists());
}

#[test]
fn capture_probe() {
  if std::env::var("DOTASK_CAPTURE_PROBE").is_ok() {
    let path = std::env::args().find(|a| a.ends_with(".probe")).unwrap();
    fs::write(path, b"artifact").unwrap();
    use std::io::Write;
    std::io::stdout().write_all(&[0xff]).unwrap();
  }
}

#[test]
fn invalid_captured_stdout_does_not_publish_a_staged_process_output() {
  let root = tempfile::tempdir().unwrap();
  let executable = serde_json::to_string(&std::env::current_exe().unwrap()).unwrap();
  task(
    root.path(),
    &format!(
      r#"
steps:
  - exec:
      executable: {executable}
      arguments: ["--exact", "capture_probe", "--nocapture", {{output-file: true}}]
      output-file: {{path: archive, suffix: .probe}}
      capture-output: true
      environment: {{DOTASK_CAPTURE_PROBE: "true"}}
"#
    ),
  );
  let result = run(root.path());
  assert!(!result.status.success(), "{result:?}");
  assert!(!root.path().join("archive").exists());
}

#[test]
fn posix_modes_remain_metadata_only_in_help_on_every_host() {
  let root = tempfile::tempdir().unwrap();
  task(
    root.path(),
    "steps:\n  - file: {operation: directory, path: never-created, mode: \"0755\"}\n",
  );
  let output = Command::new(env!("CARGO_BIN_EXE_dotask"))
    .args(["help", "install"])
    .current_dir(root.path())
    .env("PATH", "")
    .output()
    .unwrap();
  assert!(output.status.success(), "{output:?}");
  assert!(!root.path().join("never-created").exists());
  #[cfg(windows)]
  {
    let output = run(root.path());
    assert!(!output.status.success());
    assert!(!root.path().join("never-created").exists());
  }
}

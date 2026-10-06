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
fn version_matches_embedded_application_build_metadata() {
  let output = run(&["--version"]);
  assert!(output.status.success(), "{output:?}");
  assert_eq!(
    String::from_utf8(output.stdout).unwrap().trim(),
    format!(
      "dotask {} (build {}{})",
      env!("DOTASK_APP_VERSION"),
      env!("DOTASK_BUILD_STAMP"),
      if env!("DOTASK_BUILD_COMMIT").is_empty() {
        String::new()
      } else {
        format!(
          ", commit {}{}",
          &env!("DOTASK_BUILD_COMMIT")[..7],
          if env!("DOTASK_BUILD_DIRTY") == "true" {
            "-dirty"
          } else {
            ""
          }
        )
      }
    )
  );
  assert!(output.stderr.is_empty());
  let metadata = run(&["__build-info"]);
  assert!(metadata.status.success());
  let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
  assert_eq!(metadata["version"], env!("DOTASK_APP_VERSION"));
  assert_eq!(metadata["stamp"], env!("DOTASK_BUILD_STAMP"));
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
fn rust_help_and_completion_read_metadata_without_any_toolchain() {
  let project = tempfile::tempdir().unwrap();
  std::fs::create_dir(project.path().join(".tasks")).unwrap();
  std::fs::write(project.path().join(".tasks/run.rs"), "// dotask: 1\n// description: Native task\n// options: [{name: label, choices: [first, second], default: first}]\n// end-dotask\nnot valid Rust;\n").unwrap();
  let help = in_project(project.path(), &["help", "run"]);
  assert!(help.status.success(), "{help:?}");
  assert!(String::from_utf8_lossy(&help.stdout).contains("Native task"));
  let completion = in_project(
    project.path(),
    &[
      "__complete",
      "--line",
      "dotask run --label ",
      "--shell",
      "powershell",
    ],
  );
  assert!(completion.status.success(), "{completion:?}");
  assert!(String::from_utf8_lossy(&completion.stdout).contains("second"));
}

#[test]
fn csharp_headers_help_completion_and_private_catalog_need_no_sdk() {
  let project = tempfile::tempdir().unwrap();
  std::fs::create_dir(project.path().join(".tasks")).unwrap();
  std::fs::write(project.path().join(".tasks/build.cs"), "\u{feff}\r\n// dotask: 1\r\n// description: C# metadata without execution\r\n// options: [{name: mode, choices: [fast, full], default: fast}]\r\n// end-dotask\r\nnot valid C#;\r\n").unwrap();
  let output = in_project(project.path(), &["help", "build"]);
  assert!(output.status.success(), "{output:?}");
  let text = String::from_utf8(output.stdout).unwrap();
  assert!(
    text.contains("C# metadata without execution")
      && text.contains("--mode")
      && text.contains("Unavailable: requires .NET SDK"),
    "{text}"
  );
  assert!(output.stderr.is_empty());
  let output = in_project(project.path(), &["__complete", "--line", "dotask build --mode f"]);
  assert!(output.status.success(), "{output:?}");
  assert!(String::from_utf8_lossy(&output.stdout).contains("full"));
  let output = in_project(project.path(), &["build"]);
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains(".NET SDK"), "{output:?}");
  let private = project.path().join("private/tools");
  std::fs::create_dir_all(&private).unwrap();
  std::fs::write(
    private.join("check.cs"),
    "// dotask: 1\n// description: Private metadata\n// end-dotask\ninvalid source",
  )
  .unwrap();
  let output = Command::new(env!("CARGO_BIN_EXE_dotask"))
    .current_dir(project.path())
    .args(["--list", "private-tasks/tools/*"])
    .env("PATH", "")
    .env_remove("DOTNET_HOST_PATH")
    .env_remove("DOTNET_ROOT")
    .env("DOTASK_PRIVATE_TASKS", project.path().join("private"))
    .env("DOTASK_CACHE_HOME", project.path().join("cache"))
    .output()
    .unwrap();
  assert!(output.status.success(), "{output:?}");
  assert!(String::from_utf8_lossy(&output.stdout).contains("Private metadata"));
}

#[test]
fn unavailable_rust_tasks_remain_visible_and_fail_with_toolchain_guidance() {
  let project = tempfile::tempdir().unwrap();
  std::fs::create_dir(project.path().join(".tasks")).unwrap();
  std::fs::write(
    project.path().join(".tasks/check.rs"),
    "// dotask: 1\n// description: Visible Rust task\n// end-dotask\nfn main() {}\n",
  )
  .unwrap();
  let output = in_project(project.path(), &["help"]);
  assert!(output.status.success());
  let text = String::from_utf8_lossy(&output.stdout);
  assert!(
    text.contains("Visible Rust task") && text.contains("requires Rust 1.95+ and Cargo"),
    "{text}"
  );
  let output = in_project(project.path(), &["check"]);
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("Rust 1.95+ and Cargo"));
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

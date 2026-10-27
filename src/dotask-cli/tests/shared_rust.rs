//! Published Rust group acceptance using the checkout CLI and normal management.
//! No task in this flow invokes Git/GitHub or modifies an active installation.
#[path = "../../../shared-tasks/rust/_support/Rust.rs"]
mod rust_operations;
use sha2::{Digest, Sha256};
use std::{
  fs,
  path::Path,
  process::{Command, Output},
};
fn success(output: Output) -> Output {
  assert!(
    output.status.success(),
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  output
}
#[test]
fn catalog_tasks_install_with_dependencies_and_run_offline_without_project_policy() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let temporary = tempfile::tempdir().unwrap();
  let fixture = temporary.path().join("project");
  fs::create_dir(&fixture).unwrap();
  fs::create_dir(fixture.join(".tasks")).unwrap();
  fs::write(fixture.join(".dotasks.yaml"), "settings: {}\n").unwrap();
  let files = [
    "build.task",
    "run.task",
    "test.task",
    "format.task",
    "clippy.task",
    "cargo.rs",
    "update-version.rs",
    "_support/Rust.rs",
    "README.md",
  ];
  let online = temporary.path().join("online");
  fs::create_dir_all(online.join("rust/_support")).unwrap();
  for name in files {
    fs::copy(root.join("shared-tasks/rust").join(name), online.join("rust").join(name)).unwrap();
  }
  let catalog = fs::read(root.join("shared-tasks/catalog.json")).unwrap();
  fs::write(online.join("catalog.json"), &catalog).unwrap();
  let invoke_with = |args: &[&str], online: bool| {
    let exe = Path::new(env!("CARGO_BIN_EXE_dotask"));
    Command::new(exe)
      .current_dir(&fixture)
      .args(args)
      .env(
        "DOTASK_CACHE_HOME",
        temporary.path().join(if online { "cache" } else { "empty-cache" }),
      )
      .env("DOTASK_PRIVATE_TASKS", temporary.path().join("private"))
      .env(
        "DOTASK_ONLINE_TASKS",
        if online {
          temporary.path().join("online")
        } else {
          temporary.path().join("unavailable")
        },
      )
      // Fixture output must not inherit the source checkout's target override.
      // Its verification hook executes the binary in this isolated directory.
      .env("CARGO_TARGET_DIR", temporary.path().join("target"))
      // The source CLI uses its source SDK, never an installed application.
      .env("DOTASK_SOURCE_RUST_SDK", root.join("src/dotask-sdk"))
      .env("DOTASK_SOURCE_SDK_STAGE", exe.parent().unwrap().join("sdk"))
      .output()
      .unwrap()
  };
  // Install one root first to prove transitive YAML/Rust dependencies and
  // shared support ownership, then add the whole mixed-language group.
  success(invoke_with(&["--add", "rust/run"], true));
  for name in [
    "run.task",
    "build.task",
    "format.task",
    "clippy.task",
    "cargo.rs",
    "_support/Rust.rs",
    "README.md",
  ] {
    assert_eq!(
      fs::read(fixture.join(".tasks/_/rust").join(name)).unwrap(),
      fs::read(root.join("shared-tasks/rust").join(name)).unwrap()
    );
  }
  assert!(!fixture.join(".tasks/_/rust/test.task").exists());
  success(invoke_with(&["--add", "rust"], true));
  let tracking = fs::read(fixture.join(".dotasks-lock.yaml")).unwrap();
  success(invoke_with(&["--sync", "--dry-run"], true));
  success(invoke_with(&["--sync"], true));
  assert_eq!(fs::read(fixture.join(".dotasks-lock.yaml")).unwrap(), tracking);
  let readme = fixture.join(".tasks/_/rust/README.md");
  let original_readme = fs::read(&readme).unwrap();
  fs::write(&readme, b"local edit must survive\n").unwrap();
  // A no-op can preserve local edits; an actual upstream update must refuse
  // to overwrite them. Change the fixture source and its declared hashes.
  let upstream = b"upstream documentation update\n";
  fs::write(online.join("rust/README.md"), upstream).unwrap();
  let mut changed: serde_json::Value = serde_json::from_slice(&catalog).unwrap();
  for task in changed["tasks"].as_array_mut().unwrap() {
    for file in task["files"].as_array_mut().unwrap() {
      if file["path"] == "rust/README.md" {
        file["sha256"] = format!("{:x}", Sha256::digest(upstream)).into();
      }
    }
  }
  fs::write(online.join("catalog.json"), serde_json::to_vec(&changed).unwrap()).unwrap();
  assert!(!invoke_with(&["--sync"], true).status.success());
  assert_eq!(fs::read(&readme).unwrap(), b"local edit must survive\n");
  assert_eq!(fs::read(fixture.join(".dotasks-lock.yaml")).unwrap(), tracking);
  fs::write(&readme, &original_readme).unwrap();
  fs::write(online.join("rust/README.md"), original_readme).unwrap();
  fs::write(online.join("catalog.json"), catalog).unwrap();
  fs::create_dir(fixture.join("src")).unwrap();
  fs::create_dir(fixture.join(".cargo")).unwrap();
  fs::write(
    fixture.join(".cargo/config.toml"),
    format!(
      "[build]\ntarget-dir = {:?}\n",
      temporary.path().join("target").to_string_lossy().replace('\\', "/")
    ),
  )
  .unwrap();
  let manifest = fixture.join("Cargo.toml");
  let lock = fixture.join("Cargo.lock");
  let source = fixture.join("src/main.rs");
  fs::write(
    &manifest,
    "[package]\nname = \"task-fixture\"\nversion = \"1.2.3\"\nedition = \"2024\"\n",
  )
  .unwrap();
  let program = "fn main(){ let args: Vec<_> = std::env::args().skip(1).collect(); if args == [\"fail\"] { std::process::exit(17); } println!(\"{}\", args.join(\"|\")); }\n#[test] fn example() { assert_eq!(2 + 2, 4); }\n";
  fs::write(&source, program).unwrap();
  success(
    Command::new("cargo")
      .current_dir(&fixture)
      .args(["generate-lockfile", "--offline"])
      .output()
      .unwrap(),
  );
  let original = fs::read(&manifest).unwrap();
  let locked = fs::read(&lock).unwrap();
  let invoke = |task: &str, args: &[&str]| {
    let mut call = vec![task];
    call.extend_from_slice(args);
    // No catalog source or populated management cache is available at runtime.
    invoke_with(&call, false)
  };
  success(invoke("_/rust/build", &[]));
  assert_ne!(fs::read_to_string(&source).unwrap(), program, "Build must apply formatting");
  assert_eq!(fs::read(&manifest).unwrap(), original);
  assert_eq!(fs::read(&lock).unwrap(), locked);
  let output = success(invoke(
    "_/rust/run",
    &[
      "--args",
      r#"["two words","", "quote\"slash\\", "\u65e5\u672c\u8a9e"]"#,
    ],
  ));
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains("two words||quote\"slash\\|\u{65e5}\u{672c}\u{8a9e}"),
    "{stdout}"
  );
  assert_eq!(
    stdout.matches("Private-content check passed:").count(),
    1,
    "Run builds/scans once"
  );
  assert_eq!(invoke("_/rust/run", &["--args", r#"["fail"]"#]).status.code(), Some(17));
  assert!(!invoke("_/rust/run", &["--args", r#"[1]"#]).status.success());
  success(invoke("_/rust/test", &[]));
  // Only an absent hook is skipped. A real hook runs after Rust tests and its
  // failures, invalid metadata, ambiguity and recursion remain visible.
  let hook = fixture.join(".tasks/verify.task");
  let fixture_binary = temporary.path().join("target/release").join(if cfg!(windows) {
    "task-fixture.exe"
  } else {
    "task-fixture"
  });
  let executable = serde_json::to_string(&fixture_binary.to_string_lossy().replace('\\', "/")).unwrap();
  fs::write(
    &hook,
    format!("steps:\n  - exec:\n      executable: {executable}\n      arguments: [verified]\n"),
  )
  .unwrap();
  let output = success(invoke("_/rust/test", &[]));
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.find("test result:").unwrap() < stdout.rfind("verified").unwrap());
  fs::write(
    &hook,
    format!("steps:\n  - exec:\n      executable: {executable}\n      arguments: [fail]\n"),
  )
  .unwrap();
  assert_eq!(invoke("_/rust/test", &[]).status.code(), Some(17));
  success(invoke("_/rust/test", &["--verify", "false"]));
  fs::write(fixture.join(".tasks/verify.rs"), "fn main() {}\n").unwrap();
  assert!(!invoke("_/rust/test", &[]).status.success(), "Ambiguous verification must fail");
  fs::remove_file(fixture.join(".tasks/verify.rs")).unwrap();
  fs::write(&hook, "unknown: true\nsteps: []\n").unwrap();
  assert!(!invoke("_/rust/test", &[]).status.success(), "Invalid verification must fail");
  fs::write(&hook, "steps: [{run: _/rust/test}]\n").unwrap();
  let output = invoke("_/rust/test", &[]);
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("Target cycle:"));
  fs::remove_file(&hook).unwrap();
  // A later build must not allow run/staging to consume different bytes from
  // the successful scan. Exercise the low-level operation without rebuilding.
  let inspected = fs::read(&fixture_binary).unwrap();
  let mut changed = inspected.clone();
  changed.extend_from_slice(b"changed after inspection");
  fs::write(&fixture_binary, changed).unwrap();
  let output = invoke("_/rust/cargo", &["--operation", "run"]);
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("Executable changed since"));
  fs::write(&fixture_binary, inspected).unwrap();
  assert_eq!(fs::read(&manifest).unwrap(), original);
  assert_eq!(fs::read(&lock).unwrap(), locked);
  // Explicit versioning changes only version files. No selector means patch;
  // previews and conflicting selectors leave both files intact.
  success(invoke("_/rust/update-version", &["--dry-run"]));
  assert_eq!(fs::read(&manifest).unwrap(), original);
  assert_eq!(fs::read(&lock).unwrap(), locked);
  assert!(!invoke("_/rust/update-version", &["--increment", "--increment-major"])
    .status
    .success());
  for (args, version) in [
    (vec![], "1.2.4"),
    (vec!["--increment"], "1.2.5"),
    (vec!["--increment-minor"], "1.3.0"),
    (vec!["--increment-major"], "2.0.0"),
  ] {
    success(invoke("_/rust/update-version", &args));
    for path in [&manifest, &lock] {
      assert!(fs::read_to_string(path).unwrap().contains(&format!("version = \"{version}\"")));
    }
  }
  // Cargo formatting/build/run/test must not rewrite catalog-managed sources.
  for name in files {
    assert_eq!(
      fs::read(fixture.join(".tasks/_/rust").join(name)).unwrap(),
      fs::read(root.join("shared-tasks/rust").join(name)).unwrap()
    );
  }
  fs::write(&source, "fn main() { let unused = 1; }\n").unwrap();
  let output = invoke("_/rust/run", &[]);
  assert!(!output.status.success());
  assert!(
    !String::from_utf8_lossy(&output.stdout).contains("Private-content check passed:"),
    "Clippy must stop before build/run"
  );
  fs::write(&source, "fn main() { println!(\"/home/private/build-machine\"); }\n").unwrap();
  let output = invoke("_/rust/test", &[]);
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("Private content"));
  assert!(
    !String::from_utf8_lossy(&output.stdout).contains("test result:"),
    "Scan failure must stop before tests"
  );
}

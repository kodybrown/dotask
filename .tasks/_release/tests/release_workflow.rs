//! Execute the publisher's actual inline script against a fake gh. No network,
//! tag creation, or release publication is possible in these tests.
use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};

fn workflow() -> Value {
  serde_saphyr::from_str(
    &fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/release.yml")).unwrap(),
  )
  .unwrap()
}
#[test]
fn main_pushes_publish_manual_runs_only_build_and_only_publisher_can_write() {
  let data = workflow();
  assert!(data["on"].get("push").is_some());
  assert_eq!(data["on"]["push"]["branches"], serde_json::json!(["**"]));
  assert!(data["on"]["push"].get("tags").is_none());
  assert!(data["on"].get("pull_request").is_some());
  assert!(data["on"].get("workflow_dispatch").is_some());
  assert_eq!(data["permissions"]["contents"], "read");
  assert_eq!(
    data["jobs"]["publish"]["if"],
    "github.event_name == 'push' && github.ref == 'refs/heads/main'"
  );
  for (name, job) in data["jobs"].as_object().unwrap() {
    if name == "publish" {
      assert_eq!(job["permissions"]["contents"], "write");
    } else {
      assert!(job.get("permissions").is_none());
    }
  }
  assert_eq!(data["jobs"]["collect"]["needs"], serde_json::json!(["native"]));
  assert!(data["jobs"].get("macos").is_none());
  let platforms: Vec<_> = data["jobs"]["native"]["strategy"]["matrix"]["include"]
    .as_array()
    .unwrap()
    .iter()
    .map(|entry| entry["platform"].as_str().unwrap())
    .collect();
  assert_eq!(platforms, ["windows", "linux"]);
  assert_eq!(data["jobs"]["publish"]["needs"], "collect");
  let windows_package = data["jobs"]["native"]["steps"]
    .as_array()
    .unwrap()
    .iter()
    .find(|step| {
      step["run"].as_str().is_some_and(|run| run.contains("create-dotask-installer"))
        && step["if"] == "runner.os == 'Windows'"
    })
    .unwrap();
  // A Bash -> cmd /c command string embeds escaped quotes in typed paths.
  // The native cmd shell owns Windows quoting without an extra interpreter.
  assert_eq!(windows_package["shell"], "cmd");
  assert!(windows_package["run"]
    .as_str()
    .unwrap()
    .contains("--output \"%RELEASE_OUTPUT%\""));
  for job in data["jobs"].as_object().unwrap().values() {
    for step in job["steps"].as_array().unwrap() {
      if let Some(action) = step["uses"].as_str() {
        let (_, commit) = action.split_once('@').unwrap();
        assert!(
          commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()),
          "Unpinned action: {action}"
        );
      }
    }
  }
}
#[test]
fn publisher_creates_exact_tested_release_and_refuses_version_reuse_or_partial_drafts() {
  let bash = if cfg!(windows) {
    let git = Command::new("git").arg("--exec-path").output().unwrap();
    PathBuf::from(String::from_utf8(git.stdout).unwrap().trim())
      .ancestors()
      .nth(3)
      .unwrap()
      .join("bin/bash.exe")
  } else {
    PathBuf::from("bash")
  };
  assert!(
    Command::new(&bash)
      .args(["-c", "command -v jq"])
      .output()
      .is_ok_and(|v| v.status.success()),
    "Publisher shell acceptance requires Bash and jq"
  );
  let data = workflow();
  let script = data["jobs"]["publish"]["steps"].as_array().unwrap().last().unwrap()["run"]
    .as_str()
    .unwrap();
  let work = tempfile::Builder::new().prefix("doinstall publisher ").tempdir().unwrap();
  let script_path = work.path().join("publish.sh");
  // A shell function takes precedence over every executable on PATH, including
  // Windows/MSYS PATH conversion. Real gh can never be reached by this script.
  fs::write(
    &script_path,
    format!("gh() {{ bash \"$TEST_GH\" \"$@\"; }}\nTMPDIR=\"$TEST_SHELL_TEMP\"\nexport TMPDIR\n{script}"),
  )
  .unwrap();
  let tools = work.path().join("tools");
  let assets = work.path().join("assets");
  let runner = work.path().join("runner");
  fs::create_dir(&tools).unwrap();
  fs::create_dir(&assets).unwrap();
  fs::create_dir(&runner).unwrap();
  let commit = "a".repeat(40);
  fs::write(
    assets.join("RELEASE-METADATA.json"),
    format!(r#"{{"tag":"v1.2.3","source_commit":"{commit}"}}"#),
  )
  .unwrap();
  fs::write(
    tools.join("gh"),
    r##"#!/usr/bin/env bash
set -eu
printf '%s\n' "$*" >> "$TEST_LOG"
case "$1 $2" in
  'api repos/example/tool/git/matching-refs/tags/v1.2.3')
    [[ "$TEST_CASE" != network ]] || exit 1
    if [[ "$TEST_CASE" == new || "$TEST_CASE" == upload-failure ]]; then exit 0; fi
    target="$COMMIT"
    [[ "$TEST_CASE" != mismatch ]] || target=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
    printf '{"object":{"type":"commit","sha":"%s"}}\n' "$target"
    ;;
  'api repos/example/tool/releases')
    if [[ "$TEST_CASE" == existing || "$TEST_CASE" == metadata-mismatch || "$TEST_CASE" == draft ]]; then
      draft=false; [[ "$TEST_CASE" != draft ]] || draft=true
      printf '{"tag_name":"v1.2.3","draft":%s}\n' "$draft"
    fi
    ;;
  'release download')
    target="$COMMIT"; [[ "$TEST_CASE" != metadata-mismatch ]] || target=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
    printf '{"tag":"v1.2.3","source_commit":"%s"}\n' "$target" > "$7/RELEASE-METADATA.json"
    ;;
  'release create')
    [[ "$*" == *"--target $COMMIT"* ]] || exit 90
    [[ "$*" == *"--draft"* ]] || exit 91
    [[ "$TEST_CASE" != upload-failure ]] || exit 1
    ;;
  'release edit') [[ "$*" == 'release edit v1.2.3 --draft=false' ]] || exit 92 ;;
  *) echo "Unexpected mock gh arguments: $*" >&2; exit 99 ;;
esac
"##,
  )
  .unwrap();
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(tools.join("gh"), fs::Permissions::from_mode(0o755)).unwrap();
  }
  let log = work.path().join("commands.log");
  // Windows CI disables MSYS argument conversion for the outer cmd.exe call.
  // Keep that condition in this regression and force an MSYS-only path that
  // native jq cannot open. Local /tmp junctions must not hide the CI mismatch.
  // Set it after Bash startup, which may normalize an inherited TMPDIR. The
  // shell owns file access; native tools must receive metadata through stdin.
  // Everything still resides inside the fixture and is removed by TempDir.
  let native_root = work.path().to_string_lossy().replace('\\', "/");
  let shell_root = if cfg!(windows) {
    let (drive, tail) = native_root.split_once(":/").expect("Windows fixture needs a drive path");
    format!("/proc/cygdrive/{}/{tail}", drive.to_ascii_lowercase())
  } else {
    native_root
  };
  for case in [
    "new",
    "existing",
    "mismatch",
    "network",
    "draft",
    "metadata-mismatch",
    "upload-failure",
  ] {
    fs::write(&log, "").unwrap();
    let mut command = Command::new(&bash);
    if cfg!(windows) {
      command.env("MSYS_NO_PATHCONV", "1");
    }
    let output = command
      .arg(&script_path)
      .env("TEST_LOG", &log)
      .env("TEST_GH", tools.join("gh"))
      .env("TEST_CASE", case)
      .env("COMMIT", &commit)
      .env("GH_REPO", "example/tool")
      .env("ASSETS", format!("{shell_root}/assets"))
      .env("RUNNER_TEMP", format!("{shell_root}/runner"))
      .env("TEST_SHELL_TEMP", format!("{shell_root}/runner"))
      .output()
      .unwrap();
    assert_eq!(
      output.status.success(),
      ["new", "existing"].contains(&case),
      "{case}: {}{}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
    let commands = fs::read_to_string(&log).unwrap();
    if cfg!(windows) && ["existing", "metadata-mismatch"].contains(&case) {
      assert!(
        commands.contains("--dir /proc/cygdrive/"),
        "Must exercise an MSYS-only download path"
      );
    }
    assert_eq!(
      commands.contains("release create"),
      ["new", "upload-failure"].contains(&case),
      "{case}: {commands}"
    );
    assert_eq!(commands.contains("release edit"), case == "new", "{case}: {commands}");
    assert!(!commands.contains("--clobber") && !commands.contains("--force") && !commands.contains("release delete"));
  }
}

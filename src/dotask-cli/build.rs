#[path = "../../.tasks/_support/BuildInfo.rs"]
mod build_info;
use std::{env, path::PathBuf, process::Command};
fn main() {
  let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
  for name in [
    "DOTASK_BUILD_STAMP",
    "DOTASK_BUILD_COMMIT",
    "DOTASK_BUILD_DIRTY",
    "DOTASK_APP_VERSION",
  ] {
    println!("cargo:rerun-if-env-changed={name}");
  }
  for path in ["src", "Cargo.toml", "../../.tasks/_support/BuildInfo.rs"] {
    println!("cargo:rerun-if-changed={path}");
  }
  // Git state changes independently of Rust source. Watch the actual metadata
  // paths so worktree checkouts, commits, and staging invalidate build metadata.
  for name in ["HEAD", "index", "packed-refs"] {
    if let Ok(output) = Command::new("git")
      .args(["rev-parse", "--git-path", name])
      .current_dir(&root)
      .output()
      && output.status.success()
    {
      let path = root.join(String::from_utf8(output.stdout).unwrap().trim());
      // Watching an absent packed-refs file makes Cargo rerun this script on
      // every invocation. Existing loose refs are watched separately below.
      if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
      }
    }
  }
  if let Ok(output) = Command::new("git")
    .args(["symbolic-ref", "-q", "HEAD"])
    .current_dir(&root)
    .output()
    && output.status.success()
  {
    let reference = String::from_utf8(output.stdout).unwrap();
    if let Ok(output) = Command::new("git")
      .args(["rev-parse", "--git-path", reference.trim()])
      .current_dir(&root)
      .output()
      && output.status.success()
    {
      println!(
        "cargo:rerun-if-changed={}",
        root.join(String::from_utf8(output.stdout).unwrap().trim()).display()
      );
    }
  }
  let stamp = env::var("DOTASK_BUILD_STAMP")
    .ok()
    .map(|v| build_info::Stamp::parse(&v).expect("Invalid DOTASK_BUILD_STAMP"))
    .unwrap_or_else(build_info::Stamp::now);
  let (git_commit, git_dirty) = build_info::git(&root);
  let commit = env::var("DOTASK_BUILD_COMMIT").unwrap_or(git_commit);
  assert!(
    commit.is_empty() || commit.len() >= 7 && commit.len() <= 64 && commit.chars().all(|c| c.is_ascii_hexdigit()),
    "Invalid build commit"
  );
  let dirty = !commit.is_empty()
    && env::var("DOTASK_BUILD_DIRTY")
      .ok()
      .map(|v| v.parse::<bool>().expect("Invalid build dirty flag"))
      .unwrap_or(git_dirty);
  let package = env::var("CARGO_PKG_VERSION").unwrap();
  let prefix = package.split('.').take(2).collect::<Vec<_>>().join(".");
  let version = env::var("DOTASK_APP_VERSION").unwrap_or_else(|_| stamp.version(&prefix));
  assert!(
    !version.trim().is_empty() && !version.contains(['\n', '\r']),
    "Invalid app version"
  );
  for (name, value) in [
    ("DOTASK_APP_VERSION", version),
    ("DOTASK_BUILD_STAMP", stamp.text()),
    ("DOTASK_BUILD_COMMIT", commit),
    ("DOTASK_BUILD_DIRTY", dirty.to_string()),
  ] {
    println!("cargo:rustc-env={name}={value}");
  }
}

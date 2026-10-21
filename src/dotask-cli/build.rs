#[path = "src/build_identity.rs"]
mod build_identity;
#[path = "../../.tasks/_support/BuildInfo.rs"]
mod build_info;
use std::{env, path::PathBuf, process::Command};
fn main() {
  let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
  println!("cargo:rerun-if-env-changed=RUST_BUILD_TIMESTAMP");
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
  let seconds = env::var("RUST_BUILD_TIMESTAMP")
    .map(|value| value.parse::<u64>().expect("RUST_BUILD_TIMESTAMP must be Unix seconds"))
    .unwrap_or_else(|_| build_identity::now());
  let stamp = build_info::Stamp::from_minutes(seconds / 60);
  let (commit, dirty) = build_info::git(&root);
  assert!(
    commit.is_empty() || commit.len() >= 7 && commit.len() <= 64 && commit.chars().all(|c| c.is_ascii_hexdigit()),
    "Invalid build commit"
  );
  let package = env::var("CARGO_PKG_VERSION").unwrap();
  // Cargo owns version precedence. Human UTC build metadata never increments
  // the package or changes the installer version.
  let (metadata, utc) = build_identity::timestamp(seconds);
  for (name, value) in [
    ("DOTASK_DISPLAY_VERSION", format!("{package}+{metadata}")),
    ("DOTASK_BUILD_UTC", utc),
    ("DOTASK_BUILD_UNIX_SECONDS", seconds.to_string()),
    ("DOTASK_APP_VERSION", package),
    ("DOTASK_BUILD_STAMP", stamp.text()),
    ("DOTASK_BUILD_COMMIT", commit),
    ("DOTASK_BUILD_DIRTY", dirty.to_string()),
  ] {
    println!("cargo:rustc-env={name}={value}");
  }
}

//! ---
//! description: Build the bundled Windows shims with LLVM, or verify their recorded hashes.
//! options:
//!   - { name: verify, type: bool, default: false, description: Check source and binary hashes without requiring LLVM. }
//! examples: [dotask shim --verify]
//! ---
use dotask_sdk::{
  bail, json, serde_json,
  sha2::{Digest, Sha256},
  tempfile, BuildContext, Result,
};
use std::fs;
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let root = project.path("src/Dotask.Shim");
  let assets = root.join("assets");
  let architectures = [
    ("x64", "x86_64", "i386:x86-64"),
    ("arm64", "aarch64", "arm64"),
  ];
  if !project.boolean("verify")? {
    let temporary = tempfile::Builder::new().prefix("dotask-shim-").tempdir()?;
    fs::create_dir_all(&assets)?;
    for (name, triple, machine) in architectures {
      let library = temporary.path().join(format!("kernel32-{name}.lib"));
      let object = temporary.path().join(format!("shim-{name}.obj"));
      project.execute(
        project
          .command("llvm-dlltool")
          .args(["-m", machine, "-d"])
          .arg(root.join("kernel32.def"))
          .arg("-l")
          .arg(&library),
      )?;
      project.execute(
        project
          .command("clang")
          .arg(format!("--target={triple}-pc-windows-msvc"))
          .arg("-c")
          .arg(root.join("shim.c"))
          .arg("-o")
          .arg(&object)
          .args([
            "-Os",
            "-ffreestanding",
            "-fno-builtin",
            "-fno-stack-protector",
            "-Wall",
            "-Wextra",
            "-Werror",
          ]),
      )?;
      project.execute(
        project
          .command("lld-link")
          .arg(&object)
          .arg(&library)
          .args([
            "/entry:mainCRTStartup",
            "/subsystem:console",
            "/nodefaultlib",
            "/dynamicbase",
            "/nxcompat",
            "/timestamp:0",
            "/Brepro",
          ])
          .arg(format!("/out:{}", assets.join(format!("win-{name}.exe")).display())),
      )?;
    }
  }
  let mut hashes = serde_json::Map::new();
  for name in [
    "shim.c",
    "kernel32.def",
    "assets/win-x64.exe",
    "assets/win-arm64.exe",
  ] {
    hashes.insert(name.into(), json!(format!("{:x}", Sha256::digest(fs::read(root.join(name))?))));
  }
  hashes.insert(
    "build-task.rs".into(),
    json!(format!("{:x}", Sha256::digest(fs::read(project.task_file())?))),
  );
  let record = serde_json::to_string_pretty(&hashes)? + "\n";
  let manifest = assets.join("hashes.json");
  if project.boolean("verify")? {
    if !manifest.is_file() || fs::read_to_string(manifest)? != record {
      bail!("Shim sources or bundled binaries changed. Rebuild with dotask shim and review the binaries and hashes together.");
    }
    println!("Bundled shim hashes match their recorded sources and binaries.");
  } else {
    fs::write(manifest, record)?;
  }
  Ok(())
}

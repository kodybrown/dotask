//! Copyable Rust task operations. No product names, repositories or SDK paths.
#![allow(dead_code)]
use dotask_sdk::{bail, json, serde_json, BuildContext, Context, Result, Value};
use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::PathBuf,
  time::{SystemTime, UNIX_EPOCH},
};
fn string(p: &BuildContext, name: &str, default: &str) -> String {
  p.string(name).unwrap_or(default).to_owned()
}
fn flag(p: &BuildContext, name: &str) -> bool {
  p.boolean(name).unwrap_or(false)
}
pub fn metadata(p: &BuildContext) -> Result<Value> {
  Ok(serde_json::from_str(
    &p.capture(
      p.command("cargo")
        .args([
          "metadata",
          "--format-version",
          "1",
          "--no-deps",
          "--locked",
          "--manifest-path",
        ])
        .arg(p.path(string(p, "manifest", "Cargo.toml"))),
    )?,
  )?)
}
fn package<'a>(p: &BuildContext, data: &'a Value) -> Result<&'a Value> {
  let manifest = fs::canonicalize(p.path(string(p, "manifest", "Cargo.toml")))?;
  let name = string(p, "package", "");
  data["packages"]
    .as_array()
    .context("Missing Cargo packages")?
    .iter()
    .find(|v| {
      if name.is_empty() {
        v["manifest_path"]
          .as_str()
          .and_then(|v| fs::canonicalize(v).ok())
          .is_some_and(|v| v == manifest)
      } else {
        v["name"] == name
      }
    })
    .context("Select a package with --package when using a virtual workspace")
}
fn parts(version: &str) -> Result<[u64; 3]> {
  let values: Vec<_> = version.split('.').collect();
  if values.len() != 3
    || values
      .iter()
      .any(|v| v.is_empty() || v.len() > 1 && v.starts_with('0') || !v.bytes().all(|b| b.is_ascii_digit()))
  {
    bail!("Version must be MAJOR.MINOR.PATCH");
  }
  Ok([values[0].parse()?, values[1].parse()?, values[2].parse()?])
}
fn seconds() -> Result<u64> {
  Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn calendar(seconds: u64) -> (String, String) {
  let mut days = seconds / 86400;
  let mut year = 1970;
  let leap = |y| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
  while days >= if leap(year) { 366 } else { 365 } {
    days -= if leap(year) { 366 } else { 365 };
    year += 1;
  }
  let mut month = 1;
  for count in [
    31,
    if leap(year) { 29 } else { 28 },
    31,
    30,
    31,
    30,
    31,
    31,
    30,
    31,
    30,
    31,
  ] {
    if days < count {
      break;
    }
    days -= count;
    month += 1;
  }
  (
    format!("{:02}{month:02}.{:02}{:02}", year % 100, days + 1, seconds % 86400 / 3600),
    format!(
      "{year:04}-{month:02}-{:02}T{:02}:{:02}:{:02}Z",
      days + 1,
      seconds % 86400 / 3600,
      seconds % 3600 / 60,
      seconds % 60
    ),
  )
}
fn target(p: &BuildContext, data: &Value) -> Result<PathBuf> {
  let path = PathBuf::from(data["target_directory"].as_str().context("Missing target_directory")?);
  if std::path::absolute(&path)?.starts_with(std::path::absolute(p.root())?) {
    bail!("Cargo target_directory must be outside source");
  }
  fs::create_dir_all(&path)?;
  let path = fs::canonicalize(path)?;
  if path.starts_with(fs::canonicalize(p.root())?) {
    bail!("Cargo target_directory must be outside source");
  }
  if let Some(build) = data["build_directory"].as_str()
    && std::path::absolute(build)?.starts_with(std::path::absolute(p.root())?)
  {
    bail!("Cargo build_directory must be outside source");
  }
  Ok(path)
}
fn record(p: &BuildContext, data: &Value, seconds: u64) -> Result<()> {
  let package = package(p, data)?;
  let version = package["version"].as_str().context("Missing version")?;
  parts(version)?;
  let (stamp, utc) = calendar(seconds);
  let directory = target(p, data)?.join("rust-build");
  fs::create_dir_all(&directory)?;
  let path = directory.join(format!("{}.json", package["name"].as_str().unwrap()));
  fs::write(
    &path,
    serde_json::to_string_pretty(
      &json!({"version":version,"display_version":format!("{version}+{stamp}"),"built_utc":utc,"unix_seconds":seconds}),
    )? + "\n",
  )?;
  println!("{}+{stamp} (built {utc}); metadata: {}", version, path.display());
  Ok(())
}
/// Replace only the selected table's literal field, retaining BOM, EOLs, comments,
/// and all unrelated packages. Inherited/nonliteral versions are refused.
fn edit_version(text: &str, table: &str, old: &str, new: &str) -> Result<String> {
  let mut active = false;
  let mut result = String::new();
  let mut count = 0;
  for line in text.split_inclusive('\n') {
    let trimmed = line.trim_start_matches('\u{feff}').trim();
    if trimmed.starts_with('[') {
      active = trimmed == table;
    }
    if active && trimmed.starts_with("version") && trimmed[7..].trim_start().starts_with('=') {
      let value = line.find('=').unwrap() + 1;
      let field = line[value..].trim_start();
      let literal = format!("\"{old}\"");
      if !field.starts_with(&literal)
        || !field[literal.len()..].trim().is_empty() && !field[literal.len()..].trim().starts_with('#')
      {
        bail!("Version field changed or is not a simple string");
      }
      let offset = value + line[value..].len() - field.len();
      result.push_str(&line[..offset]);
      result.push_str(&format!("\"{new}\""));
      result.push_str(&line[offset + literal.len()..]);
      count += 1;
    } else {
      result.push_str(line);
    }
  }
  if count != 1 {
    bail!("Expected one literal version in {table}");
  }
  Ok(result)
}
pub fn update(p: &BuildContext) -> Result<()> {
  let data = metadata(p)?;
  let package = package(p, &data)?;
  let old = package["version"].as_str().unwrap();
  let mut version = parts(old)?;
  let mut flags = [
    flag(p, "increment-major"),
    flag(p, "increment-minor"),
    flag(p, "increment"),
  ];
  if flags.iter().filter(|v| **v).count() > 1 {
    bail!("Choose only one increment flag");
  }
  // Version changes happen only through this explicitly invoked task. With no
  // selector the user's release-preparation action means one patch increment.
  if !flags.iter().any(|v| *v) {
    flags[2] = true;
  }
  if let Some(index) = flags.iter().position(|v| *v) {
    version[index] = version[index].checked_add(1).context("Version overflow")?;
    version[index + 1..].fill(0);
    let new = format!("{}.{}.{}", version[0], version[1], version[2]);
    println!("{old} -> {new}");
    let manifest = PathBuf::from(package["manifest_path"].as_str().unwrap());
    let lock = PathBuf::from(data["workspace_root"].as_str().unwrap()).join("Cargo.lock");
    let source = fs::read_to_string(&manifest)?;
    let changed = edit_version(&source, "[package]", old, &new)?;
    let locked = fs::read_to_string(&lock)?;
    let name = package["name"].as_str().unwrap();
    let chunks: Vec<_> = locked.match_indices("[[package]]").map(|(i, _)| i).collect();
    let mut lock_changed = None;
    for (i, start) in chunks.iter().enumerate() {
      let end = chunks.get(i + 1).copied().unwrap_or(locked.len());
      let block = &locked[*start..end];
      if block.lines().any(|v| v.trim() == format!("name = \"{name}\""))
        && !block.lines().any(|v| v.trim_start().starts_with("source ="))
      {
        if lock_changed.is_some() {
          bail!("Ambiguous local lockfile package");
        }
        lock_changed = Some(format!(
          "{}{}{}",
          &locked[..*start],
          edit_version(block, "[[package]]", old, &new)?,
          &locked[end..]
        ));
      }
    }
    let lock_changed = lock_changed.context("Missing local package in Cargo.lock")?;
    if flag(p, "dry-run") {
      return Ok(());
    }
    // Lock both inputs, then recheck their bytes before writing. Failures restore
    // the manifest rather than leaving a half-applied package-version update.
    let mut first = fs::OpenOptions::new().read(true).write(true).open(&manifest)?;
    let mut second = fs::OpenOptions::new().read(true).write(true).open(&lock)?;
    first.try_lock()?;
    second.try_lock()?;
    use std::io::{Read, Seek, SeekFrom, Write};
    // Windows byte-range locks also block new handles in this process. Re-read
    // through the locked handles themselves before applying either replacement.
    let mut current = String::new();
    first.read_to_string(&mut current)?;
    let mut current_lock = String::new();
    second.read_to_string(&mut current_lock)?;
    if current != source || current_lock != locked {
      bail!("Version files changed concurrently");
    }
    let replace = |file: &mut fs::File, text: &str| -> std::io::Result<()> {
      file.seek(SeekFrom::Start(0))?;
      file.write_all(text.as_bytes())?;
      file.set_len(text.len() as u64)?;
      file.sync_all()
    };
    replace(&mut first, &changed)?;
    if let Err(error) = replace(&mut second, &lock_changed) {
      replace(&mut first, &source)?;
      replace(&mut second, &locked)?;
      return Err(error.into());
    }
    drop(first);
    drop(second);
    match metadata(p) {
      Ok(_) => (),
      Err(error) => {
        if fs::read_to_string(&manifest)? == changed && fs::read_to_string(&lock)? == lock_changed {
          fs::write(&manifest, source)?;
          fs::write(&lock, locked)?;
        }
        return Err(error);
      }
    }
    return Ok(());
  }
  Ok(())
}
fn privacy(data: &[u8], markers: &[String]) -> Result<()> {
  for text in [
    String::from_utf8_lossy(data).into_owned(),
    String::from_utf16_lossy(
      &data
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect::<Vec<_>>(),
    ),
  ] {
    let normalized = text.replace('\\', "/").to_lowercase();
    if markers
      .iter()
      .any(|v| normalized.contains(&v.replace('\\', "/").trim_start_matches("//?/").to_lowercase()))
    {
      bail!("Private build path in compiled executable");
    }
    for private in [
      "/home/",
      "/users/",
      "/root/.cargo/",
      "/github/workspace/",
      "/__w/",
      ":/tmp/",
      ":/agent/",
      ":/actions-runner/",
      "-----begin private key-----",
      "-----begin rsa private key-----",
      "-----begin openssh private key-----",
      "-----begin ec private key-----",
    ] {
      if normalized.contains(private) {
        bail!("Private content in compiled executable");
      }
    }
    for token in text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
      if ["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"]
        .iter()
        .any(|prefix| token.starts_with(prefix) && token.len() >= 34)
        || token.starts_with("AKIA") && token.len() == 20
      {
        bail!("Credential pattern in compiled executable");
      }
    }
    for key in [
      "github_token",
      "actions_runtime_token",
      "aws_secret_access_key",
    ] {
      for (_, tail) in normalized.match_indices(key).map(|(i, k)| (i, &normalized[i + k.len()..])) {
        if tail.trim_start().starts_with('=') && !tail.trim_start()[1..].trim().is_empty() {
          bail!("Captured environment value in executable");
        }
      }
    }
  }
  Ok(())
}
pub fn operation(p: &BuildContext) -> Result<()> {
  let action = p.string("operation")?;
  if action == "run" {
    let data = metadata(p)?;
    let package = package(p, &data)?;
    let profile = string(p, "profile", "release");
    let report: Value = serde_json::from_slice(&fs::read(build_report(p, &data)?)?)?;
    if report["profile"] != profile || report["version"] != package["version"] {
      bail!("The selected package/profile must be built before running");
    }
    let binaries = report["binaries"].as_object().context("Missing built executable inventory")?;
    let requested = string(p, "bin", "");
    let selected = if !requested.is_empty() {
      requested
    } else if let Some(default) = package["default_run"].as_str() {
      default.to_owned()
    } else if binaries.len() == 1 {
      binaries.keys().next().unwrap().clone()
    } else {
      bail!("Select the executable with --bin");
    };
    let binary = binaries
      .get(&selected)
      .and_then(Value::as_str)
      .context("Executable was not built/scanned")?;
    let arguments: Vec<String> =
      serde_json::from_str(&string(p, "args", "[]")).context("--args must be a JSON array of strings")?;
    // Execute Cargo's reported artifact directly: another cargo run could
    // recompile with different flags and bypass the just-completed scan.
    return p.execute(p.command(binary).args(arguments));
  }
  let mut command = p.command("cargo");
  command.arg(action);
  match action {
    "fmt" => {
      command
        .args(["--all", "--manifest-path"])
        .arg(p.path(string(p, "manifest", "Cargo.toml")));
      if flag(p, "check") {
        command.args(["--", "--check"]);
      }
    }
    "test" | "clippy" => {
      // Clippy and tests compile too; enforce output policy before invoking
      // either rather than discovering a source-local target during build.
      target(p, &metadata(p)?)?;
      command.arg("--locked");
      let name = string(p, "package", "");
      if name.is_empty() {
        command.arg("--workspace");
      } else {
        command.args(["--package", &name]);
      }
      command.arg("--manifest-path").arg(p.path(string(p, "manifest", "Cargo.toml")));
      if action == "test" {
        command.arg("--release");
      } else {
        command.args(["--all-targets", "--", "-D", "warnings"]);
      }
    }
    "build" => {
      let data = metadata(p)?;
      target(p, &data)?;
      let package = package(p, &data)?;
      let seconds = seconds()?;
      record(p, &data, seconds)?;
      command
        .args(["--locked", "--profile"])
        .arg(string(p, "profile", "release"))
        .args([
          "--package",
          package["name"].as_str().unwrap(),
          "--manifest-path",
        ])
        .arg(p.path(string(p, "manifest", "Cargo.toml")))
        .arg("--message-format=json-render-diagnostics");
      let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME"))?;
      let sysroot = p.capture(p.command("rustc").args(["--print", "sysroot"]))?.trim().to_owned();
      let cargo = std::env::var("CARGO_HOME").unwrap_or_else(|_| format!("{home}/.cargo"));
      let markers = vec![
        p.root().to_string_lossy().into_owned(),
        home,
        cargo,
        sysroot,
        data["target_directory"].as_str().unwrap().into(),
      ];
      let mut flags = Vec::new();
      for (private, public) in markers
        .iter()
        .take(4)
        .zip(["/source", "/build-home", "/cargo", "/rust-toolchain"])
      {
        for spelling in BTreeSet::from([
          private.clone(),
          private.replace('\\', "/"),
          private.trim_start_matches(r"\\?\").into(),
        ]) {
          flags.push(format!("--remap-path-prefix={spelling}={public}"));
        }
      }
      if cfg!(windows) {
        flags.extend(
          [
            "-C",
            "target-feature=+crt-static",
            "-C",
            "link-arg=/PDBALTPATH:application.pdb",
          ]
          .map(String::from),
        );
      }
      command
        .env_remove("RUSTFLAGS")
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\x1f"))
        .env("RUST_BUILD_TIMESTAMP", seconds.to_string());
      let output = p.capture(&mut command)?;
      let mut binaries = BTreeMap::new();
      for line in output.lines() {
        if let Ok(record) = serde_json::from_str::<Value>(line) {
          if record["reason"] == "compiler-artifact"
            && let Some(path) = record["executable"].as_str()
            && record["target"]["kind"]
              .as_array()
              .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"))
          {
            binaries.insert(
              record["target"]["name"].as_str().context("Missing binary name")?.to_owned(),
              path.to_owned(),
            );
          }
          if let Some(message) = record["message"]["rendered"].as_str() {
            eprint!("{message}");
          }
        }
      }
      if binaries.is_empty() {
        bail!("Build produced no executable to scan");
      }
      for binary in binaries.values().map(PathBuf::from) {
        privacy(&fs::read(&binary)?, &markers)?;
        println!("Private-content check passed: {}", binary.display());
      }
      // Write only after every reported executable passed inspection. Run/test
      // orchestration stops on build failure and cannot consume an old success.
      fs::write(
        build_report(p, &data)?,
        serde_json::to_vec_pretty(&json!({
          "version": package["version"], "profile": string(p, "profile", "release"), "binaries": binaries
        }))?,
      )?;
      return Ok(());
    }
    _ => bail!("Unsupported Cargo operation"),
  }
  p.execute(&mut command)?;
  if action == "fmt" && flag(p, "format-tasks") {
    let mut fmt = p.command("rustfmt");
    fmt.args(["--edition", "2024"]);
    if flag(p, "check") {
      fmt.arg("--check");
    }
    for entry in fs::read_dir(p.task_file().parent().unwrap())? {
      let path = entry?.path();
      if path.extension().is_some_and(|v| v == "rs") {
        fmt.arg(path);
      }
    }
    p.execute(&mut fmt)?;
  }
  Ok(())
}
fn build_report(p: &BuildContext, data: &Value) -> Result<PathBuf> {
  let directory = target(p, data)?.join("rust-build");
  fs::create_dir_all(&directory)?;
  Ok(directory.join(format!("{}-executables.json", package(p, data)?["name"].as_str().unwrap())))
}
#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn stable_versions_refuse_ambiguous_forms() {
    assert_eq!(parts("12.34.56").unwrap(), [12, 34, 56]);
    for value in [
      "1.2",
      "1.2.3.4",
      "01.2.3",
      "1.2.3+date",
      "1.2.3-preview",
      "1.2.-3",
    ] {
      assert!(parts(value).is_err());
    }
  }
  #[test]
  fn version_edit_retains_source_bytes_outside_literal() {
    let before = "\u{feff}[package]\r\nname = \"example\"\r\nversion = \"0.1.0\" # keep\r\n\r\n[dependencies]\r\nversioned = \"unchanged\"\r\n";
    assert_eq!(
      edit_version(before, "[package]", "0.1.0", "0.2.0").unwrap(),
      before.replacen("\"0.1.0\"", "\"0.2.0\"", 1)
    );
    assert!(edit_version("[package]\nversion.workspace = true\n", "[package]", "0.1.0", "0.2.0").is_err());
    assert!(edit_version(before, "[package]", "9.9.9", "0.2.0").is_err());
  }
  #[test]
  fn privacy_check_covers_utf8_utf16_and_environment_values() {
    for text in [
      r"C:\Users\private\file",
      "-----BEGIN OPENSSH PRIVATE KEY-----",
      "GITHUB_TOKEN=captured",
      "github_pat_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    ] {
      for bytes in [
        text.as_bytes().to_vec(),
        text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
      ] {
        assert!(privacy(&bytes, &[]).is_err());
      }
    }
    assert!(privacy(b"/custom/private/build.rs", &["/custom/private".into()]).is_err());
    privacy(b"/source/src/main.rs /cargo/registry /rust-toolchain", &[]).unwrap();
  }
  #[test]
  fn calendar_uses_utc_and_preserves_leap_days() {
    assert_eq!(calendar(1709164800), ("2402.2900".into(), "2024-02-29T00:00:00Z".into()));
    assert_eq!(calendar(1704067200), ("2401.0100".into(), "2024-01-01T00:00:00Z".into()));
  }
}

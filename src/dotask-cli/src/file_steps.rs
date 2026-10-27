//! File changes use same-directory staging and explicit optimistic concurrency.
//! Existing text keeps its encoding, line endings and final-newline policy.
use crate::{
  declarative::keys,
  process_steps,
  project::{Directory, Target},
};
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
  fs,
  io::Write,
  path::{Path, PathBuf},
};

pub(crate) fn validate(action: &Value, target: &Target) -> Result<()> {
  let operation = action["operation"]
    .as_str()
    .ok_or_else(|| anyhow::anyhow!("file requires operation."))?;
  let extra: &[&str] = match operation {
    "read" => &["optional", "format"],
    "write" => &["content", "expected-sha256", "mode"],
    "render" => &["source", "values", "expected-sha256", "mode"],
    "replace" => &["find", "replacement", "expected-sha256", "mode"],
    "copy" => &["source", "expected-sha256", "mode"],
    "directory" => &["mode"],
    "frontmatter" => &[
      "create-if-missing",
      "body-if-empty",
      "initial",
      "fill-empty",
      "set",
      "match-existing",
      "expected-sha256",
      "mode",
    ],
    _ => bail!("Unknown file operation '{operation}'."),
  };
  let allowed = ["operation", "path"]
    .into_iter()
    .chain(extra.iter().copied())
    .collect::<Vec<_>>();
  keys(action, &allowed)?;
  if operation == "frontmatter" {
    crate::frontmatter::validate(action, target)?;
  }
  let path = action.get("path").ok_or_else(|| anyhow::anyhow!("file requires path."))?;
  process_steps::validate(path, target)?;
  for name in [
    "content",
    "source",
    "find",
    "replacement",
    "expected-sha256",
  ] {
    if let Some(value) = action.get(name) {
      process_steps::validate(value, target)?;
    }
  }
  for required in match operation {
    "write" => &["content"][..],
    "render" | "copy" => &["source"][..],
    "replace" => &["find", "replacement", "expected-sha256"][..],
    _ => &[],
  } {
    ensure!(action.get(*required).is_some(), "file {operation} requires {required}.");
  }
  if let Some(values) = action.get("values") {
    for (name, value) in values
      .as_object()
      .ok_or_else(|| anyhow::anyhow!("render values must be a mapping."))?
    {
      ensure!(crate::catalog::identifier(name), "Invalid template value name '{name}'.");
      process_steps::validate(value, target)?;
    }
  }
  if let Some(optional) = action.get("optional") {
    ensure!(optional.is_boolean(), "file optional must be boolean.");
  }
  if let Some(format) = action.get("format") {
    ensure!(
      matches!(format.as_str(), Some("text" | "json")),
      "read format must be text or json."
    );
  }
  if let Some(mode) = action.get("mode") {
    parse_mode(mode)?;
  }
  Ok(())
}

fn parse_mode(value: &Value) -> Result<u32> {
  let mode = value
    .as_str()
    .filter(|s| s.len() == 4 && s.starts_with('0'))
    .and_then(|s| u32::from_str_radix(s, 8).ok())
    .filter(|m| *m <= 0o777)
    .ok_or_else(|| anyhow::anyhow!("mode must be a quoted octal permission such as '0644'."))?;
  Ok(mode)
}

fn safe_path(path: &Path) -> Result<()> {
  let mut parent = Some(path);
  while let Some(part) = parent {
    match fs::symlink_metadata(part) {
      Ok(metadata) => {
        ensure!(
          !metadata.file_type().is_symlink(),
          "File actions refuse symbolic links: {}",
          part.display()
        );
        #[cfg(windows)]
        {
          use std::os::windows::fs::MetadataExt;
          ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "File actions refuse reparse points: {}",
            part.display()
          );
        }
      }
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(error) => return Err(error.into()),
    }
    parent = part.parent();
  }
  Ok(())
}
fn hash(bytes: &[u8]) -> String {
  format!("{:x}", Sha256::digest(bytes))
}
fn read(path: &Path) -> Result<Option<Vec<u8>>> {
  safe_path(path)?;
  if let Ok(metadata) = fs::metadata(path) {
    ensure!(
      metadata.is_file() && metadata.len() <= 16 * 1024 * 1024,
      "File must be regular and at most 16 MiB."
    );
  }
  match fs::read(path) {
    Ok(bytes) => {
      ensure!(bytes.len() <= 16 * 1024 * 1024, "File exceeds the 16 MiB file-action limit.");
      Ok(Some(bytes))
    }
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(error) => Err(error).with_context(|| format!("Read {}", path.display())),
  }
}
fn decode(bytes: &[u8]) -> Result<String> {
  if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
    ensure!(bytes.len().is_multiple_of(2), "Invalid UTF-16 file.");
    let little = bytes[0] == 0xff;
    let words = bytes[2..]
      .chunks_exact(2)
      .map(|b| {
        if little {
          u16::from_le_bytes([b[0], b[1]])
        } else {
          u16::from_be_bytes([b[0], b[1]])
        }
      })
      .collect::<Vec<_>>();
    Ok(String::from_utf16(&words)?)
  } else {
    Ok(String::from_utf8(
      bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes).to_vec(),
    )?)
  }
}
fn encode(text: &str, original: Option<&[u8]>) -> Result<Vec<u8>> {
  let mut text = text.to_owned();
  if let Some(original) = original {
    let old = decode(original)?;
    let without_crlf = old.replace("\r\n", "");
    ensure!(
      !(without_crlf.contains('\r') || old.contains("\r\n") && without_crlf.contains('\n')),
      "Cannot safely preserve mixed line endings."
    );
    let eol = if old.contains("\r\n") { "\r\n" } else { "\n" };
    text = text.replace("\r\n", "\n");
    ensure!(!text.contains('\r'), "Text contains an unsupported bare CR.");
    if old.ends_with('\n') {
      if !text.ends_with('\n') {
        text.push('\n');
      }
    } else {
      text = text.trim_end_matches('\n').to_owned();
    }
    text = text.replace('\n', eol);
    if original.starts_with(&[0xff, 0xfe]) || original.starts_with(&[0xfe, 0xff]) {
      let little = original[0] == 0xff;
      let mut bytes = original[..2].to_vec();
      for word in text.encode_utf16() {
        bytes.extend(if little {
          word.to_le_bytes()
        } else {
          word.to_be_bytes()
        });
      }
      return Ok(bytes);
    }
    if original.starts_with(&[0xef, 0xbb, 0xbf]) {
      return Ok([&[0xef, 0xbb, 0xbf][..], text.as_bytes()].concat());
    }
  }
  Ok(text.into_bytes())
}

pub(crate) fn render(
  template: &str,
  values: &serde_json::Map<String, Value>,
  resolve: impl Fn(&Value) -> Result<String>,
) -> Result<String> {
  let mut output = String::new();
  let mut remaining = template;
  while let Some(start) = remaining.find("{{") {
    output.push_str(&remaining[..start]);
    remaining = &remaining[start + 2..];
    let end = remaining
      .find("}}")
      .ok_or_else(|| anyhow::anyhow!("Unclosed template placeholder."))?;
    let name = remaining[..end].trim();
    let value = values
      .get(name)
      .ok_or_else(|| anyhow::anyhow!("Missing template value '{name}'."))?;
    output.push_str(&resolve(value)?);
    remaining = &remaining[end + 2..];
  }
  output.push_str(remaining);
  Ok(output)
}

/// A cooperating writer lock closes the read/stage/replace gap between dotask
/// processes. The final hash check also detects edits by non-cooperating writers.
struct Lock(PathBuf);
impl Drop for Lock {
  fn drop(&mut self) {
    let _ = fs::remove_file(&self.0);
  }
}
fn write(path: &Path, bytes: &[u8], expected: Option<&str>, mode: Option<u32>) -> Result<bool> {
  safe_path(path)?;
  let parent = path.parent().ok_or_else(|| anyhow::anyhow!("File has no parent."))?;
  fs::create_dir_all(parent)?;
  let lock_path = parent.join(format!(".{}.dotask.lock", path.file_name().unwrap().to_string_lossy()));
  fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(&lock_path)
    .context("File is locked by another writer; an interrupted writer's lock must be inspected before removal.")?;
  let _lock = Lock(lock_path);
  let original = read(path)?;
  let actual = original.as_ref().map(|b| hash(b)).unwrap_or_else(|| "missing".into());
  if let Some(expected) = expected {
    ensure!(expected == actual, "File changed since it was read: {}", path.display());
  }
  #[cfg(unix)]
  let mode_changed = {
    use std::os::unix::fs::PermissionsExt;
    mode.is_some_and(|desired| fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o777 != desired))
  };
  #[cfg(not(unix))]
  let mode_changed = false;
  if original.as_deref() == Some(bytes) && !mode_changed {
    return Ok(false);
  }
  ensure!(
    original.is_none() || expected.is_some(),
    "Changing an existing file requires expected-sha256: {}",
    path.display()
  );
  let mut staged = tempfile::NamedTempFile::new_in(parent)?;
  staged.write_all(bytes)?;
  if original.is_some() {
    staged.as_file().set_permissions(fs::metadata(path)?.permissions())?;
  }
  #[cfg(unix)]
  if let Some(mode) = mode {
    use std::os::unix::fs::PermissionsExt;
    staged.as_file().set_permissions(fs::Permissions::from_mode(mode))?;
  }
  #[cfg(not(unix))]
  let _ = mode;
  staged.as_file().sync_all()?;
  safe_path(path)?;
  ensure!(read(path)? == original, "File changed during staging: {}", path.display());
  if original.is_some() {
    staged.persist(path).map_err(|e| e.error)?;
  } else {
    staged.persist_noclobber(path).map_err(|e| e.error)?;
  }
  Ok(true)
}

pub(crate) fn execute(action: &Value, directory: &Directory, parameters: &Value, settings: &Value) -> Result<Value> {
  let resolve = |v: &Value| -> Result<String> {
    process_steps::resolve(v, directory, parameters, settings)?
      .into_string()
      .map_err(|_| anyhow::anyhow!("Text action requires UTF-8 values."))
  };
  let path =
    |v: &Value| process_steps::resolve_path(process_steps::resolve(v, directory, parameters, settings)?, directory);
  let destination = path(&action["path"])?;
  ensure!(destination.to_str().is_some(), "File action result paths require UTF-8.");
  safe_path(&destination)?;
  let operation = action["operation"].as_str().unwrap();
  let mode = action.get("mode").map(parse_mode).transpose()?;
  ensure!(cfg!(unix) || mode.is_none(), "Explicit POSIX mode is supported only on Unix.");
  if operation == "directory" {
    let exists = destination.is_dir();
    ensure!(!destination.exists() || exists, "Directory path is occupied by a file.");
    fs::create_dir_all(&destination)?;
    #[cfg(unix)]
    if !exists && let Some(mode) = mode {
      use std::os::unix::fs::PermissionsExt;
      fs::set_permissions(&destination, fs::Permissions::from_mode(mode))?;
    }
    return Ok(json!({"path":destination,"changed":!exists,"exists":true}));
  }
  let original = read(&destination)?;
  if operation == "frontmatter" {
    ensure!(
      original.is_some() || action["create-if-missing"] == true,
      "Frontmatter file does not exist: {}",
      destination.display()
    );
    let old_text = original.as_deref().map(decode).transpose()?;
    let text = crate::frontmatter::update(old_text.as_deref(), action)?;
    let bytes = encode(&text, original.as_deref())?;
    ensure!(bytes.len() <= 16 * 1024 * 1024, "File exceeds the 16 MiB file-action limit.");
    let observed = original.as_ref().map(|b| hash(b)).unwrap_or_else(|| "missing".into());
    let expected = action.get("expected-sha256").map(resolve).transpose()?.unwrap_or(observed);
    let changed = write(&destination, &bytes, Some(&expected), mode)?;
    let body_empty = crate::frontmatter::body_empty(&text)?;
    return Ok(
      json!({"path":destination,"exists":true,"changed":changed,"sha256":hash(&bytes),"body-empty":body_empty}),
    );
  }
  if operation == "read" {
    if let Some(bytes) = original {
      let content = decode(&bytes)?;
      let mut result = json!({"path":destination,"exists":true,"content":content,"sha256":hash(&bytes)});
      if action["format"] == "json" {
        result["data"] = serde_json::from_str(&content).context("File is not valid JSON.")?;
      }
      return Ok(result);
    }
    ensure!(action["optional"] == true, "File does not exist: {}", destination.display());
    return Ok(json!({"path":destination,"exists":false,"content":"","sha256":"missing"}));
  }
  let expected = action.get("expected-sha256").map(resolve).transpose()?;
  let bytes = match operation {
    "copy" => read(&path(&action["source"])?)?.ok_or_else(|| anyhow::anyhow!("Copy source does not exist."))?,
    "write" => encode(&resolve(&action["content"])?, original.as_deref())?,
    "render" => {
      let template =
        decode(&read(&path(&action["source"])?)?.ok_or_else(|| anyhow::anyhow!("Template does not exist."))?)?;
      let values = action.get("values").and_then(Value::as_object).cloned().unwrap_or_default();
      encode(&render(&template, &values, resolve)?, original.as_deref())?
    }
    "replace" => {
      let old = decode(
        original
          .as_deref()
          .ok_or_else(|| anyhow::anyhow!("Replace requires an existing file."))?,
      )?;
      let find = resolve(&action["find"])?;
      ensure!(
        !find.is_empty() && old.matches(&find).count() == 1,
        "replace requires exactly one literal match."
      );
      encode(&old.replacen(&find, &resolve(&action["replacement"])?, 1), original.as_deref())?
    }
    _ => unreachable!(),
  };
  ensure!(bytes.len() <= 16 * 1024 * 1024, "File exceeds the 16 MiB file-action limit.");
  let changed = write(&destination, &bytes, expected.as_deref(), mode)?;
  Ok(json!({"path":destination,"changed":changed,"exists":true,"sha256":hash(&bytes)}))
}

#[cfg(test)]
mod tests {
  use super::*;
  fn physical_temporary() -> tempfile::TempDir {
    // macOS exposes TMPDIR through /var -> /private/var. Resolve only this
    // system parent; links created inside the fixture must still be refused.
    tempfile::tempdir_in(fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap()
  }
  #[test]
  fn writes_are_guarded_and_idempotent() {
    let temp = physical_temporary();
    let path = temp.path().join("config");
    assert!(write(&path, b"first", Some("missing"), None).unwrap());
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert!(!write(&path, b"first", None, None).unwrap());
    assert_eq!(modified, fs::metadata(&path).unwrap().modified().unwrap());
    assert!(write(&path, b"second", None, None).is_err());
    assert!(write(&path, b"second", Some("incorrect"), None).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"first");
    assert!(write(&path, b"second", Some(&hash(b"first")), None).unwrap());
  }
  #[test]
  fn text_preserves_bom_crlf_and_final_newline() {
    let old = [b"\xef\xbb\xbf".as_slice(), b"a\r\nb\r\n"].concat();
    assert_eq!(
      encode("c\nd", Some(&old)).unwrap(),
      [b"\xef\xbb\xbf".as_slice(), b"c\r\nd\r\n"].concat()
    );
    let mut old = vec![0xff, 0xfe];
    old.extend("a\r\nb".encode_utf16().flat_map(u16::to_le_bytes));
    assert_eq!(decode(&encode("c\nd\n", Some(&old)).unwrap()).unwrap(), "c\r\nd");
    assert!(encode("new", Some(b"a\r\nb\nc")).is_err());
  }
  #[test]
  fn template_values_are_not_reinterpreted() {
    let values = json!({"name":"{{secret}}"});
    assert_eq!(
      render("{{name}}", values.as_object().unwrap(), |v| Ok(v.as_str().unwrap().into())).unwrap(),
      "{{secret}}"
    );
    assert!(render("{{missing}}", values.as_object().unwrap(), |_| Ok(String::new())).is_err());
  }
  #[cfg(unix)]
  #[test]
  fn links_and_linked_parents_are_refused() {
    use std::os::unix::fs::symlink;
    let temp = physical_temporary();
    let real = temp.path().join("real");
    fs::create_dir(&real).unwrap();
    symlink(&real, temp.path().join("alias")).unwrap();
    assert!(write(&temp.path().join("alias/file"), b"value", None, None).is_err());
    assert!(!real.join("file").exists());
  }
}

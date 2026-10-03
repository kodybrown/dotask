use anyhow::{bail, Result};
use sha2::{Digest, Sha256};
use std::{
  fs,
  io::Write,
  path::{Path, PathBuf},
};

pub(crate) fn hash(bytes: &[u8]) -> String {
  format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn is_hash(s: &str) -> bool {
  s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
pub(crate) fn hash_file(path: &Path) -> Result<Option<String>> {
  match fs::read(path) {
    Ok(b) => Ok(Some(hash(&b))),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound || path.is_dir() => Ok(None),
    Err(e) => Err(e.into()),
  }
}
fn device(s: &str) -> bool {
  let s = s.split('.').next().unwrap_or_default().to_ascii_uppercase();
  ["CON", "PRN", "AUX", "NUL"].contains(&s.as_str())
    || (s.len() == 4 && (s.starts_with("COM") || s.starts_with("LPT")) && matches!(s.as_bytes()[3], b'1'..=b'9'))
}
pub(crate) fn id(id: &str, parts: usize) -> Result<()> {
  let segments: Vec<_> = id.split('/').collect();
  if segments.len() != parts
    || segments.iter().enumerate().any(|(i, s)| {
      !(i == 0 && *s == "_" && (parts == 1 || parts == 3)) && (!crate::catalog::identifier(s) || device(s))
    })
  {
    bail!(
      "Invalid shared task identifier '{id}'. Use group/task names made of letters, digits, underscores, and hyphens."
    );
  }
  Ok(())
}
pub(crate) fn relative(path: &str) -> Result<()> {
  if path.split('/').any(|p| {
    p.is_empty()
      || p == "."
      || p == ".."
      || p.ends_with(['.', ' '])
      || device(p)
      || p.bytes().any(|b| !b.is_ascii_alphanumeric() && !b"_-. ".contains(&b))
  }) {
    bail!("Unsafe or non-portable shared task path '{path}'.");
  }
  Ok(())
}
pub(crate) fn check_link(path: &Path) -> Result<()> {
  match fs::symlink_metadata(path) {
    Ok(m) if crate::initialization::is_link(&m) => bail!(
      "Shared task operations refuse symbolic links/reparse points: {}",
      path.display()
    ),
    Ok(_) => Ok(()),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
    Err(e) => Err(e.into()),
  }
}
pub(crate) fn resolve(root: &Path, path: &str) -> Result<PathBuf> {
  relative(path)?;
  let mut current = root.to_path_buf();
  check_link(&current)?;
  for segment in path.split('/') {
    if current.is_dir() {
      let matches = fs::read_dir(&current)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name())
        .filter(|n| n.to_string_lossy().eq_ignore_ascii_case(segment))
        .collect::<Vec<_>>();
      if matches.len() > 1 || matches.iter().any(|n| n != segment) {
        bail!(
          "Path casing conflicts with an existing entry: {}",
          current.join(segment).display()
        );
      }
    }
    current.push(segment);
    check_link(&current)?;
  }
  Ok(current)
}
pub(crate) fn atomic(path: &Path, bytes: &[u8], replace: bool) -> Result<()> {
  let parent = path.parent().unwrap();
  fs::create_dir_all(parent)?;
  let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
  temporary.write_all(bytes)?;
  temporary.as_file().sync_all()?;
  if replace {
    temporary.persist(path)?;
  } else {
    temporary.persist_noclobber(path)?;
  }
  Ok(())
}
pub(crate) fn private_directory(path: &Path) -> Result<()> {
  fs::create_dir_all(path)?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
  }
  Ok(())
}
// Revisions are hashes of System.Text.Json's ordered, indented wire format.
// Preserve that format so switching CLI implementations does not invent updates.
pub(crate) fn dotnet_json(value: &serde_json::Value) -> String {
  use serde_json::Value;
  fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
      match c {
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        '\u{8}' => out.push_str("\\b"),
        '\u{c}' => out.push_str("\\f"),
        '\\' => out.push_str("\\\\"),
        c if c.is_ascii() && !c.is_control() && !"\"&'<>+`".contains(c) => out.push(c),
        c => {
          let mut buffer = [0u16; 2];
          for unit in c.encode_utf16(&mut buffer) {
            out.push_str(&format!("\\u{unit:04X}"));
          }
        }
      }
    }
    out.push('"');
    out
  }
  fn write(v: &Value, level: usize) -> String {
    let pad = "  ".repeat(level);
    let child = "  ".repeat(level + 1);
    match v {
      Value::String(s) => quote(s),
      Value::Object(o) if !o.is_empty() => format!(
        "{{\n{}\n{pad}}}",
        o.iter()
          .map(|(k, v)| format!("{child}{}: {}", quote(k), write(v, level + 1)))
          .collect::<Vec<_>>()
          .join(",\n")
      ),
      Value::Array(a) if !a.is_empty() => format!(
        "[\n{}\n{pad}]",
        a.iter()
          .map(|v| format!("{child}{}", write(v, level + 1)))
          .collect::<Vec<_>>()
          .join(",\n")
      ),
      _ => v.to_string(),
    }
  }
  let text = write(value, 0);
  if cfg!(windows) {
    text.replace('\n', "\r\n")
  } else {
    text
  }
}

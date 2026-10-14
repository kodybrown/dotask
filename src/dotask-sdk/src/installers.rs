//! Optional installer discovery belongs to the task SDK, not CLI discovery/help.
//! Configuration is user-owned. Successful-build state is separate, per platform,
//! and written only by an explicit recording operation after packaging succeeds.
use crate::{bail, json, lookup, validate_installer, BuildContext, Context, Result, Value};
use sha2::{Digest, Sha256};
use std::{
  cmp::Ordering,
  fs::{self, File},
  io::{BufRead, IsTerminal, Read, Write},
  path::{Component, Path, PathBuf},
  time::{SystemTime, UNIX_EPOCH},
};

#[cfg(any(windows, test))]
fn windows_argument(value: &str) -> String {
  // ShellExecute takes one Windows command line. Quote each token with the
  // inverse CommandLineToArgvW rules, including trailing slashes and empty args.
  let mut result = String::from("\"");
  let mut slashes = 0;
  for ch in value.chars() {
    if ch == '\\' {
      slashes += 1;
      continue;
    }
    result.push_str(&"\\".repeat(if ch == '"' { slashes * 2 + 1 } else { slashes }));
    slashes = 0;
    result.push(ch);
  }
  result.push_str(&"\\".repeat(slashes * 2));
  result.push('"');
  result
}
#[cfg(windows)]
pub(crate) fn run_windows_executable(file: &Path, arguments: &[String]) -> Result<i32> {
  use std::os::windows::ffi::OsStrExt;
  use windows::{
    core::{w, PCWSTR},
    Win32::{
      Foundation::{CloseHandle, WAIT_FAILED},
      System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE},
      UI::{
        Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW},
        WindowsAndMessaging::SW_SHOWNORMAL,
      },
    },
  };
  let executable: Vec<_> = file.as_os_str().encode_wide().chain(Some(0)).collect();
  let directory: Vec<_> = file.parent().unwrap().as_os_str().encode_wide().chain(Some(0)).collect();
  let parameters: Vec<_> = arguments
    .iter()
    .map(|s| windows_argument(s))
    .collect::<Vec<_>>()
    .join(" ")
    .encode_utf16()
    .chain(Some(0))
    .collect();
  let mut info = SHELLEXECUTEINFOW {
    cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
    fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
    lpVerb: w!("open"),
    lpFile: PCWSTR(executable.as_ptr()),
    lpParameters: PCWSTR(parameters.as_ptr()),
    lpDirectory: PCWSTR(directory.as_ptr()),
    nShow: SW_SHOWNORMAL.0,
    ..Default::default()
  };
  // Honor the installer's manifest without forcing elevation. MSI and scripts
  // use their explicit runners; this is only native Windows EXE activation.
  unsafe { ShellExecuteExW(&mut info) }.context("Cannot start installer")?;
  if info.hProcess.is_invalid() {
    bail!("Installer launched without a process handle; completion cannot be observed");
  }
  let result = (|| -> Result<i32> {
    if unsafe { WaitForSingleObject(info.hProcess, INFINITE) } == WAIT_FAILED {
      return Err(std::io::Error::last_os_error().into());
    }
    let mut code = 0;
    unsafe { GetExitCodeProcess(info.hProcess, &mut code) }?;
    Ok(code as i32)
  })();
  unsafe { CloseHandle(info.hProcess) }?;
  result
}

pub fn record_path(context: &BuildContext) -> PathBuf {
  // A project may itself be reached through a junction. Trust the selected
  // root, then reject linked state paths beneath its canonical location.
  fs::canonicalize(context.root())
    .unwrap_or_else(|_| context.root().to_owned())
    .join("artifacts/.dotask/installers")
    .join(format!("{}-{}.json", context.os(), context.architecture()))
}
fn is_link(metadata: &fs::Metadata) -> bool {
  let linked = metadata.file_type().is_symlink();
  #[cfg(windows)]
  {
    use std::os::windows::fs::MetadataExt;
    linked || metadata.file_attributes() & 0x400 != 0
  }
  #[cfg(not(windows))]
  {
    linked
  }
}
fn ordinary(path: &Path) -> bool {
  fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !is_link(&m))
}
fn no_links(path: &Path) -> Result<()> {
  for ancestor in path.ancestors() {
    match fs::symlink_metadata(ancestor) {
      Ok(metadata) if is_link(&metadata) => {
        bail!("Installer state/configuration cannot contain links: {}", ancestor.display())
      }
      Ok(_) => (),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
      Err(error) => return Err(error.into()),
    }
  }
  Ok(())
}
fn digest(path: &Path) -> Result<String> {
  let mut hash = Sha256::new();
  let mut input = File::open(path)?;
  let mut buffer = [0; 65536];
  loop {
    let count = input.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    hash.update(&buffer[..count]);
  }
  Ok(format!("{:x}", hash.finalize()))
}
fn state_lock(context: &BuildContext, name: &str) -> Result<File> {
  let directory = fs::canonicalize(context.root())?.join("artifacts/.dotask/installers");
  no_links(&directory)?;
  fs::create_dir_all(&directory)?;
  let path = directory.join(name);
  no_links(&path)?;
  let file = fs::OpenOptions::new()
    .create(true)
    .truncate(false)
    .read(true)
    .write(true)
    .open(path)?;
  file
    .try_lock()
    .context("Another installer state/configuration update is in progress")?;
  Ok(file)
}
fn read_record(context: &BuildContext) -> Result<Option<Value>> {
  let path = record_path(context);
  no_links(&path)?;
  if !path.try_exists()? {
    return Ok(None);
  }
  let value: Value = serde_json::from_slice(&fs::read(&path)?)
    .with_context(|| format!("Invalid installer record: {}", path.display()))?;
  let artifact = &value["Installer"];
  let host = context.installer_artifact_shape();
  if value["Schema"] != 1
    || artifact["OS"] != host["OS"]
    || artifact["Architecture"] != host["Architecture"]
    || !value["Sha256"]
      .as_str()
      .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
    || value["CompletedAtUnixMilliseconds"].as_u64().is_none()
    || !artifact["FilePath"].as_str().is_some_and(|p| Path::new(p).is_absolute())
    || !matches!(artifact["Kind"].as_i64(), Some(0..=3))
    || !artifact["DefaultArguments"]
      .as_array()
      .is_some_and(|a| a.iter().all(|v| v.as_str().is_some_and(|s| !s.contains('\0'))))
  {
    bail!(
      "Unrecognized installer record {}; preserve it and correct it explicitly.",
      path.display()
    );
  }
  Ok(Some(value))
}
pub(crate) fn record(context: &BuildContext, artifact: &Value) -> Result<()> {
  validate_installer(artifact, context)?;
  let file = Path::new(artifact["FilePath"].as_str().unwrap());
  if !ordinary(file) {
    bail!("Record an ordinary installer file, not a link: {}", file.display());
  }
  let path = record_path(context);
  if file == path {
    bail!("Installer cannot be its own successful-build record");
  }
  let _lock = state_lock(context, &format!("{}-{}.lock", context.os(), context.architecture()))?;
  // Refuse unknown state without requiring an old artifact to still exist.
  read_record(context)?;
  let mut normalized = artifact.clone();
  normalized["DefaultArguments"] = artifact
    .get("DefaultArguments")
    .filter(|v| v.is_array())
    .cloned()
    .unwrap_or_else(|| json!([]));
  let value = json!({"Schema":1,"Installer":normalized,"Sha256":digest(file)?,
    "CompletedAtUnixMilliseconds":SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64});
  let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
  serde_json::to_writer(temporary.as_file_mut(), &value)?;
  temporary.as_file_mut().sync_all()?;
  temporary.persist(path).map_err(|error| error.error)?;
  Ok(())
}
fn recorded(context: &BuildContext) -> Result<Option<Value>> {
  let Some(value) = read_record(context)? else {
    return Ok(None);
  };
  let artifact = &value["Installer"];
  validate_installer(artifact, context)
    .context("The last recorded installer is unavailable; build another installer or configure installer_file")?;
  let path = Path::new(artifact["FilePath"].as_str().unwrap());
  if !ordinary(path) || digest(path)? != value["Sha256"].as_str().unwrap() {
    bail!(
      "The recorded installer has changed since its successful build: {}",
      path.display()
    );
  }
  Ok(Some(artifact.clone()))
}
fn setting<'a>(context: &'a BuildContext, name: &str) -> Result<Option<&'a str>> {
  match context.setting(name) {
    Err(_) => Ok(None),
    Ok(Value::Null) => Ok(None),
    Ok(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value)),
    _ => bail!("Setting {name} must be a nonempty string"),
  }
}
fn artifact(context: &BuildContext, file: &Path) -> Result<Value> {
  let file = fs::canonicalize(file)?;
  let saved = recorded(context)
    .ok()
    .flatten()
    .filter(|saved| fs::canonicalize(saved["FilePath"].as_str().unwrap()).ok().as_ref() == Some(&file));
  let had_saved = saved.is_some();
  let mut value = saved.unwrap_or_else(|| context.installer_artifact_shape());
  value["FilePath"] = json!(file);
  if let Ok(arguments) = context.setting("installer.installer_args") {
    value["DefaultArguments"] = arguments.clone();
  }
  let extension = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
  let kind = setting(context, "installer.kind")?;
  value["Kind"] = match kind {
    Some("executable") => 0,
    Some("msi") => 1,
    Some("shell-script") => 2,
    Some("dotnet-assembly") => 3,
    Some(other) => bail!("Unknown installer.kind: {other}"),
    None if had_saved => value["Kind"].as_i64().unwrap(),
    None => match extension.as_str() {
      "msi" => 1,
      "sh" => 2,
      "dll" => 3,
      _ => 0,
    },
  }
  .into();
  validate_installer(&value, context)?;
  Ok(value)
}

#[derive(Clone, Debug)]
enum Part {
  Literal(String),
  Version,
  Hash,
  Dirty,
  Wildcard,
}
fn parts(component: &str) -> Result<Vec<Part>> {
  let mut result = vec![];
  let mut text = component;
  while !text.is_empty() {
    if let Some(rest) = text.strip_prefix("${") {
      let end = rest.find('}').context("Unclosed installer_file placeholder")?;
      result.push(match &rest[..end] {
        "version" => Part::Version,
        "hash" => Part::Hash,
        "dirty" => Part::Dirty,
        other => bail!("Unknown installer_file placeholder: {other}"),
      });
      text = &rest[end + 1..];
    } else if let Some(rest) = text.strip_prefix('*') {
      result.push(Part::Wildcard);
      text = rest;
    } else {
      let end = text
        .char_indices()
        .skip(1)
        .find(|(i, c)| *c == '*' || text[*i..].starts_with("${"))
        .map_or(text.len(), |(i, _)| i);
      result.push(Part::Literal(text[..end].into()));
      text = &text[end..];
    }
  }
  Ok(result)
}
fn valid_hash(text: &str) -> bool {
  (7..=64).contains(&text.len()) && text.bytes().all(|c| c.is_ascii_hexdigit())
}
fn version(text: &str) -> Option<(Vec<&str>, Option<&str>)> {
  let (text, build) = text.split_once('+').map_or((text, None), |(main, build)| (main, Some(build)));
  let identifiers = |s: &str| {
    s.split('.')
      .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
  };
  let (main, pre) = text.split_once('-').map_or((text, None), |(main, pre)| (main, Some(pre)));
  let components: Vec<_> = main.split('.').collect();
  if components
    .iter()
    .any(|c| c.is_empty() || !c.bytes().all(|b| b.is_ascii_digit()))
    || pre.is_some_and(|p| !identifiers(p))
    || build.is_some_and(|b| !identifiers(b))
  {
    return None;
  }
  Some((components, pre))
}
fn number_cmp(a: &str, b: &str) -> Ordering {
  let a = a.trim_start_matches('0');
  let b = b.trim_start_matches('0');
  a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}
fn version_cmp(a: &str, b: &str) -> Ordering {
  let (a, ap) = version(a).unwrap();
  let (b, bp) = version(b).unwrap();
  for i in 0..a.len().max(b.len()) {
    let cmp = number_cmp(a.get(i).unwrap_or(&"0"), b.get(i).unwrap_or(&"0"));
    if cmp != Ordering::Equal {
      return cmp;
    }
  }
  match (ap, bp) {
    (None, None) => Ordering::Equal,
    (None, _) => Ordering::Greater,
    (_, None) => Ordering::Less,
    (Some(a), Some(b)) => {
      for (a, b) in a.split('.').zip(b.split('.')) {
        let an = a.bytes().all(|c| c.is_ascii_digit());
        let bn = b.bytes().all(|c| c.is_ascii_digit());
        let cmp = match (an, bn) {
          (true, true) => number_cmp(a, b),
          (true, false) => Ordering::Less,
          (false, true) => Ordering::Greater,
          _ => a.cmp(b),
        };
        if cmp != Ordering::Equal {
          return cmp;
        }
      }
      a.split('.').count().cmp(&b.split('.').count())
    }
  }
}
fn matches(parts: &[Part], text: &str, captured: Option<String>, budget: &mut usize) -> Result<Option<Option<String>>> {
  *budget = budget
    .checked_sub(1)
    .context("Installer template matching exceeded its work limit; simplify installer_file")?;
  let Some((part, rest)) = parts.split_first() else {
    return Ok(text.is_empty().then_some(captured));
  };
  if let Part::Literal(literal) = part {
    let Some(head) = text.get(..literal.len()) else {
      return Ok(None);
    };
    if head == literal || cfg!(windows) && head.eq_ignore_ascii_case(literal) {
      return matches(rest, &text[literal.len()..], captured, budget);
    }
    return Ok(None);
  }
  // Captures stay in a single path component. The bounded filename length and
  // a work budget keep pathological wildcard backtracking bounded.
  // Prefer the shortest successful capture so an adjacent ${dirty} owns its
  // suffix instead of being consumed as part of a prerelease version.
  for end in text.char_indices().map(|(i, _)| i).chain(std::iter::once(text.len())) {
    let value = &text[..end];
    let valid = match part {
      Part::Version => version(value).is_some(),
      Part::Hash => valid_hash(value),
      Part::Dirty => matches!(value, "" | "-dirty"),
      Part::Wildcard => true,
      _ => unreachable!(),
    };
    if valid
      && let Some(found) = matches(
        rest,
        &text[end..],
        if matches!(part, Part::Version) {
          Some(value.to_owned())
        } else {
          captured.clone()
        },
        budget,
      )?
    {
      return Ok(Some(found));
    }
  }
  Ok(None)
}
fn select(context: &BuildContext, pattern: &str) -> Result<PathBuf> {
  if setting(context, "installer.select")?.is_some_and(|value| value != "highest-version") {
    bail!("installer.select supports highest-version");
  }
  let pattern = pattern
    .replace("${os}", context.os())
    .replace("${arch}", context.architecture())
    .replace("${exe-extension}", if cfg!(windows) { ".exe" } else { "" });
  let full = context.path(pattern);
  let mut candidates = vec![(PathBuf::new(), None::<String>)];
  let mut counts = [0usize; 4];
  let mut inspected = 0;
  for component in full.components() {
    let Component::Normal(name) = component else {
      for (path, _) in &mut candidates {
        path.push(component.as_os_str());
      }
      continue;
    };
    let name = name.to_str().context("Installer templates require Unicode paths")?;
    let tokens = parts(name)?;
    for token in &tokens {
      let index = match token {
        Part::Version => 0,
        Part::Hash => 1,
        Part::Dirty => 2,
        Part::Wildcard => 3,
        _ => continue,
      };
      counts[index] += 1;
    }
    if counts[..3].iter().any(|count| *count > 1) || counts[3] > 4 {
      bail!("installer_file permits one version/hash/dirty capture and at most four * wildcards");
    }
    let dynamic = tokens.iter().any(|p| !matches!(p, Part::Literal(_)));
    let mut next = vec![];
    for (parent, captured) in candidates {
      if !dynamic {
        next.push((parent.join(name), captured));
        continue;
      }
      if !parent.is_dir() {
        continue;
      }
      for entry in fs::read_dir(&parent)? {
        inspected += 1;
        if inspected > 20000 {
          bail!("Installer template search exceeded 20000 entries; narrow installer_file");
        }
        let entry = entry?;
        if is_link(&fs::symlink_metadata(entry.path())?) {
          continue;
        }
        let filename = entry.file_name();
        if let Some(filename) = filename.to_str()
          && let Some(captured) = matches(&tokens, filename, captured.clone(), &mut 20000)?
        {
          next.push((entry.path(), captured));
        }
      }
    }
    candidates = next;
  }
  candidates.retain(|(path, _)| ordinary(path));
  if candidates.is_empty() {
    bail!(
      "No installer matches installer.installer_file: {}. Build an installer or correct the setting.",
      full.display()
    );
  }
  candidates.sort_by(|a, b| match (&a.1, &b.1) {
    (Some(a), Some(b)) => version_cmp(b, a),
    _ => a.0.cmp(&b.0),
  });
  let best = candidates[0].1.as_deref();
  let tied: Vec<_> = candidates
    .iter()
    .filter(|(_, v)| match (best, v.as_deref()) {
      (Some(a), Some(b)) => version_cmp(a, b) == Ordering::Equal,
      _ => true,
    })
    .collect();
  if tied.len() != 1 {
    bail!(
      "Ambiguous latest installer (hashes and timestamps do not order equal versions):\n{}",
      tied.iter().map(|(p, _)| p.display().to_string()).collect::<Vec<_>>().join("\n")
    );
  }
  Ok(candidates.remove(0).0)
}
pub(crate) fn find(context: &BuildContext) -> Result<Option<Value>> {
  if let Some(pattern) = setting(context, "installer.installer_file")? {
    return artifact(context, &select(context, pattern)?).map(Some);
  }
  recorded(context)
}

fn discover(root: &Path) -> Result<Vec<PathBuf>> {
  fn visit(path: &Path, depth: usize, count: &mut usize, found: &mut Vec<PathBuf>) -> Result<()> {
    if depth > 12 {
      return Ok(());
    }
    // Retained uninstallers are not newly produced application installers.
    if path.join("installation.yaml").is_file() || path.join("installer/installation.yaml").is_file() {
      return Ok(());
    }
    for entry in fs::read_dir(path)? {
      *count += 1;
      if *count > 20000 {
        bail!("Installer discovery reached 20000 entries; configure installer_file explicitly");
      }
      let entry = entry?;
      let metadata = fs::symlink_metadata(entry.path())?;
      if is_link(&metadata) {
        continue;
      }
      let name = entry.file_name();
      let name = name.to_string_lossy().to_ascii_lowercase();
      if metadata.is_dir() {
        if name.starts_with('.')
          || matches!(
            name.as_str(),
            "bin" | "obj" | "target" | "node_modules" | "vendor" | "payload" | "sdk"
          )
        {
          continue;
        }
        visit(&entry.path(), depth + 1, count, found)?;
      } else if metadata.is_file()
        && (matches!(
          name.as_str(),
          "installer.exe" | "setup.exe" | "installer" | "setup" | "setup.sh" | "installer.sh"
        ) || name.ends_with(".msi")
          || name.ends_with("setup.exe")
          || name.ends_with("-installer.exe"))
      {
        found.push(entry.path());
        if found.len() > 50 {
          bail!("More than 50 possible installers; configure installer_file explicitly");
        }
      }
    }
    Ok(())
  }
  let mut found = vec![];
  visit(root, 0, &mut 0, &mut found)?;
  found.sort_by_key(|p| {
    (
      !p.components()
        .any(|c| matches!(c.as_os_str().to_str(), Some("artifacts" | "dist" | "installers"))),
      p.clone(),
    )
  });
  Ok(found)
}
fn suggested(context: &BuildContext, file: &Path) -> String {
  let root = fs::canonicalize(context.root()).unwrap_or_else(|_| context.root().to_owned());
  let file = fs::canonicalize(file).unwrap_or_else(|_| file.to_owned());
  let relative = file.strip_prefix(&root).unwrap_or(&file).to_string_lossy().replace('\\', "/");
  relative
    .split('/')
    .map(|component| {
      if component == format!("{}-{}", context.os(), context.architecture()) {
        return "${os}-${arch}".into();
      }
      if component == "installer.exe" || component == "installer" {
        return "installer${exe-extension}".into();
      }
      let clean = component.strip_suffix("-dirty").unwrap_or(component);
      let (base, hash) = clean
        .rsplit_once('-')
        .filter(|(_, hash)| valid_hash(hash))
        .map_or((clean, None), |(base, hash)| (base, Some(hash)));
      for (index, ch) in base.char_indices() {
        if ch.is_ascii_digit()
          && index > 0
          && base.as_bytes()[index - 1] == b'-'
          && base[index..].contains('.')
          && version(&base[index..]).is_some()
        {
          return format!(
            "{}${{version}}{}${{dirty}}",
            &base[..index],
            if hash.is_some() { "-${hash}" } else { "" }
          );
        }
      }
      component.to_owned()
    })
    .collect::<Vec<_>>()
    .join("/")
}

/// Add only a missing scalar to ordinary block mappings. Unsupported YAML forms
/// are left intact with manual guidance; never serialize the user's whole file.
fn insert_setting(source: &str, value: &str) -> Result<String> {
  let bom = source.starts_with('\u{feff}');
  let text = source.trim_start_matches('\u{feff}');
  let mut before: Value = if text.trim().is_empty() {
    json!({})
  } else {
    serde_saphyr::from_str(text)?
  };
  if !before.is_object() {
    bail!("Project configuration must be a mapping");
  }
  if lookup(&before, "settings.installer.installer_file").is_some() {
    bail!("installer_file was already configured; it was not overwritten");
  }
  let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
  let lines: Vec<_> = text.split_inclusive('\n').collect();
  let indent = |line: &str| line.len() - line.trim_start_matches(' ').len();
  let key = |line: &str, name: &str| -> bool {
    line
      .trim_start_matches(' ')
      .split_once(':')
      .is_some_and(|(k, _)| k.eq_ignore_ascii_case(name))
  };
  let block = |line: &str| {
    line
      .split_once(':')
      .is_some_and(|(_, value)| value.trim().is_empty() || value.trim().starts_with('#'))
  };
  let expand_empty = |index: usize| -> Option<String> {
    // --init emits settings: {}. Expanding an empty mapping loses no existing
    // members; retain every surrounding byte, including its trailing comment.
    let line = lines[index];
    let colon = line.find(':')?;
    let content = line[colon + 1..].trim_start();
    let tail = content.strip_prefix("{}")?.trim();
    if !tail.is_empty() && !tail.starts_with('#') {
      return None;
    }
    let offset = lines[..index].iter().map(|line| line.len()).sum::<usize>()
      + if bom { '\u{feff}'.len_utf8() } else { 0 }
      + line.len()
      - content.len();
    let mut expanded = source.to_owned();
    expanded.replace_range(offset..offset + 2, "");
    Some(expanded)
  };
  let end_of = |start: usize, level: usize| {
    (start + 1..lines.len())
      .find(|i| {
        let trimmed = lines[*i].trim();
        !trimmed.is_empty() && !trimmed.starts_with('#') && indent(lines[*i]) <= level
      })
      .unwrap_or(lines.len())
  };
  let quoted = format!("'{}'", value.replace('\'', "''"));
  let (at, addition) = if let Some(s) = lines.iter().position(|line| indent(line) == 0 && key(line, "settings")) {
    if !block(lines[s]) {
      if let Some(expanded) = expand_empty(s) {
        return insert_setting(&expanded, value);
      }
      bail!("Automatic setup supports block-style settings only; add installer_file manually using the example");
    }
    let end = end_of(s, 0);
    let unit = (s + 1..end)
      .filter(|i| !lines[*i].trim().is_empty() && !lines[*i].trim_start().starts_with('#'))
      .map(|i| indent(lines[i]))
      .min()
      .unwrap_or(2);
    if let Some(i) = (s + 1..end).find(|i| indent(lines[*i]) == unit && key(lines[*i], "installer")) {
      if !block(lines[i]) {
        if let Some(expanded) = expand_empty(i) {
          return insert_setting(&expanded, value);
        }
        bail!(
          "Automatic setup supports block-style installer settings only; add installer_file manually using the example"
        );
      }
      (
        end_of(i, unit),
        format!("{}installer_file: {quoted}{eol}", " ".repeat(unit * 2)),
      )
    } else {
      (
        end,
        format!(
          "{}installer:{eol}{}installer_file: {quoted}{eol}",
          " ".repeat(unit),
          " ".repeat(unit * 2)
        ),
      )
    }
  } else {
    if lookup(&before, "settings").is_some() {
      bail!("Cannot safely locate block-style settings; add installer_file manually");
    }
    (
      lines.len(),
      format!("settings:{eol}  installer:{eol}    installer_file: {quoted}{eol}"),
    )
  };
  let offset: usize = lines[..at].iter().map(|line| line.len()).sum();
  let separator = if offset > 0 && !text[..offset].ends_with('\n') {
    eol
  } else {
    ""
  };
  let mut result = format!(
    "{}{}{}{}{}",
    if bom { "\u{feff}" } else { "" },
    &text[..offset],
    separator,
    addition,
    &text[offset..]
  );
  if !source.is_empty() && !source.ends_with('\n') && result.ends_with(eol) {
    result.truncate(result.len() - eol.len());
  }
  // Parsing the surgical edit must produce exactly the intended mapping change.
  let settings_key = before
    .as_object()
    .unwrap()
    .keys()
    .find(|k| k.eq_ignore_ascii_case("settings"))
    .cloned()
    .unwrap_or("settings".into());
  let settings = before.as_object_mut().unwrap().entry(settings_key).or_insert_with(|| json!({}));
  if settings.is_null() {
    *settings = json!({});
  }
  let installer_key = settings
    .as_object()
    .context("settings must be a mapping")?
    .keys()
    .find(|k| k.eq_ignore_ascii_case("installer"))
    .cloned()
    .unwrap_or("installer".into());
  let installer = settings
    .as_object_mut()
    .unwrap()
    .entry(installer_key)
    .or_insert_with(|| json!({}));
  if installer.is_null() {
    *installer = json!({});
  }
  installer
    .as_object_mut()
    .context("installer settings must be a mapping")?
    .insert("installer_file".into(), json!(value));
  let after: Value = serde_saphyr::from_str(result.trim_start_matches('\u{feff}'))?;
  if after != before {
    bail!("Cannot safely insert installer_file without changing other configuration; edit it manually");
  }
  Ok(result)
}
fn configure(context: &BuildContext, value: &str, observed: Option<&[u8]>) -> Result<()> {
  // Creating the root file alongside legacy configuration would make the next
  // CLI invocation fail. Leave consolidation of user-owned configuration explicit.
  let tasks = context.data["TaskDirectory"]
    .as_str()
    .map(PathBuf::from)
    .unwrap_or_else(|| context.root().join(".tasks"));
  if observed.is_none() && tasks.join("config.yaml").try_exists()? {
    bail!(
      "Legacy task-directory config.yaml exists. Move it to .dotasks.yaml explicitly before saving installer_file."
    );
  }
  let _lock = state_lock(context, "configuration.lock")?;
  let path = fs::canonicalize(context.root())?.join(".dotasks.yaml");
  no_links(&path)?;
  let current = match fs::read(&path) {
    Ok(bytes) => Some(bytes),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
    Err(e) => return Err(e.into()),
  };
  if current.as_deref() != observed {
    bail!(".dotasks.yaml changed during setup; it was not modified. Run install again.");
  }
  let text = std::str::from_utf8(current.as_deref().unwrap_or_default())
    .context("Configuration must be UTF-8; edit installer_file manually")?;
  let updated = insert_setting(text, value)?;
  let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
  temporary.write_all(updated.as_bytes())?;
  if let Ok(metadata) = fs::metadata(&path) {
    temporary.as_file().set_permissions(metadata.permissions())?;
  }
  temporary.as_file_mut().sync_all()?;
  if fs::read(&path).ok().as_deref() != observed {
    bail!(".dotasks.yaml changed during setup; it was not modified");
  }
  temporary.persist(path).map_err(|error| error.error)?;
  Ok(())
}
fn ask(input: &mut impl BufRead, output: &mut impl Write, prompt: &str) -> Result<String> {
  write!(output, "{prompt}")?;
  output.flush()?;
  let mut text = String::new();
  if input.read_line(&mut text)? == 0 {
    bail!("Installer setup cancelled (end of input)");
  }
  Ok(text.trim().to_owned())
}
fn yes(input: &mut impl BufRead, output: &mut impl Write, prompt: &str) -> Result<bool> {
  let reply = ask(input, output, prompt)?;
  Ok(reply.eq_ignore_ascii_case("yes") || reply.eq_ignore_ascii_case("y"))
}
fn setup(
  context: &BuildContext,
  input: &mut impl BufRead,
  output: &mut impl Write,
  interactive: bool,
) -> Result<Value> {
  let observed = match fs::read(context.root().join(".dotasks.yaml")) {
    Ok(bytes) => Some(bytes),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
    Err(e) => return Err(e.into()),
  };
  writeln!(output, "installer.installer_file is not configured. Add this to .dotasks.yaml (replace YOUR-APP):\n\nsettings:\n  installer:\n    installer_file: './artifacts/installers/${{os}}-${{arch}}/YOUR-APP-${{version}}-${{hash}}${{dirty}}/installer${{exe-extension}}'\n\nNo creation task is required. SDK record_installer / RecordInstallerAsync can record successful builds.")?;
  let saved = match recorded(context) {
    Ok(saved) => saved,
    Err(e) => {
      writeln!(output, "Saved installer unavailable: {e:#}")?;
      None
    }
  };
  let candidates = if let Some(saved) = &saved {
    vec![PathBuf::from(saved["FilePath"].as_str().unwrap())]
  } else {
    discover(context.root())?
  };
  for (i, file) in candidates.iter().enumerate() {
    writeln!(output, "  {}. {}", i + 1, file.display())?;
  }
  if !interactive {
    bail!("Installer setup needs an interactive terminal. Set settings.installer.installer_file using the example, then run dotask install again.");
  }
  if candidates.is_empty() {
    if yes(input, output, "No installer found. Add an editable placeholder? [y/N] ")? {
      configure(
        context,
        "./artifacts/installers/${os}-${arch}/YOUR-APP-${version}-${hash}${dirty}/installer${exe-extension}",
        observed.as_deref(),
      )?;
    }
    bail!("Build an installer and replace YOUR-APP in installer_file, then run dotask install again.");
  }
  let index = if candidates.len() == 1 {
    if !yes(input, output, "Use this installer now? [y/N] ")? {
      bail!("Installer setup cancelled; no installer was launched");
    }
    0
  } else {
    ask(input, output, "Choose an installer number (blank cancels): ")?
      .parse::<usize>()
      .ok()
      .filter(|i| *i > 0 && *i <= candidates.len())
      .context("Installer selection cancelled or invalid")?
      - 1
  };
  let selected = &candidates[index];
  let suggestion = suggested(context, selected);
  writeln!(output, "Suggested installer_file: {suggestion}")?;
  if yes(input, output, "Save this setting in .dotasks.yaml? [y/N] ")? {
    configure(context, &suggestion, observed.as_deref())?;
  }
  artifact(context, selected)
}
pub(crate) fn install(context: &BuildContext, arguments: Option<&[String]>) -> Result<()> {
  if arguments.is_some_and(|args| args.iter().any(|arg| arg.contains('\0'))) {
    bail!("Installer arguments cannot contain NUL");
  }
  let selected = if setting(context, "installer.installer_file")?.is_some() {
    find(context)?.context("No installer configured")?
  } else {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let interactive = stdin.is_terminal() && stdout.is_terminal();
    setup(context, &mut stdin.lock(), &mut stdout.lock(), interactive)?
  };
  println!("Selected installer: {}", selected["FilePath"].as_str().unwrap());
  context.run_installer(&selected, arguments)
}

#[cfg(test)]
mod tests {
  use super::*;
  fn fixture(settings: Value) -> (tempfile::TempDir, BuildContext) {
    let root = tempfile::tempdir().unwrap();
    let context = BuildContext {
      data: json!({"RootDirectory":root.path(),"Settings":settings}),
    };
    (root, context)
  }
  fn package(context: &BuildContext, name: &str) -> PathBuf {
    let path = context
      .root()
      .join("artifacts/installers")
      .join(format!("{}-{}", context.os(), context.architecture()))
      .join(name)
      .join(if cfg!(windows) {
        "installer.exe"
      } else {
        "installer"
      });
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"installer bytes").unwrap();
    path
  }
  fn pattern() -> &'static str {
    "./artifacts/installers/${os}-${arch}/dotask-${version}-${hash}${dirty}/installer${exe-extension}"
  }
  #[test]
  fn template_selects_numeric_highest_version_and_supports_dirty_and_prereleases() {
    let (_root, context) = fixture(json!({"installer":{"installer_file":pattern()}}));
    package(&context, "dotask-0.1.26285.999-1111111");
    package(&context, "dotask-0.1.26285.1741-2222222");
    package(&context, "dotask-0.1.26286.1000-rc.2-3333333");
    let expected = package(&context, "dotask-0.1.26286.1000-4444444-dirty");
    let selected = context.find_installer().unwrap().unwrap();
    assert_eq!(
      Path::new(selected["FilePath"].as_str().unwrap()),
      fs::canonicalize(expected).unwrap()
    );
    assert_eq!(version_cmp("1.10.0", "1.9.99"), Ordering::Greater);
    assert_eq!(version_cmp("1.2-rc.10", "1.2-rc.2"), Ordering::Greater);
    assert_eq!(version_cmp("1.2.0+build1", "1.2+build2"), Ordering::Equal);
    assert!(version("1.2+").is_none());
    assert!(version("1.2-rc..1").is_none());
    let tokens = parts("app-${version}${dirty}").unwrap();
    assert_eq!(
      matches(&tokens, "app-1.2-dirty", None, &mut 20000).unwrap(),
      Some(Some("1.2".into()))
    );
  }
  #[test]
  fn ambiguous_hashes_and_invalid_templates_never_fall_back_to_a_record() {
    let (_root, mut context) = fixture(json!({"installer":{"installer_file":pattern()}}));
    let first = package(&context, "dotask-1.0.0-1111111");
    context.record_installer(&context.installer_artifact(&first).unwrap()).unwrap();
    package(&context, "dotask-1.0.0-2222222");
    assert!(context.find_installer().unwrap_err().to_string().contains("Ambiguous"));
    context.data["Settings"]["installer"]["installer_file"] = json!("artifacts/${unknown}/installer.exe");
    assert!(context.find_installer().unwrap_err().to_string().contains("Unknown"));
    context.data["Settings"]["installer"]["installer_file"] = json!("absent/installer.exe");
    assert!(context
      .find_installer()
      .unwrap_err()
      .to_string()
      .contains("No installer matches"));
  }
  #[test]
  fn record_is_read_only_to_load_and_failed_or_unknown_updates_preserve_state() {
    let (_root, context) = fixture(json!({}));
    assert!(context.find_installer().unwrap().is_none());
    assert!(!context.root().join("artifacts").exists());
    let file = package(&context, "dotask-1.0.0-1111111");
    let artifact = context.installer_artifact(&file).unwrap();
    context.record_installer(&artifact).unwrap();
    let before = fs::read(record_path(&context)).unwrap();
    assert!(context.find_installer().unwrap().is_some());
    assert_eq!(before, fs::read(record_path(&context)).unwrap());
    let mut invalid = artifact.clone();
    invalid["FilePath"] = json!(file.with_extension("missing"));
    assert!(context.record_installer(&invalid).is_err());
    assert_eq!(before, fs::read(record_path(&context)).unwrap());
    fs::write(&file, b"failed build overwrote output").unwrap();
    assert!(context.find_installer().unwrap_err().to_string().contains("changed"));
    fs::write(record_path(&context), b"{\"Schema\":42}").unwrap();
    assert!(context.record_installer(&artifact).is_err());
    assert_eq!(fs::read(record_path(&context)).unwrap(), b"{\"Schema\":42}");
  }
  #[test]
  fn source_preserving_setup_handles_comments_bom_crlf_and_missing_final_newline() {
    let source = "\u{feff}version: 1\r\n# keep header\r\nsettings:\r\n  installer: # keep comment\r\n    output: artifacts # trailing\r\nname: 'Example'";
    let updated = insert_setting(source, "a 'quoted'/setup.exe").unwrap();
    assert_eq!(updated, "\u{feff}version: 1\r\n# keep header\r\nsettings:\r\n  installer: # keep comment\r\n    output: artifacts # trailing\r\n    installer_file: 'a ''quoted''/setup.exe'\r\nname: 'Example'");
    assert!(insert_setting("version: 1\nsettings: {installer: {output: a}}\n", "setup.exe").is_err());
    assert!(insert_setting("settings:\n  installer:\n    installer_file: existing\n", "other").is_err());
    assert!(insert_setting("version: 1\n", "setup.exe")
      .unwrap()
      .ends_with("    installer_file: 'setup.exe'\n"));
    assert!(insert_setting("settings:\n  installer:\n", "setup.exe").is_ok());
    assert!(insert_setting("settings:\n", "setup.exe").is_ok());
    let initialized = insert_setting("\u{feff}version: 1\r\nsettings: {} # retain\r\n", "setup.exe").unwrap();
    assert!(initialized.starts_with("\u{feff}version: 1\r\nsettings:  # retain\r\n"));
    assert!(insert_setting("settings:\n  installer: {} # retain\n", "setup.exe")
      .unwrap()
      .contains("installer:  # retain"));
  }
  #[test]
  fn interactive_setup_can_save_a_template_or_decline_without_editing() {
    let (_root, context) = fixture(json!({}));
    let original = b"# user config\nversion: 1\nsettings:\n  unrelated: true\n";
    fs::write(context.root().join(".dotasks.yaml"), original).unwrap();
    let file = package(&context, "dotask-1.2.3-abcdef1");
    let saved = context.installer_artifact(&file).unwrap();
    context.record_installer(&saved).unwrap();
    let mut output = vec![];
    assert!(setup(&context, &mut &b""[..], &mut output, false).is_err());
    assert_eq!(fs::read(context.root().join(".dotasks.yaml")).unwrap(), original);
    setup(&context, &mut &b"y\nn\n"[..], &mut output, true).unwrap();
    assert_eq!(fs::read(context.root().join(".dotasks.yaml")).unwrap(), original);
    setup(&context, &mut &b"y\ny\n"[..], &mut output, true).unwrap();
    let updated = fs::read_to_string(context.root().join(".dotasks.yaml")).unwrap();
    assert!(updated.contains("dotask-${version}-${hash}${dirty}"));
    assert!(updated.starts_with("# user config\n"));
    assert!(updated.contains("  unrelated: true\n"));
    let external = tempfile::tempdir().unwrap();
    let outside = external.path().join(if cfg!(windows) { "setup.exe" } else { "setup" });
    fs::write(&outside, b"installer").unwrap();
    assert_eq!(
      fs::canonicalize(select(&context, &suggested(&context, &outside)).unwrap()).unwrap(),
      fs::canonicalize(outside).unwrap()
    );
  }
  #[test]
  fn placeholder_setup_and_concurrent_changes_preserve_user_owned_files() {
    let (_root, context) = fixture(json!({}));
    let mut output = vec![];
    assert!(setup(&context, &mut &b"n\n"[..], &mut output, true).is_err());
    assert!(!context.root().join(".dotasks.yaml").exists());
    assert!(setup(&context, &mut &b"y\n"[..], &mut output, true).is_err());
    assert!(fs::read_to_string(context.root().join(".dotasks.yaml"))
      .unwrap()
      .contains("YOUR-APP"));
    let before = fs::read(context.root().join(".dotasks.yaml")).unwrap();
    fs::write(context.root().join(".dotasks.yaml"), b"settings: {user: changed}\n").unwrap();
    assert!(configure(&context, "setup.exe", Some(&before)).is_err());
    assert_eq!(
      fs::read(context.root().join(".dotasks.yaml")).unwrap(),
      b"settings: {user: changed}\n"
    );
    fs::remove_file(context.root().join(".dotasks.yaml")).unwrap();
    fs::create_dir(context.root().join(".tasks")).unwrap();
    fs::write(context.root().join(".tasks/config.yaml"), b"version: 1\n").unwrap();
    assert!(configure(&context, "setup.exe", None)
      .unwrap_err()
      .to_string()
      .contains("Legacy"));
    assert!(!context.root().join(".dotasks.yaml").exists());
  }
  #[test]
  fn discovery_excludes_payloads_dependencies_and_retained_uninstallers() {
    let (_root, context) = fixture(json!({}));
    let wanted = package(&context, "dotask-1.0-abcdef1");
    for dir in [
      "node_modules/dependency",
      "artifacts/payload",
      "artifacts/installed/installer",
    ] {
      let dir = context.root().join(dir);
      fs::create_dir_all(&dir).unwrap();
      fs::write(dir.join("setup.exe"), b"ignored").unwrap();
      if dir.ends_with("installer") {
        fs::write(dir.join("installation.yaml"), b"receipt").unwrap();
      }
    }
    assert_eq!(discover(context.root()).unwrap(), vec![wanted]);
  }
  #[test]
  fn matching_work_is_bounded_and_windows_arguments_keep_quotes_and_slashes() {
    assert!(matches(&parts("*a*a*a*a*z").unwrap(), &"a".repeat(120), None, &mut 20000).is_err());
    assert_eq!(windows_argument(""), "\"\"");
    assert_eq!(windows_argument("a\\\"b"), "\"a\\\\\\\"b\"");
    assert_eq!(windows_argument("trailing\\"), "\"trailing\\\\\"");
  }
}

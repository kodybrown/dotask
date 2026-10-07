use crate::{
  config, files,
  model::{OwnedFile, Package, Shortcut},
};
use anyhow::{ensure, Context, Result};
use std::{
  collections::{BTreeMap, HashSet},
  fs,
  path::{Path, PathBuf},
};

pub fn record(path: &Path) -> Result<OwnedFile> {
  files::safe_path(path.parent().context("Launcher has no parent")?)?;
  let meta = fs::symlink_metadata(path)?;
  if meta.file_type().is_symlink() {
    Ok(OwnedFile {
      path: path.to_string_lossy().into(),
      hash: None,
      link: Some(fs::read_link(path)?.to_string_lossy().into()),
    })
  } else {
    files::safe_path(path)?;
    Ok(OwnedFile {
      path: path.to_string_lossy().into(),
      hash: Some(files::file_hash(path)?),
      link: None,
    })
  }
}
pub fn verify(file: &OwnedFile) -> Result<()> {
  let path = Path::new(&file.path);
  files::safe_path(path.parent().context("Invalid launcher path")?)?;
  if fs::symlink_metadata(path).is_ok() {
    ensure!(record(path)? == *file, "Owned launcher changed: {}", path.display());
  }
  Ok(())
}
pub struct Prepared {
  pub destination: PathBuf,
  pub source: PathBuf,
  pub shortcut_directory: Option<PathBuf>,
}
pub fn prepare(
  package: &Package,
  root: &Path,
  build: &Path,
  values: &BTreeMap<String, serde_json::Value>,
  stage: &Path,
) -> Result<Vec<Prepared>> {
  let mut result = Vec::new();
  let mut destinations = HashSet::new();
  let mut add = |source: PathBuf, destination: PathBuf, shortcut_directory: Option<PathBuf>| -> Result<()> {
    ensure!(
      destinations.insert(destination.to_string_lossy().to_lowercase()),
      "Duplicate launcher: {}",
      destination.display()
    );
    files::safe_path(destination.parent().context("Missing parent")?)?;
    result.push(Prepared {
      source,
      destination,
      shortcut_directory,
    });
    Ok(())
  };
  for (index, command) in package.runtime.commands.iter().enumerate() {
    let target = build.join(&command.executable);
    let mut directories = vec![root.to_path_buf()];
    if config::enabled(values, "additional-command") {
      directories.push(config::path(values, "bin-dir")?);
    }
    for (location, directory) in directories.into_iter().enumerate() {
      let source = stage.join(format!("command-{index}-{location}"));
      #[cfg(windows)]
      {
        let bytes: &[u8] = if cfg!(target_arch = "aarch64") {
          include_bytes!("../../Dotask.Shim/assets/win-arm64.exe")
        } else {
          include_bytes!("../../Dotask.Shim/assets/win-x64.exe")
        };
        fs::write(&source, bytes)?;
        add(source.clone(), directory.join(format!("{}.exe", command.name)), None)?;
        let sidecar = source.with_extension("shim");
        ensure!(!target.to_string_lossy().contains(['"', '\n', '\r']), "Invalid shim target");
        fs::write(&sidecar, format!("path = \"{}\"\n", target.display()))?;
        add(sidecar, directory.join(format!("{}.shim", command.name)), None)?;
      }
      #[cfg(unix)]
      {
        std::os::unix::fs::symlink(&target, &source)?;
        add(source, directory.join(&command.name), None)?;
      }
    }
  }
  for (index, shortcut) in package.runtime.shortcuts.iter().enumerate() {
    let shortcut = Shortcut {
      name: config::expand(&shortcut.name, values)?,
      ..shortcut.clone()
    };
    files::shortcut_name(&shortcut.name)?;
    let mut locations = Vec::new();
    if shortcut.local && config::enabled(values, "local-shortcuts") {
      locations.push((root.to_path_buf(), false));
    }
    if shortcut.desktop && config::enabled(values, "desktop-shortcuts") {
      locations.push((config::path(values, "desktop-dir")?, false));
    }
    if shortcut.start_menu && config::enabled(values, "start-menu-shortcuts") {
      ensure!(
        !cfg!(target_os = "macos"),
        "macOS has no Start menu; disable start-menu-shortcuts"
      );
      let mut directory = config::path(values, "start-menu-dir")?;
      let nested = cfg!(windows) && config::enabled(values, "start-menu-nested");
      if nested {
        directory.push(&shortcut.name);
      }
      locations.push((directory, nested));
    }
    for (n, (directory, nested)) in locations.into_iter().enumerate() {
      let extension = if cfg!(windows) {
        "lnk"
      } else if cfg!(target_os = "macos") {
        if shortcut.terminal {
          "command"
        } else {
          "alias"
        }
      } else {
        "desktop"
      };
      let source = stage.join(format!("shortcut-{index}-{n}.{extension}"));
      shortcut_file(&source, build, &shortcut)?;
      let owned_directory = nested.then(|| directory.clone());
      add(
        source,
        directory.join(format!("{}.{extension}", shortcut.name)),
        owned_directory,
      )?;
    }
  }
  Ok(result)
}

#[cfg(windows)]
fn shortcut_file(output: &Path, build: &Path, shortcut: &Shortcut) -> Result<()> {
  use windows::{
    core::{Interface, PCWSTR},
    Win32::{
      System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
      },
      UI::Shell::{IShellLinkW, ShellLink},
    },
  };
  fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
  }
  // COM creates real shell shortcuts; no PowerShell, shell interpretation, or
  // quoting of user input into executable script text is involved.
  unsafe {
    CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
    let result = (|| -> Result<()> {
      let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
      link.SetPath(PCWSTR(wide(&build.join(&shortcut.executable).to_string_lossy()).as_ptr()))?;
      let args = shortcut
        .arguments
        .iter()
        .map(|v| quote_windows(v))
        .collect::<Vec<_>>()
        .join(" ");
      link.SetArguments(PCWSTR(wide(&args).as_ptr()))?;
      link.SetWorkingDirectory(PCWSTR(
        wide(
          &build
            .join(shortcut.working_directory.as_deref().unwrap_or(""))
            .to_string_lossy(),
        )
        .as_ptr(),
      ))?;
      if let Some(icon) = &shortcut.icon {
        link.SetIconLocation(PCWSTR(wide(&build.join(icon).to_string_lossy()).as_ptr()), 0)?;
      }
      let persist: IPersistFile = link.cast()?;
      persist.Save(PCWSTR(wide(&output.to_string_lossy()).as_ptr()), true)?;
      Ok(())
    })();
    CoUninitialize();
    result
  }
}
#[cfg(windows)]
fn quote_windows(value: &str) -> String {
  let mut result = String::from("\"");
  let mut slashes = 0;
  for c in value.chars() {
    if c == '\\' {
      slashes += 1;
      continue;
    }
    result.push_str(&"\\".repeat(if c == '"' { slashes * 2 + 1 } else { slashes }));
    slashes = 0;
    result.push(c);
  }
  result.push_str(&"\\".repeat(slashes * 2));
  result.push('"');
  result
}
#[cfg(target_os = "linux")]
fn shortcut_file(output: &Path, build: &Path, shortcut: &Shortcut) -> Result<()> {
  use std::os::unix::fs::PermissionsExt;
  fn field(value: &str) -> String {
    value
      .replace('\\', "\\\\")
      .replace('\n', "\\n")
      .replace('\r', "\\r")
      .replace('\t', "\\t")
  }
  fn token(value: &str) -> String {
    format!(
      "\"{}\"",
      field(value)
        .replace('%', "%%")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$")
    )
  }
  let target = build.join(&shortcut.executable);
  let args = shortcut.arguments.iter().map(|v| token(v)).collect::<Vec<_>>().join(" ");
  let mut content = format!(
    "[Desktop Entry]\nType=Application\nName={}\nExec={} {}\nTerminal={}\nPath={}\n",
    field(&shortcut.name),
    token(&target.to_string_lossy()),
    args,
    shortcut.terminal,
    field(
      &build
        .join(shortcut.working_directory.as_deref().unwrap_or(""))
        .to_string_lossy()
    )
  );
  if let Some(icon) = &shortcut.icon {
    content.push_str(&format!("Icon={}\n", field(&build.join(icon).to_string_lossy())));
  }
  fs::write(output, content)?;
  fs::set_permissions(output, fs::Permissions::from_mode(0o755))?;
  Ok(())
}
#[cfg(target_os = "macos")]
fn shortcut_file(output: &Path, build: &Path, shortcut: &Shortcut) -> Result<()> {
  use std::os::unix::fs::PermissionsExt;
  fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
  }
  let target = build.join(&shortcut.executable);
  if shortcut.terminal {
    let args = shortcut.arguments.iter().map(|v| quote(v)).collect::<Vec<_>>().join(" ");
    fs::write(
      output,
      format!(
        "#!/bin/sh\ncd {} || exit 1\nexec {} {}\n",
        quote(
          &build
            .join(shortcut.working_directory.as_deref().unwrap_or(""))
            .to_string_lossy()
        ),
        quote(&target.to_string_lossy()),
        args
      ),
    )?;
    fs::set_permissions(output, fs::Permissions::from_mode(0o755))?;
  } else {
    ensure!(
      shortcut.arguments.is_empty() && shortcut.working_directory.is_none() && shortcut.icon.is_none(),
      "macOS aliases use the app bundle's icon and launch defaults; custom arguments require a terminal shortcut"
    );
    let result = std::process::Command::new("/usr/bin/osascript").args(["-e", "on run argv\ntell application \"Finder\" to make new alias file at POSIX file (item 2 of argv) to POSIX file (item 1 of argv) with properties {name:item 3 of argv}\nend run", &target.to_string_lossy(), &output.parent().unwrap().to_string_lossy(), &output.file_name().unwrap().to_string_lossy()]).output()?;
    ensure!(
      result.status.success(),
      "Cannot create macOS alias: {}",
      String::from_utf8_lossy(&result.stderr)
    );
  }
  Ok(())
}

pub fn apply(prepared: &[Prepared], previous: &[OwnedFile]) -> Result<Vec<OwnedFile>> {
  for old in previous {
    verify(old)?;
  }
  for item in prepared {
    if fs::symlink_metadata(&item.destination).is_ok() {
      ensure!(
        previous.iter().any(|p| Path::new(&p.path) == item.destination),
        "Unowned launcher exists: {}",
        item.destination.display()
      );
    }
  }
  let mut result = Vec::new();
  for item in prepared {
    let mut desired =
      record(&item.source).with_context(|| format!("Read staged launcher {}", item.source.display()))?;
    desired.path = item.destination.to_string_lossy().into();
    if fs::symlink_metadata(&item.destination).is_ok()
      && record(&item.destination).with_context(|| format!("Read existing launcher {}", item.destination.display()))?
        == desired
    {
      // The current dotask command may be waiting in this very shim while
      // its installer runs. Identical launchers need no replacement; only
      // their sidecars change when a new immutable build is activated.
      result.push(desired);
      continue;
    }
    fs::create_dir_all(item.destination.parent().unwrap())?;
    if fs::symlink_metadata(&item.source)?.file_type().is_symlink() {
      #[cfg(unix)]
      {
        let tmp = tempfile::tempdir_in(item.destination.parent().unwrap())?;
        let staged = tmp.path().join("link");
        std::os::unix::fs::symlink(fs::read_link(&item.source)?, &staged)?;
        fs::rename(staged, &item.destination)?;
      }
    } else {
      files::write_atomic(&item.destination, &fs::read(&item.source)?)?;
      fs::set_permissions(&item.destination, fs::metadata(&item.source)?.permissions())?;
    }
    result.push(record(&item.destination)?);
  }
  for old in previous {
    if !result.iter().any(|r| r.path == old.path) && fs::symlink_metadata(&old.path).is_ok() {
      fs::remove_file(&old.path)?;
    }
  }
  Ok(result)
}

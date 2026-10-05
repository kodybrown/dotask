use anyhow::Result;
use std::path::Path;

#[cfg(not(windows))]
pub fn contains(_: &Path) -> Result<bool> {
  Ok(false)
}
#[cfg(not(windows))]
pub fn add(_: &Path) -> Result<bool> {
  anyhow::bail!("Automatic user PATH changes are supported only on Windows")
}

#[cfg(windows)]
mod windows_path {
  use super::*;
  use crate::files;
  use anyhow::{ensure, Context};
  use windows::{
    core::{w, PCWSTR},
    Win32::{
      Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, LPARAM, WPARAM},
      System::{
        Environment::ExpandEnvironmentStringsW,
        Registry::{
          RegCloseKey, RegCreateKeyExW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
          HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
        },
      },
      UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE},
    },
  };

  struct Registry(HKEY);
  impl Drop for Registry {
    fn drop(&mut self) {
      unsafe {
        let _ = RegCloseKey(self.0);
      }
    }
  }
  #[derive(Default)]
  struct PathValue {
    text: String,
    kind: REG_VALUE_TYPE,
  }
  impl Registry {
    fn open(base: HKEY, subkey: PCWSTR, writable: bool) -> Result<Option<Self>> {
      let mut key = HKEY::default();
      let status = unsafe {
        if writable {
          RegCreateKeyExW(
            base,
            subkey,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_READ | KEY_SET_VALUE,
            None,
            &mut key,
            None,
          )
        } else {
          RegOpenKeyExW(base, subkey, None, KEY_READ, &mut key)
        }
      };
      if status == ERROR_FILE_NOT_FOUND && !writable {
        return Ok(None);
      }
      status.ok().context("Open persistent PATH registry key")?;
      Ok(Some(Self(key)))
    }
    fn read(&self) -> Result<PathValue> {
      // Another process may grow PATH between the length query and read.
      // Retry that race rather than truncating or replacing somebody's entries.
      for _ in 0..3 {
        let mut kind = REG_VALUE_TYPE::default();
        let mut size = 0;
        let status = unsafe { RegQueryValueExW(self.0, w!("Path"), None, Some(&mut kind), None, Some(&mut size)) };
        if status == ERROR_FILE_NOT_FOUND {
          return Ok(PathValue {
            text: String::new(),
            kind: REG_EXPAND_SZ,
          });
        }
        status.ok().context("Read persistent PATH length")?;
        ensure!(kind == REG_SZ || kind == REG_EXPAND_SZ, "Persistent PATH is not a string");
        ensure!(size % 2 == 0, "Persistent PATH has invalid UTF-16 data");
        let mut buffer = vec![0u16; (size / 2) as usize];
        let status = unsafe {
          RegQueryValueExW(
            self.0,
            w!("Path"),
            None,
            Some(&mut kind),
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
          )
        };
        if status == ERROR_MORE_DATA {
          continue;
        }
        status.ok().context("Read persistent PATH")?;
        ensure!(kind == REG_SZ || kind == REG_EXPAND_SZ, "Persistent PATH changed type");
        ensure!(size % 2 == 0, "Persistent PATH has invalid UTF-16 data");
        buffer.truncate((size / 2) as usize);
        if buffer.last() == Some(&0) {
          buffer.pop();
        }
        let text = String::from_utf16(&buffer).context("Persistent PATH has invalid UTF-16")?;
        ensure!(!text.contains('\0'), "Persistent PATH contains an embedded NUL");
        return Ok(PathValue { text, kind });
      }
      anyhow::bail!("Persistent PATH is changing; retry installation")
    }
    fn append(&self, directory: &Path) -> Result<bool> {
      let mut value = self.read()?;
      if matches(&value, directory)? {
        return Ok(false);
      }
      let directory = directory.to_string_lossy();
      ensure!(
        !directory.contains([';', '\0', '%']),
        "Command directory cannot be represented safely in PATH"
      );
      if !value.text.is_empty() && !value.text.ends_with(';') {
        value.text.push(';');
      }
      value.text.push_str(&directory);
      let data: Vec<u8> = value.text.encode_utf16().chain(Some(0)).flat_map(u16::to_le_bytes).collect();
      ensure!(data.len() / 2 <= 32767, "User PATH is too long");
      unsafe {
        RegSetValueExW(self.0, w!("Path"), None, value.kind, Some(&data))
          .ok()
          .context("Update user PATH")?;
      }
      Ok(true)
    }
  }
  fn expand(value: &str) -> Result<String> {
    let source: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
    let size = unsafe { ExpandEnvironmentStringsW(PCWSTR(source.as_ptr()), None) };
    ensure!(size != 0, "Cannot expand a PATH entry");
    let mut buffer = vec![0; size as usize];
    let written = unsafe { ExpandEnvironmentStringsW(PCWSTR(source.as_ptr()), Some(&mut buffer)) };
    ensure!(written != 0 && written <= size, "Environment changed while reading PATH");
    String::from_utf16(&buffer[..written as usize - 1]).context("Invalid expanded PATH entry")
  }
  fn matches(value: &PathValue, directory: &Path) -> Result<bool> {
    let directory = files::absolute(directory, &std::env::current_dir()?)?;
    for entry in value.text.split(';').filter(|entry| !entry.trim().is_empty()) {
      let expanded = if value.kind == REG_EXPAND_SZ {
        expand(entry.trim())?
      } else {
        entry.trim().into()
      };
      let entry = Path::new(expanded.trim_matches('"'));
      if entry.is_absolute()
        && let Ok(path) = files::absolute(entry, &std::env::current_dir()?)
        && path.to_string_lossy().eq_ignore_ascii_case(&directory.to_string_lossy())
      {
        return Ok(true);
      }
    }
    Ok(false)
  }
  pub fn contains(directory: &Path) -> Result<bool> {
    // Consult persisted user and system PATH, not a possibly stale terminal's
    // process environment. Neither registry value is printed or rewritten here.
    for (base, key) in [
      (HKEY_CURRENT_USER, w!("Environment")),
      (
        HKEY_LOCAL_MACHINE,
        w!("SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment"),
      ),
    ] {
      if let Some(key) = Registry::open(base, key, false)?
        && matches(&key.read()?, directory)?
      {
        return Ok(true);
      }
    }
    Ok(false)
  }
  pub fn add(directory: &Path) -> Result<bool> {
    if contains(directory)? {
      return Ok(false);
    }
    let key = Registry::open(HKEY_CURRENT_USER, w!("Environment"), true)?.context("User environment is unavailable")?;
    let added = key.append(directory)?;
    if added {
      // Existing processes keep their environment. Notify Explorer so newly
      // launched applications can pick up the changed user PATH.
      unsafe {
        let _ = SendMessageTimeoutW(
          HWND_BROADCAST,
          WM_SETTINGCHANGE,
          WPARAM(0),
          LPARAM(w!("Environment").as_ptr() as isize),
          SMTO_ABORTIFHUNG,
          1000,
          None,
        );
      }
    }
    Ok(added)
  }

  #[cfg(test)]
  mod tests {
    use super::*;
    #[test]
    fn registry_append_preserves_type_entries_and_is_idempotent() {
      use windows::Win32::System::Registry::RegDeleteTreeW;
      let fixture = tempfile::tempdir().unwrap();
      let name = format!(
        "Software\\dotask-installer-tests\\{}",
        fixture.path().file_name().unwrap().to_string_lossy()
      );
      let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
      // This test exercises the real registry API in its own disposable key.
      // It never opens HKCU\Environment for writing or broadcasts a change.
      let key = Registry::open(HKEY_CURRENT_USER, PCWSTR(wide.as_ptr()), true).unwrap().unwrap();
      for kind in [REG_SZ, REG_EXPAND_SZ] {
        let original = "%USERPROFILE%\\existing;C:\\tools;";
        let data: Vec<u8> = original.encode_utf16().chain(Some(0)).flat_map(u16::to_le_bytes).collect();
        unsafe {
          RegSetValueExW(key.0, w!("Path"), None, kind, Some(&data)).ok().unwrap();
        }
        assert!(key.append(fixture.path()).unwrap());
        let value = key.read().unwrap();
        assert_eq!(value.kind, kind);
        assert_eq!(value.text, format!("{original}{}", fixture.path().display()));
        assert!(!key.append(fixture.path()).unwrap());
        assert_eq!(key.read().unwrap().text, value.text);
      }
      drop(key);
      unsafe {
        RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(wide.as_ptr())).ok().unwrap();
      }
    }
    #[test]
    fn path_matching_handles_expansion_quotes_case_and_trailing_separator() {
      let home = std::env::var("USERPROFILE").unwrap();
      let value = PathValue {
        text: "\"%USERPROFILE%\\\";C:\\unrelated".into(),
        kind: REG_EXPAND_SZ,
      };
      assert!(matches(&value, Path::new(&home.to_uppercase())).unwrap());
      assert!(!matches(&value, &Path::new(&home).join("not-a-path-entry")).unwrap());
    }
  }
}
#[cfg(windows)]
pub use windows_path::{add, contains};

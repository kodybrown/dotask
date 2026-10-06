use crate::model::{Application, Input};
use anyhow::{bail, ensure, Result};
use std::{
  io::{BufRead, IsTerminal, Write},
  sync::atomic::{AtomicBool, Ordering},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

static INTERACTIVE: AtomicBool = AtomicBool::new(false);
static SUMMARY_STARTED: AtomicBool = AtomicBool::new(false);
static UNINSTALL: AtomicBool = AtomicBool::new(false);

pub fn begin(application: &Application, uninstall: bool) -> Result<()> {
  INTERACTIVE.store(true, Ordering::SeqCst);
  UNINSTALL.store(uninstall, Ordering::SeqCst);
  write_banner(&mut std::io::stdout(), application, columns())
}
pub fn settings() -> Result<()> {
  if INTERACTIVE.load(Ordering::SeqCst) {
    let title = if UNINSTALL.load(Ordering::SeqCst) {
      "Uninstallation Settings"
    } else {
      "Installation Settings"
    };
    write_section(&mut std::io::stdout(), title, columns(), true)?;
  }
  Ok(())
}
pub fn summary() -> Result<()> {
  if INTERACTIVE.load(Ordering::SeqCst) && !SUMMARY_STARTED.swap(true, Ordering::SeqCst) {
    let title = if UNINSTALL.load(Ordering::SeqCst) {
      "Uninstallation Summary"
    } else {
      "Installation Summary"
    };
    write_section(&mut std::io::stdout(), title, columns(), true)?;
  }
  Ok(())
}
pub fn message(text: &str) {
  let padding = if INTERACTIVE.load(Ordering::SeqCst) {
    "  "
  } else {
    ""
  };
  for line in text.lines() {
    println!("{padding}{line}");
  }
}
pub fn diagnostic(text: &str) {
  let padding = if INTERACTIVE.load(Ordering::SeqCst) {
    "  "
  } else {
    ""
  };
  for line in text.lines() {
    eprintln!("{padding}{line}");
  }
}
pub fn canceled() {
  let _ = summary();
  diagnostic("Canceled");
}
pub fn error(error: &anyhow::Error) {
  let _ = summary();
  if INTERACTIVE.load(Ordering::SeqCst) {
    diagnostic("Installer error:");
    diagnostic(&format!("{error:#}"));
  } else {
    diagnostic(&format!("Installer error: {error:#}"));
  }
}
fn write_banner(writer: &mut impl Write, application: &Application, columns: usize) -> Result<()> {
  write_section(writer, &application.name, columns, false)?;
  for text in [&application.copyright, &application.description] {
    if !text.trim().is_empty() {
      write_paragraph(writer, text, columns.saturating_sub(3).max(1))?;
    }
  }
  writeln!(writer)?;
  Ok(())
}
fn write_paragraph(writer: &mut impl Write, text: &str, width: usize) -> Result<()> {
  for paragraph in text.lines() {
    let mut line = String::new();
    for word in paragraph.split_whitespace() {
      if !line.is_empty() && line.width() + 1 + word.width() > width {
        writeln!(writer, "  {line}")?;
        line.clear();
      }
      if !line.is_empty() {
        line.push(' ');
      }
      line.push_str(word);
    }
    writeln!(writer, "  {line}")?;
  }
  Ok(())
}
fn write_section(writer: &mut impl Write, title: &str, columns: usize, leading_break: bool) -> Result<()> {
  if leading_break {
    writeln!(writer)?;
  }
  // Leave the final visible column unused to avoid terminal auto-wrap. Unicode
  // display widths matter for application names; byte/character counts differ.
  let width = columns.saturating_sub(1);
  let prefix = format!("─── {title} ");
  let mut line = String::new();
  let mut used = 0;
  for c in prefix.chars().map(|c| if c.is_control() { ' ' } else { c }) {
    let size = c.width().unwrap_or(0);
    if used + size > width {
      break;
    }
    line.push(c);
    used += size;
  }
  line.push_str(&"─".repeat(width - used));
  writeln!(writer, "{line}")?;
  if leading_break {
    writeln!(writer)?;
  }
  Ok(())
}
fn columns() -> usize {
  terminal_columns().filter(|&width| width > 0).unwrap_or(80)
}
fn terminal_columns() -> Option<usize> {
  if !std::io::stdout().is_terminal() {
    return None;
  }
  #[cfg(windows)]
  {
    use windows::Win32::System::Console::{
      GetConsoleScreenBufferInfo, GetStdHandle, CONSOLE_SCREEN_BUFFER_INFO, STD_OUTPUT_HANDLE,
    };
    // Query the visible window each time, including after a console resize.
    // Never resize, attach, or allocate a console on the user's behalf.
    unsafe {
      let handle = GetStdHandle(STD_OUTPUT_HANDLE).ok()?;
      let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
      GetConsoleScreenBufferInfo(handle, &mut info).ok()?;
      Some((info.srWindow.Right - info.srWindow.Left + 1) as usize)
    }
  }
  #[cfg(unix)]
  {
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) } == 0 {
      Some(size.ws_col as usize)
    } else {
      None
    }
  }
  #[cfg(not(any(windows, unix)))]
  {
    None
  }
}

#[derive(Debug)]
pub struct Canceled;
impl std::fmt::Display for Canceled {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("Canceled")
  }
}
impl std::error::Error for Canceled {}

pub fn ask(input: &Input, default: &str, reader: &mut impl BufRead, writer: &mut impl Write) -> Result<String> {
  let Some(prompt) = &input.prompt else {
    return Ok(default.into());
  };
  loop {
    let hint = if input.kind == "boolean" {
      if default == "true" {
        "Y/n"
      } else {
        "y/N"
      }
    } else {
      default
    };
    write!(writer, "  {prompt} [{hint}]: ")?;
    writer.flush()?;
    let mut answer = String::new();
    if reader.read_line(&mut answer)? == 0 {
      bail!(Canceled);
    }
    let answer = answer.trim();
    if answer.is_empty() {
      return Ok(default.into());
    }
    if input.kind == "boolean" {
      match answer.to_ascii_lowercase().as_str() {
        "y" | "yes" | "true" => return Ok("true".into()),
        "n" | "no" | "false" => return Ok("false".into()),
        _ => writeln!(writer, "  Please answer yes or no.")?,
      }
    } else {
      return Ok(answer.into());
    }
  }
}

pub fn confirm(input: &Input, default: bool) -> Result<()> {
  let answer = ask(
    input,
    if default { "true" } else { "false" },
    &mut std::io::stdin().lock(),
    &mut std::io::stdout(),
  )?;
  ensure!(answer == "true", Canceled);
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn yes_no_defaults_retry_and_eof() {
    let input = Input {
      kind: "boolean".into(),
      required: true,
      prompt: Some("Additional command?".into()),
      default: None,
      choices: vec![],
    };
    let mut output = Vec::new();
    assert_eq!(ask(&input, "false", &mut &b"\n"[..], &mut output).unwrap(), "false");
    assert!(String::from_utf8(output).unwrap().contains("[y/N]"));
    let mut output = Vec::new();
    assert_eq!(ask(&input, "false", &mut &b"maybe\nYES\n"[..], &mut output).unwrap(), "true");
    assert!(String::from_utf8(output).unwrap().contains("Please answer yes or no"));
    assert!(ask(&input, "true", &mut &b""[..], &mut Vec::new())
      .unwrap_err()
      .is::<Canceled>());
  }
  #[test]
  fn separators_fit_visible_columns_including_unicode_and_narrow_windows() {
    for (title, columns) in [
      ("DoTask", 80),
      ("Installation Settings", 40),
      ("日本語 Tools", 30),
      ("Installation Summary", 8),
      ("DoTask", 1),
    ] {
      let mut output = Vec::new();
      write_section(&mut output, title, columns, false).unwrap();
      let output = String::from_utf8(output).unwrap();
      assert_eq!(output.trim_end_matches('\n').width(), columns - 1);
      assert_eq!(output.lines().count(), 1);
    }
  }
  #[test]
  fn banner_uses_package_metadata_and_wraps_description() {
    let application = Application {
      id: "example".into(),
      name: "Example".into(),
      version: "1".into(),
      build: None,
      author: "Author".into(),
      copyright: "Copyright (C) 2026 Author".into(),
      description: "An application with a longer description.".into(),
    };
    let mut output = Vec::new();
    write_banner(&mut output, &application, 30).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.starts_with("─── Example "));
    assert!(output.contains("  Copyright (C) 2026 Author\n"));
    assert!(output.contains("  An application with a\n  longer description.\n"));
    assert!(output.ends_with("\n\n"));
  }
}

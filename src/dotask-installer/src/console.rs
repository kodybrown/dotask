use crate::model::Input;
use anyhow::{bail, ensure, Result};
use std::io::{BufRead, Write};

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
    write!(writer, "{prompt} [{hint}]: ")?;
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
        _ => writeln!(writer, "Please answer yes or no.")?,
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
}

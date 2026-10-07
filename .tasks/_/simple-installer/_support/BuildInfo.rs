// Shared by Cargo's build script and repository packaging tasks. Each consumer
// uses only the operations it needs; no runtime SDK or date utility is required.
#![allow(dead_code)]
use std::{
  path::Path,
  process::Command,
  time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
  pub year: u32,
  pub ordinal: u32,
  pub hour: u32,
  pub minute: u32,
}
impl Stamp {
  pub fn now() -> Self {
    Self::from_minutes(
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Clock predates Unix epoch")
        .as_secs()
        / 60,
    )
  }
  pub fn from_minutes(minutes: u64) -> Self {
    let mut days = minutes / 1440;
    let mut year = 1970;
    while days >= year_days(year) as u64 {
      days -= year_days(year) as u64;
      year += 1;
    }
    Self {
      year,
      ordinal: days as u32 + 1,
      hour: (minutes % 1440 / 60) as u32,
      minute: (minutes % 60) as u32,
    }
  }
  pub fn parse(text: &str) -> Result<Self, String> {
    if text.len() != 10
      || !text.is_ascii()
      || text.as_bytes()[5] != b'-'
      || !text.chars().enumerate().all(|(i, c)| i == 5 || c.is_ascii_digit())
    {
      return Err("Build stamp must be UTC YYDDD-HHMM".into());
    }
    let stamp = Self {
      year: 2000 + text[..2].parse::<u32>().unwrap(),
      ordinal: text[2..5].parse().unwrap(),
      hour: text[6..8].parse().unwrap(),
      minute: text[8..].parse().unwrap(),
    };
    if stamp.ordinal == 0 || stamp.ordinal > year_days(stamp.year) || stamp.hour > 23 || stamp.minute > 59 {
      return Err("Invalid UTC build stamp".into());
    }
    Ok(stamp)
  }
  pub fn minutes(&self) -> u64 {
    let days: u64 = (1970..self.year).map(|year| year_days(year) as u64).sum();
    (days + self.ordinal as u64 - 1) * 1440 + self.hour as u64 * 60 + self.minute as u64
  }
  pub fn next(&self) -> Self {
    Self::from_minutes(self.minutes() + 1)
  }
  pub fn text(&self) -> String {
    format!("{:02}{:03}-{:02}{:02}", self.year % 100, self.ordinal, self.hour, self.minute)
  }
  pub fn version(&self, prefix: &str) -> String {
    let mut day = self.ordinal;
    let mut month = 1;
    for days in [
      31,
      if year_days(self.year) == 366 { 29 } else { 28 },
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
      if day <= days {
        break;
      }
      day -= days;
      month += 1;
    }
    format!("{prefix}.{:02}{month:02}.{day:02}{:02}", self.year % 100, self.hour)
  }
  /// Expand an explicit UTC version pattern. DDD is ordinal day, not weekday.
  pub fn format_version(&self, pattern: &str) -> Result<String, String> {
    let calendar = self.version("0");
    let month = &calendar[4..6];
    let day = &calendar[7..9];
    let tokens = [
      ("yyyy", format!("{:04}", self.year)),
      ("DDD", format!("{:03}", self.ordinal)),
      ("yy", format!("{:02}", self.year % 100)),
      ("MM", month.into()),
      ("dd", day.into()),
      ("HH", format!("{:02}", self.hour)),
      ("hh", format!("{:02}", self.hour)),
      ("mm", format!("{:02}", self.minute)),
    ];
    let mut remaining = pattern;
    let mut output = String::new();
    while !remaining.is_empty() {
      if let Some((token, value)) = tokens.iter().find(|(token, _)| remaining.starts_with(token)) {
        output.push_str(value);
        remaining = &remaining[token.len()..];
      } else {
        let next = remaining.chars().next().unwrap();
        output.push(next);
        remaining = &remaining[next.len_utf8()..];
      }
    }
    version_component(&output)?;
    Ok(output)
  }
}
fn year_days(year: u32) -> u32 {
  if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
    366
  } else {
    365
  }
}

pub fn git(root: &Path) -> (String, bool) {
  let output = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(root).output();
  let commit = output
    .ok()
    .filter(|o| o.status.success())
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .unwrap_or_default()
    .trim()
    .to_string();
  if commit.is_empty() {
    return (commit, false);
  }
  let dirty = Command::new("git")
    .args(["status", "--porcelain", "--untracked-files=normal"])
    .current_dir(root)
    .output()
    .map(|o| !o.status.success() || !o.stdout.is_empty())
    .unwrap_or(true);
  (commit, dirty)
}
pub fn revision(commit: &str, dirty: bool) -> String {
  if commit.is_empty() {
    String::new()
  } else {
    format!("-{}{}", &commit[..commit.len().min(7)], if dirty { "-dirty" } else { "" })
  }
}
pub fn version_component(version: &str) -> Result<String, String> {
  if version.trim().is_empty() {
    return Err("Application version is required".into());
  }
  let mut result = String::new();
  for (index, byte) in version.bytes().enumerate() {
    let edge = (byte == b'.' || byte == b' ') && (index == 0 || index + 1 == version.len());
    if !edge && (byte.is_ascii_alphanumeric() || b"-_.+".contains(&byte)) {
      result.push(byte as char);
    } else {
      result.push_str(&format!("%{byte:02X}"));
    }
  }
  if result.len() > 100 {
    return Err("Encoded application version is too long for a build directory".into());
  }
  Ok(result)
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn calendar_version_preserves_prefix_and_minute_rolls_over_dates() {
    let stamp = Stamp::parse("26279-0612").unwrap();
    assert_eq!(stamp.version("0.1"), "0.1.2610.0606");
    assert_eq!(Stamp::from_minutes(stamp.minutes()), stamp);
    assert_eq!(Stamp::parse("26365-2359").unwrap().next().text(), "27001-0000");
    assert_eq!(Stamp::parse("24060-0000").unwrap().version("2.3"), "2.3.2402.2900");
    for bad in ["26000-0000", "26366-0000", "26279-2460", "bad"] {
      assert!(Stamp::parse(bad).is_err());
    }
  }
}

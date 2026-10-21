//! Human UTC build identity; Cargo's committed version retains release precedence.
#![allow(dead_code)]
use std::time::{SystemTime, UNIX_EPOCH};
pub fn timestamp(seconds: u64) -> (String, String) {
  let mut days = seconds / 86400;
  let mut year = 1970;
  let leap = |year| year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
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
  let day = days + 1;
  let hour = seconds % 86400 / 3600;
  let minute = seconds % 3600 / 60;
  let second = seconds % 60;
  (
    format!("{:02}{month:02}.{day:02}{hour:02}", year % 100),
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"),
  )
}
pub fn now() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("Clock predates Unix epoch")
    .as_secs()
}
#[cfg(test)]
mod tests {
  #[test]
  fn build_metadata_preserves_utc_leap_days_and_seconds() {
    assert_eq!(
      super::timestamp(1709164801),
      ("2402.2900".into(), "2024-02-29T00:00:01Z".into())
    );
    assert_eq!(
      super::timestamp(1704067200),
      ("2401.0100".into(), "2024-01-01T00:00:00Z".into())
    );
  }
}

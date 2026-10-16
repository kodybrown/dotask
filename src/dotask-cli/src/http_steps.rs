//! HTTP results are published only after every requested page has succeeded.
use crate::{declarative::keys, project::Target, workflow_values::scalar};
use anyhow::{bail, ensure, Result};
use serde_json::{json, Map, Value};
use std::{
  collections::BTreeSet,
  io::Read,
  time::{Duration, Instant},
};
use ureq::http::Uri;

const LIMIT: usize = 16 * 1024 * 1024;

pub(crate) fn validate(action: &Value, target: &Target) -> Result<()> {
  keys(
    action,
    &[
      "method",
      "url",
      "query",
      "headers",
      "response-format",
      "pagination",
      "timeout-seconds",
    ],
  )?;
  ensure!(action["method"] == "GET", "http currently supports method: GET only.");
  ensure!(action["response-format"] == "json", "http requires response-format: json.");
  crate::process_steps::validate(&action["url"], target)?;
  if let Some(url) = action["url"].as_str() {
    uri(url)?;
  }
  for name in ["query", "headers"] {
    if let Some(value) = action.get(name) {
      let map = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("http {name} requires a mapping."))?;
      for (key, value) in map {
        if name == "headers" {
          key
            .parse::<ureq::http::header::HeaderName>()
            .map_err(|_| anyhow::anyhow!("Invalid HTTP header name."))?;
        }
        ensure!(
          !value.is_null() && !value.is_array(),
          "HTTP {name} values must be scalars or references."
        );
        if value.is_object() {
          crate::process_steps::validate(value, target)?;
        }
      }
    }
  }
  if let Some(timeout) = action.get("timeout-seconds") {
    ensure!(
      timeout.as_u64().is_some_and(|n| (1..=86400).contains(&n)),
      "http timeout-seconds must be an integer from 1 to 86400."
    );
  }
  if let Some(pagination) = action.get("pagination") {
    keys(pagination, &["mode", "relation", "max-pages"])?;
    ensure!(
      pagination["mode"] == "link-header" && pagination["relation"] == "next",
      "http pagination requires mode: link-header and relation: next."
    );
    if let Some(limit) = pagination.get("max-pages") {
      ensure!(
        limit.as_u64().is_some_and(|n| (1..=1000).contains(&n)),
        "max-pages must be an integer from 1 to 1000."
      );
    }
  }
  Ok(())
}

fn uri(value: &str) -> Result<Uri> {
  let uri: Uri = value.parse().map_err(|_| anyhow::anyhow!("Invalid HTTP URL."))?;
  ensure!(
    matches!(uri.scheme_str(), Some("http" | "https")) && uri.authority().is_some_and(|a| !a.as_str().contains('@')),
    "HTTP URL must be absolute http/https without embedded credentials."
  );
  Ok(uri)
}

// Separators inside quoted parameters or URI references are data, not delimiters.
fn parts(value: &str, delimiter: char) -> Vec<&str> {
  let (mut quoted, mut angle, mut escaped, mut start) = (false, false, false, 0);
  let mut parts = Vec::new();
  for (index, c) in value.char_indices() {
    if escaped {
      escaped = false;
      continue;
    }
    match c {
      '\\' if quoted => escaped = true,
      '"' if !angle => quoted = !quoted,
      '<' if !quoted => angle = true,
      '>' if !quoted => angle = false,
      c if c == delimiter && !quoted && !angle => {
        parts.push(&value[start..index]);
        start = index + 1;
      }
      _ => {}
    }
  }
  parts.push(&value[start..]);
  parts
}

fn next_link(value: &str) -> Result<Option<String>> {
  let mut next = None;
  for entry in parts(value, ',') {
    let entry = entry.trim();
    if entry.is_empty() {
      continue;
    }
    ensure!(entry.starts_with('<'), "Invalid pagination Link header.");
    let end = entry
      .find('>')
      .ok_or_else(|| anyhow::anyhow!("Invalid pagination Link header."))?;
    for parameter in parts(&entry[end + 1..], ';') {
      if let Some((name, value)) = parameter.trim().split_once('=')
        && name.trim().eq_ignore_ascii_case("rel")
        && value.trim().trim_matches('"').split_whitespace().any(|s| s == "next")
      {
        ensure!(next.is_none(), "Ambiguous pagination next link.");
        next = Some(entry[1..end].to_owned());
      }
    }
  }
  Ok(next)
}

pub(crate) fn execute(action: Value) -> Result<Value> {
  // DNS/TLS can block in a synchronous client. Poll a worker so Ctrl+C remains
  // responsive; the worker performs reads only and cannot publish task results.
  let (sender, receiver) = std::sync::mpsc::sync_channel(1);
  let timeout = Duration::from_secs(action["timeout-seconds"].as_u64().unwrap_or(30));
  let started = Instant::now();
  std::thread::spawn(move || {
    let _ = sender.send(fetch(&action, timeout));
  });
  loop {
    crate::process::check_cancelled()?;
    ensure!(started.elapsed() < timeout, "HTTP request timed out.");
    match receiver.recv_timeout(Duration::from_millis(50)) {
      Ok(result) => return result,
      Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
      Err(_) => bail!("HTTP worker stopped unexpectedly."),
    }
  }
}

fn fetch(action: &Value, timeout: Duration) -> Result<Value> {
  let mut url = scalar(&action["url"])?;
  let origin = uri(&url)?;
  let paginated = action.get("pagination").is_some();
  let max_pages = action["pagination"]["max-pages"].as_u64().unwrap_or(100);
  let started = Instant::now();
  let mut seen = BTreeSet::new();
  let mut data = Vec::new();
  let mut total = 0;
  for page in 1..=max_pages {
    ensure!(seen.insert(url.clone()), "Repeated pagination URL.");
    let remaining = timeout
      .checked_sub(started.elapsed())
      .ok_or_else(|| anyhow::anyhow!("HTTP request timed out."))?;
    let agent = ureq::Agent::new_with_config(
      ureq::Agent::config_builder()
        .max_redirects(0)
        .max_redirects_will_error(false)
        .timeout_global(Some(remaining))
        .build(),
    );
    let mut request = agent.get(&url).header("User-Agent", "dotask");
    if page == 1
      && let Some(query) = action.get("query").and_then(Value::as_object)
    {
      for (key, value) in query {
        request = request.query(key, scalar(value)?);
      }
    }
    if let Some(headers) = action.get("headers").and_then(Value::as_object) {
      for (key, value) in headers {
        request = request.header(key, scalar(value)?);
      }
    }
    // Client errors may contain credential-bearing URLs; report only the stage.
    let mut response = request.call().map_err(|error| match error {
      ureq::Error::StatusCode(code) => anyhow::anyhow!("HTTP GET returned status {code}."),
      ureq::Error::Timeout(_) => anyhow::anyhow!("HTTP request timed out."),
      _ => anyhow::anyhow!("HTTP GET failed (transport or request error)."),
    })?;
    let status = response.status().as_u16();
    ensure!(
      (200..300).contains(&status),
      "HTTP GET returned status {status}; redirects are not followed."
    );
    let mut headers = Map::new();
    for name in response.headers().keys() {
      let values = response
        .headers()
        .get_all(name)
        .iter()
        .map(|v| v.to_str().map(str::to_owned))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| anyhow::anyhow!("HTTP response has non-text headers."))?;
      headers.insert(name.to_string(), json!(values));
    }
    let links = response
      .headers()
      .get_all("link")
      .iter()
      .map(|v| v.to_str().unwrap_or(""))
      .collect::<Vec<_>>()
      .join(",");
    let mut bytes = Vec::new();
    response
      .body_mut()
      .as_reader()
      .take((LIMIT - total + 1) as u64)
      .read_to_end(&mut bytes)
      .map_err(|_| anyhow::anyhow!("HTTP response read failed."))?;
    total += bytes.len();
    ensure!(total <= LIMIT, "HTTP responses exceed 16 MiB total.");
    let value: Value =
      serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("HTTP response is not valid JSON."))?;
    if !paginated {
      return Ok(json!({"status-code":status,"headers":headers,"data":value,"pages":1}));
    }
    data.extend(
      value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Paginated HTTP JSON must be an array on every page."))?
        .iter()
        .cloned(),
    );
    let Some(next) = next_link(&links)? else {
      return Ok(json!({"status-code":status,"headers":headers,"data":data,"pages":page}));
    };
    let next_uri = uri(&next)?;
    ensure!(
      next_uri.scheme_str() == origin.scheme_str() && next_uri.authority() == origin.authority(),
      "Pagination next URL must have the original scheme and authority."
    );
    url = next;
  }
  bail!("HTTP pagination exceeds max-pages; no partial result was published.")
}

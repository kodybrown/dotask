//! Patch selected YAML tokens instead of serializing the author's document.
//! Unsupported edits fail before publication; unrelated YAML and body stay literal.
use crate::{project::Target, workflow_values};
use anyhow::{bail, ensure, Result};
use serde_json::{json, Value};
use yaml_rust2::{
  parser::{Event, Parser},
  scanner::Marker,
};

pub(crate) fn validate(action: &Value, target: &Target) -> Result<()> {
  if let Some(flag) = action.get("create-if-missing") {
    ensure!(flag.is_boolean(), "create-if-missing must be boolean.");
  }
  for name in ["initial", "fill-empty", "set", "match-existing"] {
    if let Some(values) = action.get(name) {
      let values = values
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("frontmatter {name} must be a mapping."))?;
      for (path, value) in values {
        ensure!(
          path
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))),
          "Frontmatter paths need simple dot-separated YAML keys."
        );
        workflow_values::validate(value, target)?;
      }
      let paths: Vec<_> = values.keys().collect();
      ensure!(
        !paths
          .iter()
          .any(|a| paths.iter().any(|b| a != b && b.starts_with(&format!("{a}.")))),
        "Frontmatter field paths cannot overlap within {name}."
      );
    }
  }
  Ok(())
}

fn metadata(text: &str) -> Result<Value> {
  let value = if text.trim().is_empty() {
    json!({})
  } else {
    crate::yaml::parse(text, "YAML frontmatter")?
  };
  ensure!(value.is_object(), "YAML frontmatter must be a mapping.");
  Ok(value)
}
fn lookup<'a>(data: &'a Value, path: &str) -> Option<&'a Value> {
  let mut value = data;
  for key in path.split('.') {
    value = value.get(key)?;
  }
  Some(value)
}

pub(crate) fn update(original: Option<&str>, action: &Value) -> Result<String> {
  let text = original.unwrap_or("");
  let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
  let mut lines = text.split_inclusive('\n');
  let first = lines.next().unwrap_or("");
  let has_header = first.trim_end_matches(['\r', '\n']) == "---";
  let (mut yaml, prefix, suffix) = if has_header {
    let mut end = first.len();
    let mut closing = None;
    for line in lines {
      if line.trim_end_matches(['\r', '\n']) == "---" {
        closing = Some((end, end + line.len()));
        break;
      }
      end += line.len();
    }
    let (end, after) = closing.ok_or_else(|| anyhow::anyhow!("Unclosed YAML frontmatter; file was preserved."))?;
    (text[first.len()..end].to_owned(), &text[..first.len()], &text[end..after])
  } else {
    (String::new(), "", "")
  };
  let before = yaml.clone();
  let mut data = metadata(&yaml)?;
  if let Some(values) = action.get("match-existing").and_then(Value::as_object) {
    for (path, expected) in values {
      if let Some(actual) = lookup(&data, path).filter(|v| !v.is_null()) {
        ensure!(
          actual == expected,
          "Frontmatter identity mismatch at '{path}'; file was preserved."
        );
      }
    }
  }
  for name in ["initial", "fill-empty", "set"] {
    if name == "initial" && original.is_some() {
      continue;
    }
    if let Some(values) = action.get(name).and_then(Value::as_object) {
      for (path, value) in values {
        let existing = lookup(&data, path);
        let empty = existing.is_none_or(|v| v.is_null() || v.as_str().is_some_and(|s| s.trim().is_empty()));
        if name == "fill-empty" && (!empty || value.is_null() || value.as_str().is_some_and(|s| s.trim().is_empty())) {
          continue;
        }
        if existing == Some(value) {
          continue;
        }
        yaml = patch(&yaml, path, value, eol)?;
        data = metadata(&yaml)?;
        ensure!(
          lookup(&data, path) == Some(value),
          "Cannot safely represent frontmatter field '{path}'."
        );
      }
    }
  }
  if original.is_some() && yaml == before {
    return Ok(text.into());
  }
  if has_header {
    // Only the header substring changes; the closing delimiter and every body
    // character are copied verbatim, including the final-newline policy.
    let body_start = prefix.len() + before.len() + suffix.len();
    Ok(format!("{prefix}{yaml}{suffix}{}", &text[body_start..]))
  } else {
    Ok(format!("---{eol}{yaml}---{eol}{text}"))
  }
}

struct Node {
  end: Marker,
  anchored: bool,
  entries: Option<Vec<(String, Marker, Node)>>,
}
fn node(parser: &mut Parser<std::str::Chars<'_>>, event: Event, mark: Marker) -> Result<Node> {
  let mut result = Node {
    end: mark,
    anchored: false,
    entries: None,
  };
  match event {
    Event::MappingStart(anchor, tag) => {
      result.anchored = anchor != 0 || tag.is_some();
      let mut entries = Vec::new();
      loop {
        let (event, key_mark) = parser.next_token()?;
        if event == Event::MappingEnd {
          result.end = key_mark;
          break;
        }
        let Event::Scalar(key, _, _, _) = event else {
          bail!("Complex frontmatter keys cannot be patched.");
        };
        let (event, mark) = parser.next_token()?;
        entries.push((key, key_mark, node(parser, event, mark)?));
      }
      result.entries = Some(entries);
    }
    Event::SequenceStart(anchor, tag) => {
      result.anchored = anchor != 0 || tag.is_some();
      loop {
        let (event, mark) = parser.next_token()?;
        if event == Event::SequenceEnd {
          result.end = mark;
          break;
        }
        node(parser, event, mark)?;
      }
    }
    Event::Scalar(_, _, anchor, tag) => result.anchored = anchor != 0 || tag.is_some(),
    Event::Alias(_) => result.anchored = true,
    _ => bail!("Unsupported frontmatter layout."),
  }
  Ok(result)
}
fn line_start(text: &str, line: usize) -> usize {
  text.split_inclusive('\n').take(line.saturating_sub(1)).map(str::len).sum()
}
fn plain_entry(text: &str, key: &str, mark: Marker) -> Result<(usize, usize)> {
  let start = line_start(text, mark.line());
  let line = text[start..].split('\n').next().unwrap_or("").trim_end_matches('\r');
  let indent = line.len() - line.trim_start_matches(' ').len();
  ensure!(
    line[indent..].starts_with(&format!("{key}:")),
    "Touched frontmatter keys must use plain block YAML."
  );
  Ok((start, indent))
}
fn emit(path: &[&str], value: &Value, indent: usize, eol: &str) -> Result<String> {
  let mut text = String::new();
  for (index, key) in path.iter().enumerate() {
    text += &" ".repeat(indent + 2 * index);
    text += key;
    text += ":";
    if index + 1 == path.len() {
      text += " ";
      text += &serde_json::to_string(value)?;
    }
    text += eol;
  }
  Ok(text)
}
fn patch(text: &str, path: &str, value: &Value, eol: &str) -> Result<String> {
  if text.trim().is_empty() {
    return Ok(format!("{text}{}", emit(&path.split('.').collect::<Vec<_>>(), value, 0, eol)?));
  }
  let mut parser = Parser::new_from_str(text);
  parser.next_token()?; // stream
  parser.next_token()?; // document
  let (event, mark) = parser.next_token()?;
  let root = node(&mut parser, event, mark)?;
  let mut current = &root;
  let segments: Vec<_> = path.split('.').collect();
  for (index, key) in segments.iter().enumerate() {
    ensure!(!current.anchored, "Anchored/tagged frontmatter mappings cannot be patched.");
    let entries = current
      .entries
      .as_ref()
      .ok_or_else(|| anyhow::anyhow!("Frontmatter parent for '{path}' is not a mapping."))?;
    let first = entries
      .first()
      .ok_or_else(|| anyhow::anyhow!("Empty flow mappings cannot be patched; use block YAML."))?;
    let (_, indent) = plain_entry(text, &first.0, first.1)?;
    if let Some((name, key_mark, child)) = entries.iter().find(|(name, _, _)| name == key) {
      let (start, indent) = plain_entry(text, name, *key_mark)?;
      if index + 1 != segments.len() {
        current = child;
        continue;
      }
      ensure!(!child.anchored, "Anchored/tagged frontmatter values cannot be patched.");
      let end = text[start..].find('\n').map(|n| start + n).unwrap_or(text.len());
      let line = text[start..end].trim_end_matches('\r');
      let offset = indent + name.len() + 1;
      let raw = &line[offset..];
      let comment = comment_start(raw);
      let token = raw[..comment].trim();
      let old = lookup(&metadata(text)?, path).cloned().unwrap_or(Value::Null);
      let snippet = crate::yaml::parse(&format!("value: {token}\n"), "frontmatter token")?;
      ensure!(
        snippet["value"] == old,
        "Multiline/block frontmatter values cannot be patched safely; file was preserved."
      );
      let leading = raw.len() - raw.trim_start_matches(' ').len();
      let token_end = raw[..comment].trim_end().len();
      let left = start + offset + leading.min(token_end);
      let right = start + offset + token_end;
      let replacement = serde_json::to_string(value)?;
      let spacer = if token_end == 0 { " " } else { "" };
      return Ok(format!("{}{spacer}{replacement}{}", &text[..left], &text[right..]));
    }
    let insertion = line_start(text, current.end.line()).min(text.len());
    return Ok(format!(
      "{}{}{}",
      &text[..insertion],
      emit(&segments[index..], value, indent, eol)?,
      &text[insertion..]
    ));
  }
  unreachable!()
}
fn comment_start(text: &str) -> usize {
  let (mut quote, mut escaped, mut previous) = (None, false, ' ');
  for (index, c) in text.char_indices() {
    if escaped {
      escaped = false;
      continue;
    }
    if c == '\\' && quote == Some('"') {
      escaped = true;
      continue;
    }
    if let Some(q) = quote {
      if c == q {
        quote = None;
      }
    } else if matches!(c, '\'' | '"') {
      quote = Some(c);
    } else if c == '#' && previous.is_whitespace() {
      return index;
    }
    previous = c;
  }
  text.len()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn creation_refresh_and_noop_preserve_editorial_content() {
    let action = json!({"initial":{"title":"FixEOL","summary":"","hidden":false},"set":{"github.id":123,"github.description":"first","github.archived":false,"github.pushedAt":"2026-10-13T23:17:05Z"}});
    let created = update(None, &action).unwrap();
    assert!(created.contains("github:\n  id: 123\n"), "{created}");
    let author = created.replace("summary: \"\"", "summary: 'My own summary' # keep this")
      + "\nMy notes.\n![Screenshot](/image.png)";
    let mut next = action.clone();
    next["set"]["github.description"] = json!("second: # quoted\nnext line");
    next["set"]["github.archived"] = json!(true);
    let updated = update(Some(&author), &next).unwrap();
    assert!(updated.contains("summary: 'My own summary' # keep this"));
    assert!(updated.ends_with("My notes.\n![Screenshot](/image.png)"));
    assert_eq!(update(Some(&updated), &next).unwrap(), updated);
    assert!(updated.contains("pushedAt: \"2026-10-13T23:17:05Z\""));
  }

  #[test]
  fn nested_insertions_comments_nulls_unicode_and_crlf_survive() {
    let original = "---\r\n# editorial\r\ntitle: 'Caf\u{e9}'\r\nsummary: |\r\n  Handwritten\r\n  paragraphs.\r\ngithub:\r\n  id: 123 # identity\r\n  description: old  # keep description comment\r\n# Other configuration\r\nhidden: false\r\n---\r\n\r\nBody \u{1f680}";
    let action = json!({"initial":{"title":"ignored"},"set":{"github.description":null,"github.language":"Rust","github.archived":false}});
    let text = update(Some(original), &action).unwrap();
    assert!(text.contains("description: null  # keep description comment"), "{text}");
    assert!(text.contains("  language: \"Rust\"\r\n"), "{text}");
    assert!(text.contains("summary: |\r\n  Handwritten\r\n  paragraphs."));
    assert!(text.contains("# Other configuration"));
    assert!(text.ends_with("---\r\n\r\nBody \u{1f680}"));
  }

  #[test]
  fn fill_empty_and_identity_checks_are_explicit() {
    let original = "---\ntitle: ' '\nempty: null\nflag: false\ncount: 0\nlist: []\ngithub:\n  id: 123\n---\nnotes";
    let action = json!({"fill-empty":{"title":"Filled","empty":"yes","flag":true,"count":42,"list":[1]},"match-existing":{"github.id":123}});
    let text = update(Some(original), &action).unwrap();
    assert!(text.contains("title: \"Filled\""));
    assert!(text.contains("flag: false\ncount: 0\nlist: []"));
    assert!(update(
      Some(original),
      &json!({"match-existing":{"github.id":456},"set":{"title":"wrong"}})
    )
    .is_err());
  }

  #[test]
  fn unsafe_layouts_fail_and_existing_bodies_can_gain_a_header() {
    for text in [
      "---\ntitle: old\nbody",
      "---\ngithub: {id: 1}\n---\nbody",
      "---\ntitle: &title old\nalias: *title\n---\nbody",
      "---\ntitle: |\n  handwritten\n---\nbody",
      "---\ntitle: one\ntitle: two\n---\nbody",
    ] {
      let action = if text.contains("github:") {
        json!({"set":{"github.id":2}})
      } else {
        json!({"set":{"title":"new"}})
      };
      assert!(update(Some(text), &action).is_err(), "{text}");
    }
    let body = "# My page\n\nnotes without final newline";
    let text = update(Some(body), &json!({"initial":{"title":"not applied"},"set":{"github.id":1}})).unwrap();
    assert!(text.ends_with(body));
    assert!(!text.contains("not applied"));
  }
}

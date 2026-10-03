use anyhow::{bail, Result};
use serde_json::{Map, Value};
use std::collections::HashMap;
use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::TScalarStyle;

// Resolve scalars ourselves: YAML's implicit hex/octal/inf conversions differ
// from dotask's established decimal, finite-number configuration contract.
pub(crate) fn parse(text: &str, source: &str) -> Result<Value> {
  let mut reader = Reader {
    parser: Parser::new_from_str(text.trim_start_matches('\u{feff}')),
    anchors: HashMap::new(),
    source,
    nodes: 0,
  };
  reader.expect(Event::StreamStart)?;
  reader.expect(Event::DocumentStart)?;
  let first = reader.next()?;
  let value = reader.node(first, 0, false)?;
  reader.expect(Event::DocumentEnd)?;
  reader.expect(Event::StreamEnd)?;
  Ok(value)
}

struct Reader<'a> {
  parser: Parser<std::str::Chars<'a>>,
  anchors: HashMap<usize, Value>,
  source: &'a str,
  nodes: usize,
}

impl Reader<'_> {
  fn next(&mut self) -> Result<Event> {
    Ok(self.parser.next_token()?.0)
  }
  fn expect(&mut self, expected: Event) -> Result<()> {
    if self.next()? != expected {
      bail!("{} must contain one YAML document.", self.source);
    }
    Ok(())
  }
  fn node(&mut self, event: Event, depth: usize, key: bool) -> Result<Value> {
    self.nodes += 1;
    if depth > 32 {
      bail!("{} nesting exceeds 32 levels (or contains a recursive alias).", self.source);
    }
    if self.nodes > 100_000 {
      bail!("{} contains too many YAML nodes.", self.source);
    }
    let (anchor, result) = match event {
      Event::Scalar(raw, style, anchor, tag) => {
        let quoted =
          style != TScalarStyle::Plain || tag.is_some_and(|t| t.handle == "tag:yaml.org,2002:" && t.suffix == "str");
        (
          anchor,
          if key || quoted {
            Value::String(raw)
          } else {
            scalar(&raw)
          },
        )
      }
      Event::MappingStart(anchor, _) if !key => {
        let mut values = Map::new();
        loop {
          let event = self.next()?;
          if event == Event::MappingEnd {
            break;
          }
          let name = self.node(event, depth + 1, true)?;
          let Some(name) = name.as_str().filter(|s| !s.is_empty()) else {
            bail!("{} requires nonempty string keys.", self.source);
          };
          if values.keys().any(|k: &String| crate::configuration::same_key(k, name)) {
            bail!("{} requires nonempty string keys unique regardless of case.", self.source);
          }
          let event = self.next()?;
          values.insert(name.to_owned(), self.node(event, depth + 1, false)?);
        }
        (anchor, Value::Object(values))
      }
      Event::SequenceStart(anchor, _) if !key => {
        let mut values = Vec::new();
        loop {
          let event = self.next()?;
          if event == Event::SequenceEnd {
            break;
          }
          values.push(self.node(event, depth + 1, false)?);
        }
        (anchor, Value::Array(values))
      }
      Event::Alias(anchor) => {
        let value = self
          .anchors
          .get(&anchor)
          .ok_or_else(|| anyhow::anyhow!("{} contains a recursive or unknown alias.", self.source))?;
        // Bound expanded aliases as well as the input nesting to avoid explosive
        // copies and aliases that bypass the ordinary depth limit.
        fn size(value: &Value, depth: usize) -> Result<usize> {
          if depth > 32 {
            bail!("YAML alias nesting exceeds 32 levels.");
          }
          let children: Vec<&Value> = match value {
            Value::Array(a) => a.iter().collect(),
            Value::Object(o) => o.values().collect(),
            _ => vec![],
          };
          let mut count = 1;
          for child in children {
            count += size(child, depth + 1)?;
            if count > 100_000 {
              bail!("YAML alias expansion is too large.");
            }
          }
          Ok(count)
        }
        self.nodes += size(value, depth)?;
        if self.nodes > 100_000 {
          bail!("YAML alias expansion is too large.");
        }
        if key && !value.is_string() {
          bail!("YAML requires string keys.");
        }
        return Ok(value.clone());
      }
      _ => bail!("Unsupported YAML value in {}.", self.source),
    };
    if anchor != 0 {
      self.anchors.insert(anchor, result.clone());
    }
    Ok(result)
  }
}

fn scalar(raw: &str) -> Value {
  let value = raw.trim();
  if raw.is_empty() || raw == "~" || raw.eq_ignore_ascii_case("null") {
    Value::Null
  } else if value.eq_ignore_ascii_case("true") {
    Value::Bool(true)
  } else if value.eq_ignore_ascii_case("false") {
    Value::Bool(false)
  } else if let Ok(number) = value.parse::<i64>() {
    Value::from(number)
  } else if let Ok(number) = value.parse::<f64>()
    && number.is_finite()
  {
    Value::from(number)
  } else {
    Value::String(raw.to_owned())
  }
}

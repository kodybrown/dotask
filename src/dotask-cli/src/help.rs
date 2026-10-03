use crate::{
  configuration,
  project::{Catalog, Directory, Target},
};
use serde_json::Value;
use std::{io::IsTerminal, path::Path};

pub(crate) struct Text {
  width: Option<usize>,
}
impl Text {
  pub fn new() -> Self {
    Self {
      width: console_width().filter(|w| *w >= 60),
    }
  }
  pub fn line(&self, value: &str, indent: usize) {
    let Some(width) = self.width else {
      println!("{value}");
      return;
    };
    let indent = " ".repeat(if indent < width - 1 { indent } else { 2 });
    for (i, line) in value.replace("\r\n", "\n").replace('\r', "\n").split('\n').enumerate() {
      let mut remaining = if i == 0 || line.is_empty() {
        line.into()
      } else {
        format!("{indent}{line}")
      };
      if remaining.len() - remaining.trim_start().len() >= width - 1 {
        remaining = format!("  {}", remaining.trim_start());
      }
      while remaining.chars().count() > width {
        let chars: Vec<_> = remaining.char_indices().collect();
        let mut split = width;
        while split > 0 && !chars[split].1.is_whitespace() {
          split -= 1;
        }
        if split <= remaining.chars().take_while(|c| c.is_whitespace()).count() {
          split = width;
        }
        let byte = chars[split].0;
        println!("{}", remaining[..byte].trim_end());
        remaining = format!("{indent}{}", remaining[byte..].trim_start());
      }
      println!("{remaining}");
    }
  }
  pub fn row(&self, prefix: &str, value: &str) -> usize {
    let prefix = if self.width.is_some_and(|w| prefix.chars().count() >= w - 1) {
      if !prefix.trim().is_empty() {
        self.line(prefix.trim_end(), 2);
      }
      "  "
    } else {
      prefix
    };
    self.line(
      &if value.is_empty() {
        prefix.trim_end().into()
      } else {
        format!("{prefix}{value}")
      },
      prefix.chars().count(),
    );
    prefix.chars().count()
  }
  fn section(&self, name: &str) {
    self.line(&format!("\n{name}:"), 0);
  }
}
fn console_width() -> Option<usize> {
  if !std::io::stdout().is_terminal() {
    return None;
  }
  #[cfg(windows)]
  {
    use windows::Win32::System::Console::{
      GetConsoleScreenBufferInfo, GetStdHandle, CONSOLE_SCREEN_BUFFER_INFO, STD_OUTPUT_HANDLE,
    };
    // Read the visible console window, never create or attach a console.
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
    (unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) } == 0).then_some(size.ws_col as usize)
  }
}
pub(crate) fn initialization() {
  let out = Text::new();
  out.line("Usage: dotask --init [--use-dir PATH]\nInitialize the current directory, without searching parent projects.\nCreate missing .dotasks.yaml and .tasks/ entries; never overwrite existing files.\nThe initial name comes from the directory; description and shared settings start empty.\n", 0);
  out.row("  --use-dir PATH  ", "Create/use a task subdirectory beneath the current directory. Pass this override on subsequent commands too; it is not saved.");
  out.row("  --help, -h      ", "Show this help without creating files.");
  out.line("\nInitialization is offline and needs no language SDK. It does not add tasks or create a lock file.\nThen edit .dotasks.yaml and write tasks, or select shared tasks with dotask --add GROUP/TASK.", 0);
}
pub(crate) fn project(catalog: &Catalog, directory: &Directory, verbose: bool) {
  let out = Text::new();
  let config = &catalog.configuration;
  if verbose {
    let fallback = directory
      .root_directory
      .file_name()
      .unwrap_or(directory.root_directory.as_os_str())
      .to_string_lossy();
    out.line(config["Name"].as_str().unwrap_or(&fallback), 0);
    if let Some(s) = config["Description"].as_str() {
      out.line(s, 0);
    }
    out.section("Settings");
    settings(&out, &config["Settings"], "  ");
    out.row(
      "  tasks: ",
      &quote(&relative(&directory.root_directory, &directory.task_directory)),
    );
    out.line("", 0);
  }
  out.line("Targets:", 0);
  for target in &catalog.targets {
    let diagnostic = configuration::bind(target, config, &[], &directory.root_directory, true)
      .err()
      .map(|e| e.to_string());
    out.row(
      &format!("  {:18} ", catalog.display_name(target)),
      diagnostic.as_deref().unwrap_or(&target.description),
    );
  }
  if catalog.targets.is_empty() {
    out.line("  (no targets)", 0);
  }
  if verbose {
    combined(&out, catalog);
  }
  out.line("\nSee `dotask help <target>` for detailed information on each target.", 0);
}
pub(crate) fn target(catalog: &Catalog, target: &Target, directory: &Directory) {
  let out = Text::new();
  let diagnostic = configuration::bind(target, &catalog.configuration, &[], &directory.root_directory, true)
    .err()
    .map(|e| e.to_string());
  out.line("Target:", 0);
  let indent = out.row(
    &format!("  {:14}  ", catalog.display_name(target)),
    diagnostic.as_deref().unwrap_or(&target.description),
  );
  let source = relative(&directory.root_directory, &target.file_path);
  out.row(
    &format!("{}Source: ", " ".repeat(indent)),
    &if source.contains(' ') {
      format!("'{source}'")
    } else {
      source
    },
  );
  if let Some(remarks) = target.metadata.get("Remarks").and_then(Value::as_str) {
    out.line("", 0);
    out.row("  ", remarks);
  }
  if let Some(group) = &target.group {
    out.section("Steps");
    out.line(&format!("  Require at least one step: {}", group.require_at_least_one_step), 0);
    for step in &group.steps {
      out.row(
        "  - ",
        &format!(
          "{} ({})",
          step.run,
          if step.optional {
            "optional"
          } else {
            "required"
          }
        ),
      );
      for (name, value) in step.parameters.as_object().unwrap() {
        out.row(&format!("    with {name}: "), &value.to_string());
      }
    }
  }
  if !target.options.is_empty() {
    out.section("Options");
    options(&out, &entries(target, &catalog.configuration));
  }
  if let Some(requirements) = target
    .metadata
    .get("Requirements")
    .and_then(Value::as_array)
    .filter(|r| !r.is_empty())
  {
    out.section("Requires");
    for r in requirements {
      out.row(&format!("  - {}: ", r["Kind"].as_str().unwrap()), r["Value"].as_str().unwrap());
    }
  }
  for (key, prefix) in [("Capabilities", "  - "), ("Examples", "  ")] {
    if let Some(values) = target.metadata.get(key).and_then(Value::as_array).filter(|v| !v.is_empty()) {
      out.section(key);
      for value in values {
        out.row(prefix, value.as_str().unwrap_or_default());
      }
    }
  }
}
fn entries(target: &Target, config: &Value) -> Vec<(Value, Option<String>)> {
  let Ok(defaults) = configuration::defaults(config, &target.name) else {
    return vec![];
  };
  let defaults = Value::Object(defaults);
  target
    .options
    .iter()
    .map(|o| {
      (
        o.clone(),
        configuration::get(&defaults, o["Name"].as_str().unwrap())
          .or_else(|| configuration::get(o, "Default"))
          .and_then(Value::as_str)
          .map(str::to_owned),
      )
    })
    .collect()
}
fn combined(out: &Text, catalog: &Catalog) {
  let mut groups: Vec<Vec<(Value, Option<String>)>> = vec![];
  let signature = |o: &Value| {
    let mut choices: Vec<_> = o["Choices"]
      .as_array()
      .unwrap()
      .iter()
      .map(|s| s.as_str().unwrap().to_lowercase())
      .collect();
    choices.sort();
    serde_json::json!([
      o["Name"].as_str().unwrap().to_lowercase(),
      o["Alias"].as_str().map(str::to_lowercase),
      o["Type"],
      o["Required"],
      o["Completion"],
      choices
    ])
  };
  for (option, default) in catalog
    .targets
    .iter()
    .filter(|t| t.error.is_none())
    .flat_map(|t| entries(t, &catalog.configuration))
  {
    if let Some(group) = groups.iter_mut().find(|g| signature(&g[0].0) == signature(&option)) {
      group.push((option, default));
    } else {
      groups.push(vec![(option, default)]);
    }
  }
  let mut merged = vec![];
  for group in groups {
    let mut first = group[0].clone();
    let mut descriptions: Vec<(String, usize)> = vec![];
    for (o, _) in &group {
      let description = o["Description"].as_str().unwrap_or_default();
      if description.is_empty() {
        continue;
      }
      if let Some((_, count)) = descriptions.iter_mut().find(|(d, _)| d == description) {
        *count += 1;
      } else {
        descriptions.push((description.into(), 1));
      }
    }
    descriptions.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    if let Some((d, _)) = descriptions.first() {
      first.0["Description"] = Value::String(d.clone());
    }
    let normalize = |s: &Option<String>| {
      if first.0["Choices"].as_array().unwrap().is_empty() {
        s.clone()
      } else {
        s.as_ref().map(|s| s.to_lowercase())
      }
    };
    if group.iter().any(|(_, d)| normalize(d) != normalize(&first.1)) {
      first.1 = None;
    }
    merged.push(first);
  }
  if !merged.is_empty() {
    out.section("Target options");
    options(out, &merged);
  }
}
fn options(out: &Text, entries: &[(Value, Option<String>)]) {
  let rows: Vec<_> = entries
    .iter()
    .map(|(o, d)| {
      let choices: Vec<_> = o["Choices"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
      let signature = format!(
        "--{}{} <{}>",
        o["Name"].as_str().unwrap(),
        o["Alias"].as_str().map(|a| format!(", -{a}")).unwrap_or_default(),
        if choices.is_empty() {
          o["Type"].as_str().unwrap().into()
        } else {
          choices.join("|")
        }
      );
      let description = format!(
        "{}{}{}",
        o["Description"].as_str().unwrap_or_default(),
        if o["Required"] == true {
          " (required)"
        } else {
          ""
        },
        d.as_ref().map(|d| format!(" (default: {d})")).unwrap_or_default()
      );
      (signature, description.trim_start().to_owned())
    })
    .collect();
  let width = rows.iter().map(|r| r.0.len()).max().unwrap_or(0).min(40);
  for (signature, description) in rows {
    if signature.len() > width {
      out.line(&format!("  {signature}"), 2);
      if !description.is_empty() {
        out.row(&" ".repeat(width + 4), &description);
      }
    } else {
      out.row(&format!("  {signature:width$}  "), &description);
    }
  }
}
fn settings(out: &Text, settings: &Value, indent: &str) {
  let Some(object) = settings.as_object() else {
    return;
  };
  let mut entries: Vec<_> = object.iter().collect();
  entries.sort_by_key(|(k, _)| k.to_lowercase());
  for (name, value) in entries {
    if value.as_object().is_some_and(|o| !o.is_empty()) {
      out.line(&format!("{indent}{name}:"), 0);
      self::settings(out, value, &format!("{indent}  "));
    } else {
      out.row(
        &format!("{indent}{name}: "),
        &value.as_str().map(quote).unwrap_or_else(|| value.to_string()),
      );
    }
  }
}
fn quote(s: &str) -> String {
  let escaped: String = s
    .chars()
    .map(|c| match c {
      '\n' => "\\n".into(),
      '\r' => "\\r".into(),
      '\t' => "\\t".into(),
      c if c.is_control() => format!("\\u{:04x}", c as u32),
      c => c.to_string(),
    })
    .collect();
  if s.is_empty() || s.chars().any(|c| c.is_whitespace() || c.is_control()) {
    format!("'{}'", escaped.replace('\'', "''"))
  } else {
    escaped
  }
}
fn relative(root: &Path, file: &Path) -> String {
  // Discovery keeps roots and target paths in the same lexical spelling.
  format!(
    "./{}",
    file.strip_prefix(root).unwrap_or(file).to_string_lossy().replace('\\', "/")
  )
}

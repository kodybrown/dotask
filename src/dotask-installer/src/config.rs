use crate::{
  console, files,
  model::{Input, Package},
  user_path,
};
use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;
use std::{
  collections::BTreeMap,
  io::{BufRead, IsTerminal, Write},
  path::{Path, PathBuf},
};

#[derive(Default, Debug)]
pub struct Options {
  pub uninstall: bool,
  pub interactive: Option<bool>,
  pub validate: bool,
  pub config: Option<PathBuf>,
  pub profile: Option<String>,
  pub remove_settings: bool,
  pub settings_explicit: bool,
  pub values: BTreeMap<String, Value>,
  pub help: bool,
}
impl Options {
  pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
    let mut result = Self::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
      match arg.as_str() {
        "install" => (),
        "uninstall" => result.uninstall = true,
        "--help" | "-h" => result.help = true,
        "--interactive" | "--non-interactive" => {
          ensure!(result.interactive.is_none(), "Specify only one interaction flag");
          result.interactive = Some(arg == "--interactive");
        }
        "--validate" => result.validate = true,
        "--config" => result.config = Some(args.next().context("--config requires a path")?.into()),
        "--profile" => result.profile = Some(args.next().context("--profile requires a name")?),
        "--remove-settings" | "--leave-settings" => {
          ensure!(!result.settings_explicit, "Specify only one settings flag");
          result.settings_explicit = true;
          result.remove_settings = arg == "--remove-settings";
        }
        "--set" => {
          let item = args.next().context("--set requires name=value")?;
          let (name, value) = item.split_once('=').context("--set requires name=value")?;
          ensure!(
            result.values.insert(name.into(), Value::String(value.into())).is_none(),
            "Duplicate input: {name}"
          );
        }
        "--install-dir" | "--bin-dir" | "--shortcut-name" => {
          let value = args.next().with_context(|| format!("{arg} requires a value"))?;
          ensure!(
            result.values.insert(arg[2..].into(), value.into()).is_none(),
            "Duplicate input: {arg}"
          );
        }
        "--prune-old-versions"
        | "--desktop-shortcuts"
        | "--start-menu-shortcuts"
        | "--local-shortcuts"
        | "--additional-command"
        | "--start-menu-nested"
        | "--add-to-path" => {
          result.values.insert(arg[2..].into(), true.into());
        }
        _ => bail!("Unknown argument: {arg}. Use --help."),
      }
    }
    Ok(result)
  }
  pub fn prompts(&self, package: &Package) -> Result<bool> {
    let interactive = !self.validate && self.interactive.unwrap_or(package.runtime.interactive);
    ensure!(
      !interactive || (std::io::stdin().is_terminal() && std::io::stdout().is_terminal()),
      "Interactive installation requires a terminal. Use --non-interactive for unattended execution."
    );
    Ok(interactive)
  }
}
pub fn platform() -> &'static str {
  if cfg!(windows) {
    "windows"
  } else if cfg!(target_os = "macos") {
    "macos"
  } else {
    "linux"
  }
}
pub fn architecture() -> &'static str {
  if cfg!(target_arch = "aarch64") {
    "arm64"
  } else {
    "x64"
  }
}
pub fn text(value: &Value) -> Result<String> {
  match value {
    Value::String(s) => Ok(s.clone()),
    Value::Bool(v) => Ok(v.to_string()),
    Value::Number(v) => Ok(v.to_string()),
    _ => bail!("Input must be a scalar"),
  }
}
pub fn builtins() -> Result<BTreeMap<String, String>> {
  let home =
    std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).context("Home directory is unavailable")?;
  let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| format!("{home}/.local/share"));
  let config = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
  Ok(BTreeMap::from([
    ("home".into(), home),
    ("local-app-data".into(), local),
    ("config-home".into(), config),
  ]))
}
pub fn expand(value: &str, values: &BTreeMap<String, Value>) -> Result<String> {
  let mut output = value.to_string();
  for (key, value) in builtins()? {
    output = output.replace(&format!("${{{key}}}"), &value);
  }
  // A bounded expansion supports input references and rejects cycles/missing values.
  for _ in 0..values.len() + 1 {
    let previous = output.clone();
    for (key, value) in values {
      output = output.replace(&format!("${{{key}}}"), &text(value)?);
    }
    if output == previous {
      break;
    }
  }
  ensure!(!output.contains("${"), "Unresolved value: {output}");
  Ok(output)
}
pub fn validate(package: &Package) -> Result<()> {
  ensure!(package.schema == 1, "Unsupported schema: {}", package.schema);
  files::name(&package.application.id)?;
  crate::build_info::version_component(&package.application.version).map_err(anyhow::Error::msg)?;
  if let Some(build) = &package.application.build {
    crate::build_info::Stamp::parse(&build.stamp).map_err(anyhow::Error::msg)?;
    if let Some(commit) = &build.commit {
      ensure!(
        commit.len() >= 7 && commit.len() <= 64 && commit.chars().all(|c| c.is_ascii_hexdigit()),
        "Build commit must be a Git hexadecimal revision (at least seven characters)"
      );
    }
    ensure!(
      !build.dirty || build.commit.is_some(),
      "A dirty build marker requires a Git revision"
    );
  }
  ensure!(!package.application.name.trim().is_empty(), "application.name is required");
  ensure!(
    package.platform == platform() && package.architecture == architecture(),
    "Package targets {}-{}; engine is {}-{}",
    package.platform,
    package.architecture,
    platform(),
    architecture()
  );
  ensure!(
    !package.runtime.commands.is_empty() || !package.runtime.shortcuts.is_empty(),
    "At least one command or shortcut is required"
  );
  for command in &package.runtime.commands {
    files::name(&command.name)?;
    ensure!(
      !["app", "installer"].contains(&command.name.to_ascii_lowercase().as_str()),
      "Command name is reserved by the installation layout"
    );
    files::relative(&command.executable)?;
    ensure!(!command.name.ends_with(".exe"), "Command names omit .exe");
  }
  for shortcut in &package.runtime.shortcuts {
    ensure!(
      !shortcut.arguments.iter().any(|a| a.contains('\0')),
      "Shortcut arguments cannot contain NUL"
    );
    files::relative(&shortcut.executable)?;
    // Validate the template's shape now and its expanded name before staging.
    files::shortcut_name(&shortcut.name)?;
    if let Some(icon) = &shortcut.icon {
      files::relative(icon)?;
    }
    if let Some(dir) = &shortcut.working_directory {
      files::relative(dir)?;
    }
  }
  Ok(())
}
pub fn standard_inputs() -> BTreeMap<String, Input> {
  let mut result = BTreeMap::new();
  for name in [
    "prune-old-versions",
    "desktop-shortcuts",
    "start-menu-shortcuts",
    "start-menu-nested",
    "local-shortcuts",
    "additional-command",
    "add-to-path",
    "confirm-install",
  ] {
    result.insert(
      name.into(),
      Input {
        kind: "boolean".into(),
        required: true,
        prompt: Some(name.replace('-', " ")),
        default: Some((name == "confirm-install" || (name == "add-to-path" && cfg!(windows))).into()),
        choices: vec![],
      },
    );
  }
  result.insert(
    "shortcut-name".into(),
    Input {
      kind: "string".into(),
      required: true,
      prompt: Some("Shortcut name (without extension)".into()),
      default: None,
      choices: vec![],
    },
  );
  for name in ["install-dir", "bin-dir", "desktop-dir", "start-menu-dir"] {
    result.insert(
      name.into(),
      Input {
        kind: "path".into(),
        required: name == "install-dir",
        prompt: Some(name.replace('-', " ")),
        default: None,
        choices: vec![],
      },
    );
  }
  for (name, prompt) in [
    ("install-dir", "Install application in"),
    ("additional-command", "Place an additional command in another directory?"),
    ("bin-dir", "Additional command directory"),
    ("add-to-path", "Add this directory to your user PATH?"),
    ("confirm-install", "Install with these settings?"),
    ("start-menu-shortcuts", "Create a Start Menu shortcut?"),
    ("start-menu-nested", "Place it inside a folder with the same name?"),
  ] {
    result.get_mut(name).unwrap().prompt = Some(prompt.into());
  }
  result.get_mut("prune-old-versions").unwrap().prompt = None;
  result
}
pub fn resolve(package: &Package, options: &Options) -> Result<BTreeMap<String, Value>> {
  resolve_with_io(
    package,
    options,
    options.prompts(package)?,
    &mut std::io::stdin().lock(),
    &mut std::io::stdout(),
    user_path::contains,
  )
}
fn resolve_with_io(
  package: &Package,
  options: &Options,
  interactive: bool,
  reader: &mut impl BufRead,
  writer: &mut impl Write,
  on_path: impl Fn(&Path) -> Result<bool>,
) -> Result<BTreeMap<String, Value>> {
  let mut inputs = standard_inputs();
  inputs.extend(package.runtime.inputs.clone());
  for key in package
    .runtime
    .values
    .keys()
    .chain(options.values.keys())
    .chain(package.runtime.defaults.values().flat_map(|v| v.keys()))
    .chain(package.runtime.profiles.values().flat_map(|p| p.values.keys()))
  {
    ensure!(inputs.contains_key(key), "Unknown input: {key}");
  }
  let home = builtins()?;
  let home_path = &home["home"];
  let local = &home["local-app-data"];
  let root = if cfg!(windows) {
    Path::new(local)
      .join("Programs")
      .join(&package.application.id)
      .to_string_lossy()
      .into_owned()
  } else {
    format!("{home_path}/.local/lib/{}", package.application.id)
  };
  let mut values = BTreeMap::from([
    ("install-dir".into(), root.into()),
    ("shortcut-name".into(), package.application.name.clone().into()),
    ("desktop-dir".into(), format!("{home_path}/Desktop").into()),
  ]);
  if cfg!(windows) {
    let roaming = std::env::var("APPDATA").context("APPDATA is unavailable")?;
    values.insert(
      "start-menu-dir".into(),
      format!("{roaming}/Microsoft/Windows/Start Menu/Programs").into(),
    );
  } else if !cfg!(target_os = "macos") {
    values.insert(
      "start-menu-dir".into(),
      format!(
        "{}/applications",
        std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home_path}/.local/share"))
      )
      .into(),
    );
  }
  for (key, input) in &inputs {
    if let Some(default) = &input.default {
      values.insert(key.clone(), default.clone());
    }
  }
  for layer in ["common", platform()] {
    if let Some(defaults) = package.runtime.defaults.get(layer) {
      values.extend(defaults.clone());
    }
  }
  let detected = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
  let matches: Vec<_> = package
    .runtime
    .profiles
    .iter()
    .filter(|(_, p)| p.detect.iter().any(|d| detected.split(':').any(|s| s.eq_ignore_ascii_case(d))))
    .collect();
  let profile = if let Some(name) = &options.profile {
    Some(package.runtime.profiles.get(name).context("Unknown --profile")?)
  } else {
    ensure!(matches.len() <= 1, "Multiple environment profiles match; specify --profile");
    matches.first().map(|(_, p)| *p)
  };
  if let Some(profile) = profile {
    values.extend(profile.values.clone());
  }
  values.extend(package.runtime.values.clone());
  values.extend(options.values.clone());
  if options.values.contains_key("bin-dir") && !options.values.contains_key("additional-command") {
    values.insert("additional-command".into(), true.into());
  }
  let leading = [
    "install-dir",
    "additional-command",
    "bin-dir",
    "add-to-path",
    "desktop-shortcuts",
    "start-menu-shortcuts",
    "local-shortcuts",
    "shortcut-name",
    "start-menu-nested",
  ];
  let mut order = leading.to_vec();
  order.extend(inputs.keys().map(String::as_str).filter(|key| !leading.contains(key)));
  for key in order {
    let input = &inputs[key];
    ensure!(
      ["string", "path", "boolean", "choice"].contains(&input.kind.as_str()),
      "Invalid input type: {key}"
    );
    let allowed = match key {
      "desktop-shortcuts" => package.runtime.shortcuts.iter().any(|s| s.desktop),
      "start-menu-shortcuts" => package.runtime.shortcuts.iter().any(|s| s.start_menu),
      "local-shortcuts" => package.runtime.shortcuts.iter().any(|s| s.local),
      "desktop-dir" => enabled(&values, "desktop-shortcuts"),
      "start-menu-dir" => enabled(&values, "start-menu-shortcuts"),
      "start-menu-nested" => {
        cfg!(windows)
          && enabled(&values, "start-menu-shortcuts")
          && package.runtime.shortcuts.iter().any(|s| s.start_menu)
      }
      "shortcut-name" => {
        (enabled(&values, "start-menu-shortcuts")
          || enabled(&values, "desktop-shortcuts")
          || enabled(&values, "local-shortcuts"))
          && package.runtime.shortcuts.iter().any(|s| s.name.contains("${shortcut-name}"))
      }
      "bin-dir" => enabled(&values, "additional-command"),
      "add-to-path" => cfg!(windows),
      "confirm-install" => false,
      _ => true,
    };
    if key == "bin-dir" && !values.contains_key(key) {
      // Compute this after the install-dir answer. BIN is deliberately ignored:
      // the suggested command directory is the selected installation's parent.
      let root = path(&values, "install-dir")?;
      values.insert(
        key.into(),
        root
          .parent()
          .context("Installation needs a parent directory")?
          .to_string_lossy()
          .into_owned()
          .into(),
      );
    }
    let already_on_path = key == "add-to-path" && cfg!(windows) && on_path(&command_directory(&values)?)?;
    if already_on_path {
      if interactive {
        writeln!(writer, "  {} is already on PATH.", command_directory(&values)?.display())?;
      }
      // No prompt and no registry write when the selected directory is present.
      values.insert(key.into(), false.into());
    }
    if interactive
      && !options.values.contains_key(key)
      && !(key == "additional-command" && options.values.contains_key("bin-dir"))
      && allowed
      && !already_on_path
    {
      let default = values
        .get(key)
        .map(text)
        .transpose()?
        .map(|v| expand(&v, &values))
        .transpose()?
        .unwrap_or_default();
      let answer = loop {
        let answer = console::ask(input, &default, reader, writer)?;
        if key == "shortcut-name"
          && let Err(error) = files::shortcut_name(&expand(&answer, &values)?)
        {
          writeln!(writer, "  {error}")?;
          continue;
        }
        break answer;
      };
      values.insert(key.into(), answer.into());
    }
    if let Some(value) = values.get(key) {
      let raw = expand(&text(value)?, &values)?;
      if key == "shortcut-name" && allowed {
        files::shortcut_name(&raw)?;
      }
      ensure!(!input.required || !raw.trim().is_empty(), "Required input is empty: {key}");
      let converted = if input.kind == "boolean" {
        match raw.as_str() {
          "true" => Value::Bool(true),
          "false" => Value::Bool(false),
          _ => bail!("{key} must be true or false"),
        }
      } else {
        Value::String(raw.clone())
      };
      ensure!(
        input.kind != "choice" || input.choices.contains(&raw),
        "Invalid choice for {key}: {raw}"
      );
      ensure!(
        ![
          "desktop-shortcuts",
          "start-menu-shortcuts",
          "start-menu-nested",
          "local-shortcuts",
          "add-to-path"
        ]
        .contains(&key)
          || allowed
          || converted != Value::Bool(true),
        "{key} is not allowed by installer.yaml"
      );
      values.insert(key.into(), converted);
    } else {
      ensure!(!input.required, "Missing required input: {key}. Use --set {key}=VALUE");
    }
    if interactive && key == "install-dir" {
      writeln!(writer)?;
    }
  }
  Ok(values)
}
pub fn path(values: &BTreeMap<String, Value>, name: &str) -> Result<PathBuf> {
  let value = text(values.get(name).with_context(|| format!("Missing required input: {name}"))?)?;
  let path = Path::new(&value);
  ensure!(path.is_absolute(), "{name} must be absolute: {value}");
  files::absolute(path, &std::env::current_dir()?)
}
pub fn enabled(values: &BTreeMap<String, Value>, key: &str) -> bool {
  values.get(key) == Some(&Value::Bool(true))
}
pub fn command_directory(values: &BTreeMap<String, Value>) -> Result<PathBuf> {
  path(
    values,
    if enabled(values, "additional-command") {
      "bin-dir"
    } else {
      "install-dir"
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;
  fn package() -> Package {
    serde_json::from_value(json!({"schema":1,"application":{"id":"test","name":"Test","version":"1"},
      "platform":platform(),"architecture":architecture(),"payload":"payload","runtime":{"commands":[{"name":"test","executable":"test.exe"}]}})).unwrap()
  }
  #[test]
  fn interaction_flags_override_yaml_and_reject_conflicts() {
    assert!(package().runtime.interactive);
    assert_eq!(Options::parse(["--non-interactive".into()]).unwrap().interactive, Some(false));
    assert_eq!(Options::parse(["--interactive".into()]).unwrap().interactive, Some(true));
    assert!(Options::parse(["--interactive".into(), "--non-interactive".into()]).is_err());
    let mut package = package();
    package.runtime.interactive = false;
    assert!(!Options::default().prompts(&package).unwrap());
    package.runtime.interactive = true;
    assert!(!Options {
      validate: true,
      ..Options::default()
    }
    .prompts(&package)
    .unwrap());
  }
  #[test]
  fn optional_command_defaults_to_no_and_parent_follows_install_answer() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("custom app 日本語");
    let answers = format!("{}\n\n{}", root.display(), if cfg!(windows) { "n\n" } else { "" });
    let mut output = Vec::new();
    let values = resolve_with_io(
      &package(),
      &Options::default(),
      true,
      &mut answers.as_bytes(),
      &mut output,
      |_| Ok(false),
    )
    .unwrap();
    assert!(!enabled(&values, "additional-command"));
    assert_eq!(
      path(&values, "bin-dir").unwrap(),
      files::absolute(temp.path(), temp.path()).unwrap()
    );
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("[y/N]"));
    assert!(!output.contains("Additional command directory ["));
  }
  #[cfg(windows)]
  #[test]
  fn selected_parent_already_on_path_skips_path_question() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("custom app");
    let parent = files::absolute(temp.path(), temp.path()).unwrap();
    let answers = format!("{}\ny\n\n", root.display());
    let mut output = Vec::new();
    let values = resolve_with_io(
      &package(),
      &Options::default(),
      true,
      &mut answers.as_bytes(),
      &mut output,
      |directory| {
        assert_eq!(directory, parent);
        Ok(true)
      },
    )
    .unwrap();
    assert!(enabled(&values, "additional-command"));
    assert!(!enabled(&values, "add-to-path"));
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("is already on PATH"));
    assert!(!output.contains("Add this directory"));
  }
  #[test]
  fn shortcut_names_reject_reserved_names_traversal_extensions_and_unsafe_characters() {
    for name in [
      "",
      ".",
      "..",
      "CON",
      "con.txt",
      "COM¹",
      "NUL",
      "a/b",
      "a\\b",
      "a:b",
      "name.",
      "name ",
      "a\tname",
      "Probe.lnk",
      "x.desktop",
    ] {
      assert!(files::shortcut_name(name).is_err(), "Accepted {name:?}");
    }
    for name in ["Build Tools", "日本語 café", "Build.v2", "dotask"] {
      files::shortcut_name(name).unwrap();
    }
  }
  #[cfg(windows)]
  #[test]
  fn start_menu_questions_retry_invalid_names_and_allow_nesting() {
    let temp = tempfile::tempdir().unwrap();
    let mut package = package();
    package.runtime.shortcuts =
      serde_json::from_value(json!([{"name":"${shortcut-name}","executable":"test.exe","start-menu":true}])).unwrap();
    let options = Options {
      values: BTreeMap::from([
        (
          "install-dir".into(),
          temp.path().join("app").to_string_lossy().into_owned().into(),
        ),
        ("additional-command".into(), false.into()),
        ("add-to-path".into(), false.into()),
        (
          "start-menu-dir".into(),
          temp.path().join("menu").to_string_lossy().into_owned().into(),
        ),
      ]),
      ..Options::default()
    };
    let mut output = Vec::new();
    let values = resolve_with_io(
      &package,
      &options,
      true,
      &mut "y\nCON\n../escape\nBuild Tools 日本語\ny\n".as_bytes(),
      &mut output,
      |_| Ok(false),
    )
    .unwrap();
    assert_eq!(values["shortcut-name"], "Build Tools 日本語");
    assert!(enabled(&values, "start-menu-nested"));
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Reserved shortcut name"));
    assert!(output.contains("Invalid shortcut name"));
    assert!(output.contains("[y/N]"));
  }
}

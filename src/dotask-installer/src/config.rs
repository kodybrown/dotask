use crate::{
    files,
    model::{Input, Package},
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
};

#[derive(Default, Debug)]
pub struct Options {
    pub package: bool,
    pub output: Option<PathBuf>,
    pub uninstall: bool,
    pub interactive: bool,
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
                "package" => result.package = true,
                "--output" => {
                    result.output =
                        Some(args.next().context("--output requires a directory")?.into())
                }
                "install" => (),
                "uninstall" => result.uninstall = true,
                "--help" | "-h" => result.help = true,
                "--interactive" => result.interactive = true,
                "--validate" => result.validate = true,
                "--config" => {
                    result.config = Some(args.next().context("--config requires a path")?.into())
                }
                "--profile" => {
                    result.profile = Some(args.next().context("--profile requires a name")?)
                }
                "--remove-settings" | "--leave-settings" => {
                    ensure!(!result.settings_explicit, "Specify only one settings flag");
                    result.settings_explicit = true;
                    result.remove_settings = arg == "--remove-settings";
                }
                "--set" => {
                    let item = args.next().context("--set requires name=value")?;
                    let (name, value) =
                        item.split_once('=').context("--set requires name=value")?;
                    ensure!(
                        result
                            .values
                            .insert(name.into(), Value::String(value.into()))
                            .is_none(),
                        "Duplicate input: {name}"
                    );
                }
                "--install-dir" | "--bin-dir" => {
                    let value = args
                        .next()
                        .with_context(|| format!("{arg} requires a path"))?;
                    ensure!(
                        result
                            .values
                            .insert(arg[2..].into(), value.into())
                            .is_none(),
                        "Duplicate input: {arg}"
                    );
                }
                "--prune-old-versions"
                | "--desktop-shortcuts"
                | "--start-menu-shortcuts"
                | "--local-shortcuts" => {
                    result.values.insert(arg[2..].into(), true.into());
                }
                _ => bail!("Unknown argument: {arg}. Use --help."),
            }
        }
        ensure!(
            !result.interactive
                || (std::io::stdin().is_terminal() && std::io::stdout().is_terminal()),
            "--interactive requires a terminal"
        );
        Ok(result)
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
    let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .context("Home directory is unavailable")?;
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
    ensure!(
        package.schema == 1,
        "Unsupported schema: {}",
        package.schema
    );
    files::name(&package.application.id)?;
    files::name(&package.application.version)?;
    ensure!(
        !package.application.name.trim().is_empty(),
        "application.name is required"
    );
    ensure!(
        package.platform == platform() && package.architecture == architecture(),
        "Package targets {}-{}; engine is {}-{}",
        package.platform,
        package.architecture,
        platform(),
        architecture()
    );
    ensure!(
        !package.commands.is_empty() || !package.shortcuts.is_empty(),
        "At least one command or shortcut is required"
    );
    for command in &package.commands {
        files::name(&command.name)?;
        ensure!(
            !["app", "installer"].contains(&command.name.to_ascii_lowercase().as_str()),
            "Command name is reserved by the installation layout"
        );
        files::relative(&command.executable)?;
        ensure!(!command.name.ends_with(".exe"), "Command names omit .exe");
    }
    for shortcut in &package.shortcuts {
        ensure!(
            !shortcut.arguments.iter().any(|a| a.contains('\0')),
            "Shortcut arguments cannot contain NUL"
        );
        files::relative(&shortcut.executable)?;
        ensure!(
            !shortcut.name.is_empty()
                && !shortcut.name.contains([
                    '/', '\\', '\n', '\r', '\0', ':', '"', '*', '?', '<', '>', '|'
                ]),
            "Invalid shortcut name"
        );
        if let Some(icon) = &shortcut.icon {
            files::relative(icon)?;
        }
        if let Some(dir) = &shortcut.working_directory {
            files::relative(dir)?;
        }
    }
    Ok(())
}
fn standard_inputs() -> BTreeMap<String, Input> {
    let mut result = BTreeMap::new();
    for name in [
        "prune-old-versions",
        "desktop-shortcuts",
        "start-menu-shortcuts",
        "local-shortcuts",
    ] {
        result.insert(
            name.into(),
            Input {
                kind: "boolean".into(),
                required: true,
                prompt: Some(name.replace('-', " ")),
                default: Some(false.into()),
                choices: vec![],
            },
        );
    }
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
    result
}
pub fn resolve(package: &Package, options: &Options) -> Result<BTreeMap<String, Value>> {
    let mut inputs = standard_inputs();
    inputs.extend(package.inputs.clone());
    for key in package
        .values
        .keys()
        .chain(options.values.keys())
        .chain(package.defaults.values().flat_map(|v| v.keys()))
        .chain(package.profiles.values().flat_map(|p| p.values.keys()))
    {
        ensure!(inputs.contains_key(key), "Unknown input: {key}");
    }
    let home = builtins()?;
    let home_path = &home["home"];
    let local = &home["local-app-data"];
    let root = if cfg!(windows) {
        format!("{local}/Programs/{}", package.application.id)
    } else {
        format!("{home_path}/.local/lib/{}", package.application.id)
    };
    let bin = std::env::var("BIN")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                format!("{local}/bin")
            } else {
                format!("{home_path}/.local/bin")
            }
        });
    let mut values = BTreeMap::from([
        ("install-dir".into(), root.into()),
        ("bin-dir".into(), bin.into()),
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
                std::env::var("XDG_DATA_HOME")
                    .unwrap_or_else(|_| format!("{home_path}/.local/share"))
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
        if let Some(defaults) = package.defaults.get(layer) {
            values.extend(defaults.clone());
        }
    }
    let detected = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let matches: Vec<_> = package
        .profiles
        .iter()
        .filter(|(_, p)| {
            p.detect
                .iter()
                .any(|d| detected.split(':').any(|s| s.eq_ignore_ascii_case(d)))
        })
        .collect();
    let profile = if let Some(name) = &options.profile {
        Some(package.profiles.get(name).context("Unknown --profile")?)
    } else {
        ensure!(
            matches.len() <= 1,
            "Multiple environment profiles match; specify --profile"
        );
        matches.first().map(|(_, p)| *p)
    };
    if let Some(profile) = profile {
        values.extend(profile.values.clone());
    }
    values.extend(package.values.clone());
    values.extend(options.values.clone());
    for (key, input) in &inputs {
        ensure!(
            ["string", "path", "boolean", "choice"].contains(&input.kind.as_str()),
            "Invalid input type: {key}"
        );
        let allowed = match key.as_str() {
            "desktop-shortcuts" => package.shortcuts.iter().any(|s| s.desktop),
            "start-menu-shortcuts" => package.shortcuts.iter().any(|s| s.start_menu),
            "local-shortcuts" => package.shortcuts.iter().any(|s| s.local),
            _ => true,
        };
        if options.interactive
            && !options.values.contains_key(key)
            && allowed
            && let Some(prompt) = &input.prompt
        {
            let default = values.get(key).map(text).transpose()?.unwrap_or_default();
            print!("{prompt} [{}]: ", default);
            std::io::stdout().flush()?;
            let mut answer = String::new();
            ensure!(
                std::io::stdin().read_line(&mut answer)? > 0,
                "Input cancelled"
            );
            if !answer.trim().is_empty() {
                values.insert(key.clone(), answer.trim().into());
            }
        }
        if let Some(value) = values.get(key) {
            let raw = expand(&text(value)?, &values)?;
            ensure!(
                !input.required || !raw.trim().is_empty(),
                "Required input is empty: {key}"
            );
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
                allowed || converted != Value::Bool(true),
                "{key} is not allowed by installer.yaml"
            );
            values.insert(key.clone(), converted);
        } else {
            ensure!(
                !input.required,
                "Missing required input: {key}. Use --set {key}=VALUE"
            );
        }
    }
    Ok(values)
}
pub fn path(values: &BTreeMap<String, Value>, name: &str) -> Result<PathBuf> {
    let value = text(
        values
            .get(name)
            .with_context(|| format!("Missing required input: {name}"))?,
    )?;
    let path = Path::new(&value);
    ensure!(path.is_absolute(), "{name} must be absolute: {value}");
    files::absolute(path, &std::env::current_dir()?)
}
pub fn enabled(values: &BTreeMap<String, Value>, key: &str) -> bool {
    values.get(key) == Some(&Value::Bool(true))
}

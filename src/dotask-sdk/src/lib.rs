//! Helpers for one-file Rust tasks. The CLI supplies an immutable invocation
//! context; nested calls return through that CLI, including calls to C# tasks.
pub use anyhow::{anyhow, bail, Context, Result};
pub use serde_json::{self, json, Value};
pub use serde_saphyr;
pub use sha2;
use std::{
  fs,
  path::{Path, PathBuf},
  process::{Command, Stdio},
};
pub use tempfile;

#[derive(Debug)]
pub struct ProcessFailure(pub i32);
impl std::fmt::Display for ProcessFailure {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Process failed with exit code {}.", self.0)
  }
}
impl std::error::Error for ProcessFailure {}

/// Use this entry point so process failures retain their original exit codes.
pub fn run(task: fn(&BuildContext) -> Result<()>) {
  let result = BuildContext::load().and_then(|context| task(&context));
  if let Err(error) = result {
    eprintln!("dotask: {error:#}");
    std::process::exit(error.downcast_ref::<ProcessFailure>().map_or(1, |e| e.0));
  }
}

pub struct BuildContext {
  data: Value,
}
impl BuildContext {
  pub fn load() -> Result<Self> {
    let path = std::env::var_os("DOTASK_EXECUTION_CONTEXT").context("Run this task through dotask.")?;
    let data: Value = serde_json::from_slice(&fs::read(path)?)?;
    for key in [
      "RootDirectory",
      "TaskDirectory",
      "InvocationDirectory",
      "TargetFile",
      "TargetName",
      "CliExecutable",
      "SessionDirectory",
    ] {
      if data[key].as_str().is_none() {
        bail!("Invalid context: missing {key}.");
      }
    }
    Ok(Self { data })
  }
  pub fn root(&self) -> &Path {
    Path::new(self.data["RootDirectory"].as_str().unwrap())
  }
  pub fn task_file(&self) -> &Path {
    Path::new(self.data["TargetFile"].as_str().unwrap())
  }
  pub fn task_name(&self) -> &str {
    self.data["TargetName"].as_str().unwrap()
  }
  pub fn invocation(&self) -> &Path {
    Path::new(self.data["InvocationDirectory"].as_str().unwrap())
  }
  pub fn path(&self, path: impl AsRef<Path>) -> PathBuf {
    self.root().join(path)
  }
  pub fn parameters(&self) -> &Value {
    &self.data["Parameters"]
  }
  pub fn settings(&self) -> &Value {
    &self.data["Settings"]
  }
  pub fn parameter(&self, name: &str) -> Result<&Value> {
    lookup(self.parameters(), name).with_context(|| format!("Missing parameter '{name}'."))
  }
  pub fn string(&self, name: &str) -> Result<&str> {
    self
      .parameter(name)?
      .as_str()
      .with_context(|| format!("Parameter '{name}' must be a string."))
  }
  pub fn boolean(&self, name: &str) -> Result<bool> {
    self
      .parameter(name)?
      .as_bool()
      .with_context(|| format!("Parameter '{name}' must be a boolean."))
  }
  pub fn setting(&self, name: &str) -> Result<&Value> {
    lookup(self.settings(), name).with_context(|| format!("Missing setting '{name}'."))
  }
  pub fn setting_path(&self, name: &str) -> Result<PathBuf> {
    let value = self
      .setting(name)?
      .as_str()
      .filter(|s| !s.trim().is_empty())
      .with_context(|| format!("Setting '{name}' must name a directory."))?;
    Ok(self.path(value.replace(['/', '\\'], std::path::MAIN_SEPARATOR_STR)))
  }
  pub fn os(&self) -> &str {
    if cfg!(windows) {
      "windows"
    } else if cfg!(target_os = "macos") {
      "macos"
    } else {
      "linux"
    }
  }
  pub fn architecture(&self) -> &str {
    match std::env::consts::ARCH {
      "x86_64" => "x64",
      "aarch64" => "arm64",
      value => value,
    }
  }
  pub fn command(&self, executable: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(executable);
    command.current_dir(self.root());
    command
  }
  pub fn execute(&self, command: &mut Command) -> Result<()> {
    let status = command.status().context("Cannot start process")?;
    check(status.code().unwrap_or(130))
  }
  pub fn capture(&self, command: &mut Command) -> Result<String> {
    let output = command.stdin(Stdio::null()).output().context("Cannot start process")?;
    let stdout = String::from_utf8(output.stdout).context("Process stdout is not UTF-8")?;
    let stderr = String::from_utf8(output.stderr).context("Process stderr is not UTF-8")?;
    if !output.status.success() {
      eprint!("{stdout}{stderr}");
      check(output.status.code().unwrap_or(130))?;
    }
    if !stderr.trim().is_empty() {
      eprint!("{stderr}");
    }
    Ok(stdout)
  }
  pub fn exec_target(&self, target: &str, parameters: Value) -> Result<()> {
    self.call(target, parameters, 0).map(|_| ())
  }
  pub fn target_exists(&self, target: &str) -> Result<bool> {
    self.call(target, json!({}), 1)?["Exists"]
      .as_bool()
      .context("Invalid target lookup reply")
  }
  /// Absence is distinct from ambiguity, validation, compilation and task failure.
  pub fn exec_if_exists(&self, target: &str, parameters: Value) -> Result<Value> {
    self.call(target, parameters, 2)
  }
  pub fn create_installer(&self, target: &str, parameters: Value) -> Result<Value> {
    self.call(target, parameters, 3)
  }
  pub fn set_installer_result(&self, artifact: &Value) -> Result<()> {
    validate_installer(artifact, self)?;
    write_new(
      &Path::new(self.data["SessionDirectory"].as_str().unwrap()).join("installer-result.json"),
      artifact,
    )
  }
  pub fn installer_artifact(&self, file: &Path) -> Result<Value> {
    let artifact = json!({"FilePath":file,"Kind":0,"OS":match self.os() { "windows"=>0,"linux"=>1,"macos"=>2,_=>3 },"Architecture":match self.architecture() { "x64"=>1,"arm64"=>3,_=>-1 },"DefaultArguments":[]});
    validate_installer(&artifact, self)?;
    Ok(artifact)
  }
  /// Launch the returned installer with exact argument tokens and its own directory.
  pub fn run_installer(&self, artifact: &Value, arguments: Option<&[String]>) -> Result<()> {
    validate_installer(artifact, self)?;
    let file = Path::new(artifact["FilePath"].as_str().unwrap());
    let defaults: Vec<_> = artifact["DefaultArguments"]
      .as_array()
      .into_iter()
      .flatten()
      .map(|v| v.as_str().unwrap().to_owned())
      .collect();
    let arguments = arguments.unwrap_or(&defaults);
    if arguments.iter().any(|arg| arg.contains('\0')) {
      bail!("Installer arguments cannot contain NUL");
    }
    let mut command = match artifact["Kind"].as_i64().unwrap() {
      0 => self.command(file),
      1 => {
        let mut command = self.command("msiexec.exe");
        command.arg("/i").arg(file);
        command
      }
      2 => {
        let mut command = self.command("/bin/sh");
        command.arg(file);
        command
      }
      3 => {
        let mut command = self.command("dotnet");
        command.arg(file);
        command
      }
      _ => unreachable!(),
    };
    command.current_dir(file.parent().unwrap()).args(arguments);
    let code = command.status().context("Cannot start installer")?.code().unwrap_or(130);
    if artifact["Kind"] == 1 && matches!(code, 1641 | 3010) {
      println!("Installer succeeded; restart required ({code}).");
      return Ok(());
    }
    check(code)
  }
  fn call(&self, target: &str, parameters: Value, operation: u8) -> Result<Value> {
    if target.trim().is_empty() || !parameters.is_object() {
      bail!("A target and parameter mapping are required.");
    }
    let directory = tempfile::Builder::new()
      .prefix("call-")
      .tempdir_in(self.data["SessionDirectory"].as_str().unwrap())?;
    let mut context = self.data.clone();
    context["SessionDirectory"] = json!(directory.path());
    let request = directory.path().join("request.json");
    write_new(
      &request,
      &json!({"Context": context, "Target": target, "Parameters": parameters, "Operation": operation}),
    )?;
    self.execute(
      self
        .command(self.data["CliExecutable"].as_str().unwrap())
        .args(
          self.data["CliArguments"]
            .as_array()
            .context("Invalid CLI argument prefix")?
            .iter()
            .map(|v| v.as_str().unwrap()),
        )
        .arg("__exec")
        .arg(&request),
    )?;
    if operation == 0 {
      return Ok(Value::Null);
    }
    let reply = if operation == 3 {
      directory.path().join("installer-result.json")
    } else {
      directory.path().join("request.json.result")
    };
    let value = serde_json::from_slice(&fs::read(&reply).with_context(|| {
      if operation == 3 {
        format!("Target '{target}' did not return an installer")
      } else {
        format!("Target '{target}' returned no result")
      }
    })?)?;
    if operation == 3 {
      validate_installer(&value, self)?;
    }
    Ok(value)
  }
  /// Batch metadata remains an explicit, compiler-free CLI operation. Catalog
  /// authoring reads common YAML headers without any language toolchain.
  pub fn metadata(&self, root: &Path, files: &[PathBuf]) -> Result<Value> {
    let directory = tempfile::tempdir()?;
    let request = directory.path().join("request.json");
    let response = directory.path().join("response.json");
    write_new(&request, &json!({"TaskDirectory": root, "Files": files}))?;
    self.execute(
      self
        .command(self.data["CliExecutable"].as_str().unwrap())
        .arg("__metadata")
        .arg(request)
        .arg(&response),
    )?;
    Ok(serde_json::from_slice(&fs::read(response)?)?)
  }
}
fn lookup<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
  let mut current = value;
  for part in name.split('.') {
    current = current.as_object()?.iter().find(|(k, _)| k.eq_ignore_ascii_case(part))?.1;
  }
  Some(current)
}
fn check(code: i32) -> Result<()> {
  if code != 0 {
    return Err(ProcessFailure(code).into());
  }
  Ok(())
}
fn write_new(path: &Path, value: &Value) -> Result<()> {
  let mut options = fs::OpenOptions::new();
  options.write(true).create_new(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  serde_json::to_writer(options.open(path)?, value)?;
  Ok(())
}
fn validate_installer(value: &Value, context: &BuildContext) -> Result<()> {
  let file = Path::new(value["FilePath"].as_str().context("Missing installer FilePath")?);
  let os = match context.os() {
    "windows" => 0,
    "linux" => 1,
    "macos" => 2,
    _ => 3,
  };
  let arch = match context.architecture() {
    "x64" => 1,
    "arm64" => 3,
    _ => -1,
  };
  if !file.is_absolute()
    || !file.is_file()
    || value["OS"] != os
    || value["Architecture"] != arch
    || !matches!(value["Kind"].as_i64(), Some(0..=3))
  {
    bail!("Installer must be an existing absolute file for the current OS and architecture.");
  }
  if cfg!(windows) && value["Kind"] == 0 && !file.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
    bail!("Windows executable installers must be .exe files.");
  }
  if (value["Kind"] == 1 && (!cfg!(windows) || !file.extension().is_some_and(|e| e.eq_ignore_ascii_case("msi"))))
    || (value["Kind"] == 2 && cfg!(windows))
  {
    bail!("Installer kind is incompatible with this platform.");
  }
  if !value["DefaultArguments"].is_null()
    && !value["DefaultArguments"]
      .as_array()
      .is_some_and(|a| a.iter().all(|v| v.as_str().is_some_and(|s| !s.contains('\0'))))
  {
    bail!("Installer arguments must be strings without NUL characters.");
  }
  Ok(())
}

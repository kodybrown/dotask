use crate::process;
use anyhow::{bail, Context, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) struct Host {
  pub dotnet: OsString,
  assembly: PathBuf,
  apphost: Option<PathBuf>,
}

impl Host {
  pub fn locate() -> Result<Self> {
    let executable = std::env::current_exe()?;
    let assembly = executable
      .parent()
      .context("The CLI has no parent directory")?
      .join("csharp/Dotask.CSharpHost.dll");
    if !assembly.is_file() {
      bail!(
        "C# support is missing at {}. Build the complete preview with build.cmd rust-cli (or ./build.sh rust-cli).",
        assembly.display()
      );
    }
    let dotnet = std::env::var_os("DOTNET_HOST_PATH")
      .filter(|v| Path::new(v).is_file())
      .or_else(|| {
        std::env::var_os("DOTNET_ROOT")
          .map(|root| {
            PathBuf::from(root)
              .join(if cfg!(windows) {
                "dotnet.exe"
              } else {
                "dotnet"
              })
              .into_os_string()
          })
          .filter(|v| Path::new(v).is_file())
      })
      .unwrap_or_else(|| OsString::from("dotnet"));
    let apphost = assembly.parent().unwrap().join(if cfg!(windows) {
      "Dotask.CSharpHost.exe"
    } else {
      "Dotask.CSharpHost"
    });
    Ok(Self {
      dotnet,
      assembly,
      apphost: apphost.is_file().then_some(apphost),
    })
  }

  pub fn request<T: DeserializeOwned>(&self, directory: &Path, mut request: Value) -> Result<T> {
    process::check_cancelled()?;
    let exchange = tempfile::Builder::new().prefix("host-").tempdir_in(directory)?;
    let input = exchange.path().join("request.json");
    let output = exchange.path().join("response.json");
    let diagnostics = exchange.path().join("diagnostics.txt");
    request["Version"] = Value::from(2);
    write_json(&input, &request)?;
    let mut command = if let Some(apphost) = &self.apphost {
      Command::new(apphost)
    } else {
      let mut command = Command::new(&self.dotnet);
      command.arg(&self.assembly);
      command
    };
    let code = process::run(
      command
        .arg(&input)
        .arg(&output)
        .env_remove("DOTASK_EXECUTION_CONTEXT")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(File::create(&diagnostics)?),
    )?;
    if code != 0 {
      let diagnostic = std::fs::read_to_string(&diagnostics)?;
      bail!(
        "{}",
        if diagnostic.trim().is_empty() {
          format!("C# support host failed with exit code {code}.")
        } else {
          diagnostic.trim().trim_start_matches("dotask: ").to_owned()
        }
      );
    }
    let response: Value = serde_json::from_reader(File::open(&output).context("C# support returned no response")?)?;
    if response["Version"] != 2 {
      bail!("Unsupported C# support protocol version.");
    }
    Ok(serde_json::from_value(response["Result"].clone())?)
  }
}

pub(crate) fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
  let mut options = OpenOptions::new();
  options.write(true).create_new(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  serde_json::to_writer(options.open(path)?, value)?;
  Ok(())
}

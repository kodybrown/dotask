use dotask_sdk::{json, tempfile, BuildContext, Context, Result};
use std::path::{Path, PathBuf};

pub enum Tools {
  Source(tempfile::TempDir),
  Prebuilt(PathBuf),
}

impl Tools {
  pub fn prepare(project: &BuildContext) -> Result<Self> {
    let source = project.string("source").ok();
    let prebuilt = project.string("doinstall-tools").ok().filter(|path| !path.is_empty());
    let prebuilt = match (&source, prebuilt) {
      (Some(_), Some(_)) => dotask_sdk::bail!("Choose --source or --doinstall-tools, not both"),
      (Some(_), None) => None,
      (None, Some(path)) => Some(PathBuf::from(path)),
      (None, None) => std::env::var_os("DOINSTALL_TOOL_DIR").map(PathBuf::from),
    };
    let tools = if let Some(path) = prebuilt {
      Self::Prebuilt(project.path(path))
    } else {
      let source = match source {
        Some(path) => project.path(path),
        None => project
          .setting_path("doinstall.source")
          .context("Configure settings.doinstall.source or supply prebuilt tools with --doinstall-tools")?,
      };
      if !source.join("Cargo.toml").is_file() {
        dotask_sdk::bail!(
          "No DoInstall source checkout at {}. Set settings.doinstall.source, or supply prebuilt tools with --doinstall-tools.",
          source.display()
        );
      }
      println!("Building DoInstall tools from {}", source.display());
      let stage = tempfile::Builder::new().prefix("dotask-installer-tools-").tempdir()?;
      // The upstream build stages the tools together. Keep that snapshot alive
      // until all verification or packaging consumers finish.
      project.exec_target("_/doinstall/build", json!({"source":source,"stage-dir":stage.path()}))?;
      Self::Source(stage)
    };
    for name in ["doinstall", "doinstall-builder", "doinstall-launcher"] {
      let file = tools.path().join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
      if !file.is_file() {
        dotask_sdk::bail!("Required DoInstall tool is missing: {}", file.display());
      }
    }
    Ok(tools)
  }

  pub fn path(&self) -> &Path {
    match self {
      Self::Source(directory) => directory.path(),
      Self::Prebuilt(path) => path,
    }
  }
}

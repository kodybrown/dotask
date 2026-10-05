//! Delegate C# builds to the user's SDK with isolated MSBuild hooks. No managed
//! dispatcher, compiler assembly, SDK or runtime is bundled with the application.
use crate::{process, project::Target};
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::{
  fs::{self, File, OpenOptions},
  path::{Path, PathBuf},
  process::{Command, Stdio},
  time::Duration,
};

pub(crate) struct Host {
  pub dotnet: std::ffi::OsString,
  assembly: PathBuf,
}
impl Host {
  pub fn unavailable(root: &Path) -> Option<&'static str> {
    let configured = std::env::var_os("DOTNET_HOST_PATH").is_some_and(|p| Path::new(&p).is_file())
      || std::env::var_os("DOTNET_ROOT").is_some_and(|p| {
        Path::new(&p)
          .join(if cfg!(windows) {
            "dotnet.exe"
          } else {
            "dotnet"
          })
          .is_file()
      });
    (!configured && !crate::configuration::tool_exists("dotnet", root))
      .then_some("requires .NET SDK 10.0.300+ (the .NET 10 family)")
  }
  pub fn locate() -> Result<Self> {
    let assembly = std::env::current_exe()?
      .parent()
      .context("CLI has no parent")?
      .join("Dotask.Library.dll");
    if !assembly.is_file() {
      bail!(
        "C# task helpers are missing at {}. Stage the complete dotask application.",
        assembly.display()
      );
    }
    let dotnet = std::env::var_os("DOTNET_HOST_PATH")
      .filter(|p| Path::new(p).is_file())
      .or_else(|| {
        std::env::var_os("DOTNET_ROOT")
          .map(|p| {
            PathBuf::from(p)
              .join(if cfg!(windows) {
                "dotnet.exe"
              } else {
                "dotnet"
              })
              .into_os_string()
          })
          .filter(|p| Path::new(p).is_file())
      })
      .unwrap_or_else(|| "dotnet".into());
    Ok(Self { dotnet, assembly })
  }
  pub fn compile(&self, target: &Target, snapshot: &Path) -> Result<PathBuf> {
    let source = fs::canonicalize(&target.file_path)?;
    // MSBuild removes Windows extended-length prefixes from its project identity.
    let spelling = source.to_string_lossy();
    let spelling = spelling
      .strip_prefix(r"\\?\UNC\")
      .map(|s| format!(r"\\{s}"))
      .unwrap_or_else(|| spelling.strip_prefix(r"\\?\").unwrap_or(&spelling).to_owned());
    let source = PathBuf::from(spelling);
    let directory = source.parent().unwrap();
    let version = capture(
      Command::new(&self.dotnet).arg("--version").current_dir(directory),
      snapshot,
      "sdk",
    )
    .context(
      "C# tasks require a .NET SDK in the .NET 10 family, version 10.0.300 or later. Install/select the SDK and retry.",
    )?;
    let pieces: Vec<_> = version.trim().split(['.', '-']).collect();
    if pieces.len() < 3 || pieces[0] != "10" || pieces[1] != "0" || pieces[2].parse::<u32>().unwrap_or(0) < 300 {
      bail!(
        "C# tasks require .NET SDK 10.0.300+ in the .NET 10 family; selected {}.",
        version.trim()
      );
    }
    let user = hash(
      std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default()
        .as_bytes(),
    );
    let cache = std::env::temp_dir()
      .join("_dotnet/dotask")
      .join(&user[..24])
      .join(&hash(source.to_string_lossy().as_bytes())[..24]);
    fs::create_dir_all(&cache)?;
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      fs::set_permissions(cache.parent().unwrap(), fs::Permissions::from_mode(0o700))?;
    }
    let lock = OpenOptions::new()
      .create(true)
      .truncate(false)
      .read(true)
      .write(true)
      .open(cache.join("build.lock"))?;
    loop {
      match lock.try_lock() {
        Ok(()) => break,
        Err(std::fs::TryLockError::WouldBlock) => {
          process::check_cancelled()?;
          std::thread::sleep(Duration::from_millis(50));
        }
        Err(error) => return Err(error.into()),
      }
    }
    let props = cache.join("dotask.props");
    let targets = cache.join("dotask.targets");
    let record = cache.join("target-path.txt");
    let bootstrap = cache.join("dotask-bootstrap.cs");
    let entry = escape(&format!("{}.csproj", source.display()));
    let own = format!("'$(MSBuildProjectFullPath)' == '{entry}'");
    let other = format!("'$(MSBuildProjectFullPath)' != '{entry}'");
    // Replace only implicit hooks of the task's synthetic project. Explicit
    // project references retain their own build policy and SDK/package directives.
    write_changed(
      &props,
      &format!(
        r#"<Project>
<PropertyGroup Condition="{other}"><_DotaskOriginalProps>$([MSBuild]::GetPathOfFileAbove('Directory.Build.props', '$(MSBuildProjectDirectory)'))</_DotaskOriginalProps></PropertyGroup>
<Import Condition="{other} and '$(_DotaskOriginalProps)' != ''" Project="$(_DotaskOriginalProps)" />
<PropertyGroup Condition="{own}"><BaseOutputPath>{bin}</BaseOutputPath><BaseIntermediateOutputPath>{obj}</BaseIntermediateOutputPath><MSBuildProjectExtensionsPath>{obj}</MSBuildProjectExtensionsPath><UseAppHost>false</UseAppHost><PublishAot>false</PublishAot><ManagePackageVersionsCentrally>false</ManagePackageVersionsCentrally></PropertyGroup>
</Project>"#,
        bin = escape(&format!("{}{s}", cache.join("bin").display(), s = std::path::MAIN_SEPARATOR)),
        obj = escape(&format!("{}{s}", cache.join("obj").display(), s = std::path::MAIN_SEPARATOR))
      ),
    )?;
    write_changed(
      &targets,
      &format!(
        r#"<Project>
<PropertyGroup Condition="{other}"><_DotaskOriginalTargets>$([MSBuild]::GetPathOfFileAbove('Directory.Build.targets', '$(MSBuildProjectDirectory)'))</_DotaskOriginalTargets></PropertyGroup>
<Import Condition="{other} and '$(_DotaskOriginalTargets)' != ''" Project="$(_DotaskOriginalTargets)" />
<ItemGroup Condition="{own}"><Reference Include="Dotask.Library"><HintPath>{dll}</HintPath><Private>true</Private></Reference><Compile Include="{bootstrap}" /></ItemGroup>
<Target Name="DotaskRecordOutput" AfterTargets="Build" Condition="{own}"><WriteLinesToFile File="{record}" Lines="$(TargetPath)" Overwrite="true" /></Target>
</Project>"#,
        dll = escape(&self.assembly.display().to_string()),
        bootstrap = escape(&bootstrap.display().to_string()),
        record = escape(&record.display().to_string())
      ),
    )?;
    write_changed(&bootstrap, "internal static class __DotaskBootstrap { [System.Runtime.CompilerServices.ModuleInitializer] internal static void Initialize() => DoTask.Runtime.TargetRuntime.Initialize(); }\n")?;
    if record.exists() {
      fs::remove_file(&record)?;
    }
    let mut command = Command::new(&self.dotnet);
    command
      .current_dir(directory)
      .arg("build")
      .arg(&source)
      .args(["-c", "Release", "--nologo", "-v:quiet"])
      .arg(format!("-p:DirectoryBuildPropsPath={}", props.display()))
      .arg(format!("-p:DirectoryBuildTargetsPath={}", targets.display()));
    capture(&mut command, snapshot, "build").with_context(|| format!("Compilation failed for '{}'.", target.name))?;
    let assembly = PathBuf::from(fs::read_to_string(&record).context("Compiler returned no output path")?.trim());
    if !assembly.is_file() {
      bail!("Compiler output is missing: {}", assembly.display());
    }
    copy_output(assembly.parent().unwrap(), snapshot)?;
    Ok(snapshot.join(assembly.file_name().unwrap()))
  }
}
fn capture(command: &mut Command, directory: &Path, name: &str) -> Result<String> {
  let stdout = directory.join(format!("{name}.stdout"));
  let stderr = directory.join(format!("{name}.stderr"));
  let code = process::run(
    command
      .stdin(Stdio::null())
      .stdout(File::create(&stdout)?)
      .stderr(File::create(&stderr)?)
      .env_remove("DOTASK_EXECUTION_CONTEXT"),
  )?;
  let output = fs::read_to_string(stdout)?;
  let diagnostics = fs::read_to_string(stderr)?;
  if code != 0 {
    bail!("{}\n{}", output.trim(), diagnostics.trim());
  }
  if !diagnostics.trim().is_empty() {
    eprint!("{diagnostics}");
  }
  Ok(output)
}
fn hash(bytes: &[u8]) -> String {
  format!("{:x}", Sha256::digest(bytes))
}
fn escape(value: &str) -> String {
  let mut value = value.to_owned();
  for c in ['%', '$', '@', '\'', ';', '(', ')'] {
    value = value.replace(c, &format!("%{:02X}", c as u32));
  }
  value
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
}
fn write_changed(path: &Path, text: &str) -> Result<()> {
  if fs::read_to_string(path).ok().as_deref() != Some(text) {
    fs::write(path, text)?;
  }
  Ok(())
}
fn copy_output(source: &Path, destination: &Path) -> Result<()> {
  for entry in fs::read_dir(source)? {
    let entry = entry?;
    let output = destination.join(entry.file_name());
    if entry.file_type()?.is_dir() {
      fs::create_dir_all(&output)?;
      copy_output(&entry.path(), &output)?;
    } else {
      fs::copy(entry.path(), output)?;
    }
  }
  Ok(())
}

// The native acceptance uses only the delivered SDKs and the verified published
// tools. Clearing all source overrides is essential: a checkout must not mask a
// broken portable distribution. Local source-development builds remain explicit.
fn pin() -> Result<Value> {
  Ok(serde_json::from_str(include_str!("../doinstall.json"))?)
}
fn platform() -> Result<&'static str> {
  ensure!(
    cfg!(target_arch = "x86_64") && (cfg!(windows) || cfg!(target_os = "linux")),
    "Native Windows/Linux x64 required"
  );
  Ok(std::env::consts::OS)
}
fn fresh(path: &Path) -> Result<PathBuf> {
  let path = destination(path)?;
  let root = fs::canonicalize(super::root())?;
  ensure!(
    !path.starts_with(&root) && !root.starts_with(&path) && !path.exists(),
    "Output must be fresh and outside source"
  );
  fs::create_dir_all(path.parent().context("Missing output parent")?)?;
  Ok(path)
}
fn extract(entries: Vec<Entry>, output: &Path) -> Result<()> {
  validate_entries(&entries)?;
  fs::create_dir(output)?;
  for (name, bytes, mode) in entries {
    let path = output.join(name);
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(&path, bytes)?;
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      fs::set_permissions(&path, fs::Permissions::from_mode(mode & 0o777))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
  }
  Ok(())
}
pub fn prepare(output: &Path) -> Result<()> {
  let system = platform()?;
  let output = fresh(output)?;
  fs::create_dir(&output)?;
  let pin = pin()?;
  let name = pin[system]["archive"].as_str().context("Missing archive pin")?;
  let archive = output.join(name);
  let url = format!(
    "https://github.com/kodybrown/doinstall/releases/download/{}/{name}",
    pin["version"].as_str().unwrap()
  );
  run(
    Command::new(if cfg!(windows) { "curl.exe" } else { "curl" })
      .args([
        "--fail",
        "--location",
        "--retry",
        "3",
        "--proto",
        "=https",
        "--tlsv1.2",
        "--output",
      ])
      .arg(&archive)
      .arg(url),
  )?;
  ensure!(
    sha(&fs::read(&archive)?) == pin[system]["sha256"],
    "Pinned doinstall archive checksum mismatch"
  );
  extract(archive_entries(&archive)?, &output.join("tools"))?;
  ensure!(
    tools_identity(&output.join("tools"))?["published"] == true,
    "Prepared tools failed provenance validation"
  );
  println!("Verified {} {} tools", pin["version"], system);
  Ok(())
}
fn tools_identity(tools: &Path) -> Result<Value> {
  let pin = pin()?;
  let system = platform()?;
  let archive = tools
    .parent()
    .context("Missing tools parent")?
    .join(pin[system]["archive"].as_str().unwrap());
  let files = inventory(tools)?;
  if archive.is_file() {
    ensure!(
      sha(&fs::read(&archive)?) == pin[system]["sha256"],
      "Pinned doinstall checksum mismatch"
    );
    let expected: Inventory = archive_entries(&archive)?.into_iter().map(|(n, b, _)| (n, sha(&b))).collect();
    ensure!(files == expected, "Prebuilt tools differ from the verified published archive");
    let info: Value = serde_json::from_slice(&fs::read(tools.join("RELEASE.json"))?)?;
    ensure!(
      info["version"]
        == pin["version"]
          .as_str()
          .context("Missing pinned version")?
          .strip_prefix('v')
          .context("Pinned version requires v prefix")?
        && info["source_commit"] == pin["source_commit"]
        && info["platform"] == system
        && info["architecture"] == "x64",
      "Published doinstall identity mismatch"
    );
    // Even a previously prepared tools directory must be checked before a
    // version/discovery probe can execute one of its binaries.
    query_tools(tools)?;
    return Ok(
      json!({"published":true,"release":pin["version"],"source_commit":pin["source_commit"],"archive":pin[system]["archive"],"sha256":pin[system]["sha256"]}),
    );
  }
  // Explicit --source and existing local prebuilt tools can package a development
  // delivery, but the collector never publishes one as a pinned release.
  query_tools(tools)?;
  Ok(
    json!({"published":false,"identity":capture(Command::new(tools.join(super::executable("doinstall"))).arg("--version"))?,"files":files}),
  )
}
fn assemble(stage: &Path, tools: &Path, work: &Path, version: &str) -> Result<PathBuf> {
  let mut config: Value = serde_saphyr::from_str(include_str!("../../installer.yaml"))?;
  config.as_object_mut().unwrap().remove("build-steps");
  config["application"]["version"] = version.into();
  config["platform"] = platform()?.into();
  config["architecture"] = "x64".into();
  config["payload"] = stage.to_string_lossy().into_owned().into();
  config["runtime"]["commands"][0]["executable"] = super::executable("dotask").into();
  let text = serde_saphyr::to_string(&config)?.replace("${exe-extension}", std::env::consts::EXE_SUFFIX);
  fs::write(work.join("installer.yaml"), text)?;
  fs::create_dir(work.join(".tasks"))?;
  fs::write(
    work.join(".tasks/assemble.task"),
    include_bytes!("../../_/doinstall/assemble-self-extracting.task"),
  )?;
  run(
    clean_command(&stage.join(super::executable("dotask")), work, &work.join("cache"))
      .arg("assemble")
      .arg("--builder")
      .arg(tools.join(super::executable("doinstall-builder")))
      .arg("--config")
      .arg(work.join("installer.yaml"))
      .arg("--installer")
      .arg(tools.join(super::executable("doinstall")))
      .arg("--launcher-template")
      .arg(tools.join(super::executable("doinstall-launcher")))
      .arg("--output")
      .arg(work.join("delivery"))
      .arg("--result-file")
      .arg(work.join("result.json")),
  )?;
  let result: Value = serde_json::from_slice(&fs::read(work.join("result.json"))?)?;
  let path = PathBuf::from(result["FilePath"].as_str().context("Missing actual builder FilePath")?);
  ensure!(path.is_absolute() && path.is_file(), "Invalid builder result");
  Ok(path)
}
fn clean_command(binary: &Path, project: &Path, cache: &Path) -> Command {
  let mut command = Command::new(binary);
  command.current_dir(project).env("DOTASK_CACHE_HOME", cache);
  for key in [
    "DOTASK_SOURCE_ROOT",
    "DOTASK_SOURCE_RUST_SDK",
    "DOTASK_RUST_SDK",
    "DOTASK_SDK_PATH",
    "DOTASK_SOURCE_SDK",
    "DOTASK_SOURCE_SDK_STAGE",
    "DOTASK_EXECUTION_CONTEXT",
  ] {
    command.env_remove(key);
  }
  command
}
fn consumer(binary: &Path, tools: &Path, work: &Path) -> Result<()> {
  fs::create_dir(work)?;
  let tasks = work.join(".tasks");
  fs::create_dir(&tasks)?;
  // Language tasks still use the shipped SDKs; installer creation below must not.
  fs::write(tasks.join("rust.rs"), "// dotask: 1\n// description: Released Rust SDK acceptance.\n// end-dotask\nuse dotask_sdk::{BuildContext, Result};\nfn main(){dotask_sdk::run(|_: &BuildContext| -> Result<()> { let _ = dotask_sdk::json!({\"sdk\":\"rust\"}); println!(\"released-rust-sdk\"); Ok(()) });}\n")?;
  fs::write(tasks.join("csharp.cs"), "// dotask: 1\n// description: Released CSharp SDK acceptance.\n// end-dotask\nusing DoTask;\nConsole.WriteLine(\"released-csharp-sdk\");\n")?;
  fs::write(tasks.join("go.go"), "package main\nimport(\"fmt\"; dotask \"dotask.local/sdk\")\nfunc main(){dotask.Run(func(c *dotask.BuildContext)error{fmt.Println(\"released-go-sdk\");return nil})}\n")?;
  fs::write(
    tasks.join("all.task"),
    "description: Released task composition.\nsteps:\n  - run: rust\n  - run: csharp\n  - run: go\n",
  )?;
  let output = capture(clean_command(binary, work, &work.join("cache")).arg("all"))?;
  ensure!(
    output.contains("released-rust-sdk")
      && output.contains("released-csharp-sdk")
      && output.contains("released-go-sdk"),
    "Delivered SDK task execution failed"
  );
  // Exercise the shipped creator from both a portable binary and an installed
  // command shim. Only prebuilt doinstall tools remain on PATH: no SDK/compiler.
  fs::create_dir_all(tasks.join("_/doinstall"))?;
  fs::write(
    tasks.join("_/doinstall/create-installer.task"),
    include_bytes!("../../_/doinstall/create-installer.task"),
  )?;
  fs::write(
    tasks.join("_/doinstall/assemble-self-extracting.task"),
    include_bytes!("../../_/doinstall/assemble-self-extracting.task"),
  )?;
  fs::write(tasks.join("package.task"), "steps:\n  - run: _/doinstall/create-installer\n")?;
  fs::write(tasks.join("prepare.task"), "options: [{name: app-version, required: true}]\nsteps:\n  - file: {operation: write, path: prepared-version.txt, content: {parameter: app-version}}\n")?;
  let payload = work.join("payload");
  fs::create_dir(&payload)?;
  fs::write(payload.join("README.txt"), "Downstream packaging acceptance\n")?;
  let example = super::executable("example");
  fs::copy(tools.join(super::executable("doinstall")), payload.join(&example))?;
  write_json(
    &work.join(".dotasks.yaml"),
    &json!({"version":1,"settings":{"installer":{"config":"source.yaml","output":"package"},"app":{"git-hash":"none","dirty-suffix":false}}}),
  )?;
  write_json(
    &work.join("source.yaml"),
    &json!({
      "schema":1,"application":{"id":"dotask-release-smoke","name":"Smoke","version":"1.0.0"},
      "build-steps":[
        {"run":"prepare","with":{"app-version":"${version}"}},
        {"gather":[{"from":payload,"files":["README.txt", example]}]},
        {"run":"_/doinstall/assemble-self-extracting","with":{"builder":"${builder}","installer":"${installer}","launcher-template":"${launcher-template}","config":"${config}","output":"${package}","result-file":"${result-file}"}}
      ],
      "runtime":{"interactive":false,"commands":[{"name":"release-smoke","executable":example}]}
    }),
  )?;
  run(
    clean_command(binary, work, &work.join("compiler-free-cache"))
      .env("PATH", tools)
      .env("DOTNET_HOST_PATH", "missing-dotnet")
      .env("DOTASK_RUST_SDK", "missing-rust-sdk")
      .env("DOTASK_SDK_PATH", "missing-dotnet-sdk")
      .arg("package"),
  )?;
  ensure!(
    fs::read_to_string(work.join("prepared-version.txt"))? == "1.0.0",
    "Nested declarative task did not receive the creator context"
  );
  ensure!(
    !work.join("compiler-free-cache/rust").exists(),
    "Creator compiled a source task"
  );
  let record: Value = serde_json::from_slice(&fs::read(
    work.join(format!("artifacts/.dotask/installers/{}-x64.json", platform()?)),
  )?)?;
  let result = &record["Installer"];
  run(
    Command::new(result["FilePath"].as_str().context("Missing downstream FilePath")?)
      .arg("--extract-to")
      .arg(work.join("validated"))
      .args(["--", "--validate"]),
  )?;
  let install = work.join("installed");
  run(
    Command::new(result["FilePath"].as_str().context("Missing downstream FilePath")?)
      .arg("--extract-to")
      .arg(work.join("retained"))
      .args(["--", "--non-interactive", "--install-dir"])
      .arg(&install)
      .arg("--bin-dir")
      .arg(work.join("commands"))
      .args(["--set", "add-to-path=false"]),
  )?;
  run(Command::new(work.join("commands").join(super::executable("release-smoke"))).arg("--version"))?;
  run(
    Command::new(install.join("installer").join(super::executable("installer")))
      .args(["uninstall", "--non-interactive"]),
  )?;
  ensure!(!install.exists(), "Downstream uninstall left an installation");
  Ok(())
}
fn accept(
  archive: &Path,
  installer: &Path,
  tools: &Path,
  expected: &Inventory,
  markers: &BTreeSet<String>,
) -> Result<()> {
  let original_environment = persistent_environment()?;
  let entries = archive_entries(archive)?;
  ensure!(
    entries.iter().map(|(n, b, _)| (n.clone(), sha(b))).collect::<Inventory>() == *expected,
    "Portable inventory mismatch"
  );
  for (name, bytes, _) in &entries {
    scan(bytes, name, markers)?;
  }
  let temporary = tempfile::tempdir()?;
  let work = fs::canonicalize(temporary.path())?;
  let portable = work.join("portable distribution");
  extract(entries, &portable)?;
  consumer(
    &portable.join(super::executable("dotask")),
    tools,
    &work.join("portable consumer"),
  )?;
  inspect_installer(installer, expected, markers)?;
  let install = work.join("installed dotask");
  let commands = work.join("commands");
  run(
    Command::new(installer)
      .arg("--extract-to")
      .arg(work.join("retained source"))
      .args(["--", "--non-interactive", "--install-dir"])
      .arg(&install)
      .arg("--bin-dir")
      .arg(&commands)
      .args(["--set", "add-to-path=false"]),
  )?;
  let installed: Vec<_> = fs::read_dir(install.join("app"))?.collect::<std::io::Result<Vec<_>>>()?;
  ensure!(installed.len() == 1, "Unexpected installed build inventory");
  let payload = installed[0].path();
  for (name, hash) in expected {
    ensure!(sha(&fs::read(payload.join(name))?) == *hash, "Installed payload mismatch");
  }
  consumer(
    &commands.join(super::executable("dotask")),
    tools,
    &work.join("installed consumer"),
  )?;
  run(
    Command::new(install.join("installer").join(super::executable("installer")))
      .args(["uninstall", "--non-interactive"]),
  )?;
  ensure!(
    !install.exists() && !commands.join(super::executable("dotask")).exists(),
    "Temporary dotask uninstall incomplete"
  );
  ensure!(
    persistent_environment()? == original_environment,
    "Acceptance changed persistent PATH or shell profiles"
  );
  Ok(())
}
fn persistent_environment() -> Result<Vec<String>> {
  let mut result = vec![];
  if cfg!(windows) {
    for key in [
      r"HKCU\Environment",
      r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
    ] {
      let output = Command::new("reg.exe").args(["query", key, "/v", "Path"]).output()?;
      result.push(format!("{:?}:{}", output.status.code(), sha(&output.stdout)));
    }
  } else if let Some(home) = std::env::var_os("HOME") {
    for name in [".profile", ".bashrc", ".zshrc", ".config/fish/config.fish"] {
      let path = PathBuf::from(&home).join(name);
      result.push(if path.exists() {
        sha(&fs::read(path)?)
      } else {
        "absent".into()
      });
    }
  }
  Ok(result)
}
pub fn build(output: &Path, source: &Path, tools: &Path, result_file: Option<&Path>) -> Result<()> {
  let system = platform()?;
  let value = version(None)?;
  let output = fresh(output)?;
  if cfg!(target_os = "linux") {
    let release = fs::read_to_string("/etc/os-release")?;
    ensure!(
      release.lines().any(|v| v == "ID=ubuntu")
        && release.lines().any(|v| v == "VERSION_ID=\"22.04\"")
        && capture(Command::new("getconf").arg("GNU_LIBC_VERSION"))? == "glibc 2.35",
      "Linux release requires Ubuntu 22.04 / glibc 2.35"
    );
  }
  let rustc = capture(Command::new("rustc").args(["--verbose", "--version"]))?;
  ensure!(rustc.lines().any(|v| v == "release: 1.95.0"), "Wrong pinned Rust compiler");
  let dotnet = capture(Command::new("dotnet").arg("--version"))?;
  ensure!(dotnet == "10.0.402", "Wrong pinned .NET SDK");
  for key in ["target_directory", "build_directory"] {
    if let Some(path) = super::metadata()?[key].as_str() {
      ensure!(
        !destination(Path::new(path))?.starts_with(fs::canonicalize(super::root())?),
        "Cargo output inside source"
      );
    }
  }
  let dependency = tools_identity(tools)?;
  let temporary = tempfile::tempdir()?;
  let work = fs::canonicalize(temporary.path())?;
  let stage = work.join("payload");
  let names = [
    super::executable("dotask"),
    super::executable("dotask-doinstall-create-installer"),
    "sdk/rust/Cargo.toml".into(),
    "sdk/rust/src/lib.rs".into(),
    "sdk/rust/src/installers.rs".into(),
    "sdk/go/go.mod".into(),
    "sdk/go/context.go".into(),
    "sdk/go/process.go".into(),
    "sdk/go/tasks.go".into(),
    "sdk/go/installers.go".into(),
    "sdk/dotnet/Dotask.dotnet.dll".into(),
    "LICENSE.md".into(),
  ];
  ensure!(
    inventory(source)?.keys().cloned().collect::<BTreeSet<_>>() == names.iter().cloned().collect(),
    "Unexpected staged SDK/application inventory"
  );
  fs::create_dir(&stage)?;
  for name in names {
    let destination = stage.join(&name);
    fs::create_dir_all(destination.parent().unwrap())?;
    fs::copy(source.join(name), destination)?;
  }
  fs::write(stage.join("README.md"), include_str!("../PORTABLE-README.md"))?;
  licenses(&stage)?;
  if dependency["published"] == true {
    // The setup also redistributes doinstall's runtime/launcher. Carry the
    // published tool bundle's notices, without adding its binaries to our app.
    let prefix = format!("LICENSES/doinstall-{}/", dependency["release"].as_str().unwrap());
    for name in inventory(tools)?
      .keys()
      .filter(|n| *n == "LICENSE.md" || *n == "THIRD-PARTY.json" || n.starts_with("LICENSES/"))
    {
      let destination = stage.join(&prefix).join(name);
      fs::create_dir_all(destination.parent().unwrap())?;
      fs::copy(tools.join(name), destination)?;
    }
  }
  let identity: Value = serde_json::from_str(&capture(
    Command::new(stage.join(super::executable("dotask"))).arg("__build-info"),
  )?)?;
  let commit = capture(Command::new("git").current_dir(super::root()).args(["rev-parse", "HEAD"]))?;
  ensure!(
    identity["version"] == value && identity["commit"] == commit,
    "Staged executable differs from the current source identity"
  );
  let metadata = json!({"schema":1,"version":value,"tag":format!("v{value}"),"display_version":identity["display_version"],"built_utc":identity["built_utc"],"source_commit":commit,"source_dirty":identity["dirty"] == true || !capture(Command::new("git").current_dir(super::root()).args(["status","--porcelain"] ))?.is_empty(),"rustc":rustc,"cargo":capture(Command::new("cargo").arg("--version"))?,"dotnet":dotnet,"doinstall":dependency,"platform":system,"architecture":"x64","target":if cfg!(windows){"x86_64-pc-windows-msvc"}else{"x86_64-unknown-linux-gnu"},"compatibility_baseline":if cfg!(windows){"Windows Server 2022 x64"}else{"Ubuntu 22.04 x64 / glibc 2.35"},"files":inventory(&stage)?});
  write_json(&stage.join("RELEASE.json"), &metadata)?;
  let mut markers = markers()?;
  markers.extend([source, tools, &work, &output].map(|p| p.to_string_lossy().into_owned()));
  for (name, _) in inventory(&stage)? {
    scan(&fs::read(stage.join(&name))?, &name, &markers)?;
  }
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    ensure!(
      fs::metadata(stage.join("dotask"))?.permissions().mode() & 0o111 != 0,
      "Missing executable permission"
    );
    let glibc_version = Regex::new(r"GLIBC_(\d+)\.(\d+)")?;
    for executable in ["dotask", "dotask-doinstall-create-installer"] {
      ensure!(
        fs::metadata(stage.join(executable))?.permissions().mode() & 0o111 != 0,
        "Missing helper executable permission"
      );
      let bytes = fs::read(stage.join(executable))?;
      let text = String::from_utf8_lossy(&bytes);
      let versions: Vec<(u32, u32)> = glibc_version
        .captures_iter(&text)
        .map(|c| (c[1].parse().unwrap(), c[2].parse().unwrap()))
        .collect();
      ensure!(
        !versions.is_empty() && versions.iter().all(|v| *v <= (2, 35)),
        "Executable exceeds glibc 2.35 baseline"
      );
    }
  }
  let expected = inventory(&stage)?;
  let artifact = assemble(&stage, tools, &work, &value)?;
  fs::create_dir(&output)?;
  let stem = format!("dotask-v{value}-{system}-x64");
  let installer = output.join(format!("{stem}-setup{}", std::env::consts::EXE_SUFFIX));
  fs::copy(artifact, &installer)?;
  let archive = output.join(format!("{stem}-portable.{}", if cfg!(windows) { "zip" } else { "tar.gz" }));
  archive_tools(&stage, &archive, cfg!(windows))?;
  accept(&archive, &installer, tools, &expected, &markers)?;
  write_json(&output.join(format!("{stem}-metadata.json")), &metadata)?;
  sums(&output)?;
  if let Some(path) = result_file {
    write_json(path, &json!({"FilePath":installer}))?;
  }
  println!("Native dotask release acceptance passed: {}", output.display());
  Ok(())
}

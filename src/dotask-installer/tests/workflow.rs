use serde_json::{json, Value};
use std::{
  fs,
  path::PathBuf,
  process::{Command, Output},
};

struct Fixture {
  _temp: tempfile::TempDir,
  root: PathBuf,
  config: PathBuf,
  payload: PathBuf,
  bin: PathBuf,
}
impl Fixture {
  fn new() -> Self {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("installed app 日本語");
    let bin = temp.path().join("commands");
    let payload = temp.path().join("payload");
    fs::create_dir(&payload).unwrap();
    let executable = if cfg!(windows) { "probe.exe" } else { "probe" };
    // Use a real executable to exercise native shims and argument forwarding.
    fs::copy(env!("CARGO_BIN_EXE_dotask-installer"), payload.join(executable)).unwrap();
    let config = temp.path().join("probe.yaml");
    let package = json!({ "schema":1, "application":{"id":"probe","name":"Probe","version":"1.0.0"},
        "platform":if cfg!(windows){"windows"}else if cfg!(target_os="macos"){"macos"}else{"linux"},
        "architecture":if cfg!(target_arch="aarch64"){"arm64"}else{"x64"}, "payload":"payload",
        "commands":[{"name":"probe","executable":executable}],
        "shortcuts":[{"name":"Probe","executable":executable,"terminal":true,"local":true,"desktop":true,"start-menu":true}],
        "values":{"install-dir":root,"bin-dir":bin,"additional-command":true,"add-to-path":false},
        "settings":[temp.path().join("settings").to_str().unwrap()]
    });
    fs::write(&config, serde_saphyr::to_string(&package).unwrap()).unwrap();
    Self {
      _temp: temp,
      root,
      config,
      payload,
      bin,
    }
  }
  fn run(&self, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dotask-installer"));
    if !args.iter().any(|arg| ["--interactive", "--non-interactive"].contains(arg)) {
      command.arg("--non-interactive");
    }
    command.arg("--config").arg(&self.config).args(args).output().unwrap()
  }
  fn good(&self, args: &[&str]) {
    let result = self.run(args);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
  }
  fn receipt(&self) -> Value {
    serde_saphyr::from_str(&fs::read_to_string(self.root.join("installer/installation.yaml")).unwrap()).unwrap()
  }
  fn update(&self, version: &str) {
    let mut config: Value = serde_saphyr::from_str(&fs::read_to_string(&self.config).unwrap()).unwrap();
    config["application"]["version"] = version.into();
    fs::write(&self.config, serde_saphyr::to_string(&config).unwrap()).unwrap();
  }
  fn configure(&self, change: impl FnOnce(&mut Value)) {
    let mut config: Value = serde_saphyr::from_str(&fs::read_to_string(&self.config).unwrap()).unwrap();
    change(&mut config);
    fs::write(&self.config, serde_saphyr::to_string(&config).unwrap()).unwrap();
  }
  fn uninstall(&self, args: &[&str]) -> Output {
    let mut all = vec!["uninstall", "--install-dir", self.root.to_str().unwrap()];
    all.extend(args);
    self.run(&all)
  }
}
#[test]
fn install_repeat_update_prune_and_uninstall_preserve_settings() {
  let f = Fixture::new();
  let settings = f._temp.path().join("settings");
  fs::create_dir(&settings).unwrap();
  fs::write(settings.join("user.yaml"), "keep").unwrap();
  f.good(&[]);
  let first = f.receipt()["active"].clone();
  f.good(&[]);
  assert_eq!(f.receipt()["previous"], Value::Null);
  f.update("2.0.0");
  f.good(&[]);
  assert_eq!(f.receipt()["previous"], first);
  let second = f.receipt()["active"].clone();
  f.update("3.0.0");
  f.good(&["--prune-old-versions"]);
  assert_eq!(f.receipt()["previous"], second);
  assert_eq!(f.receipt()["builds"].as_object().unwrap().len(), 2);
  let command = f.root.join(if cfg!(windows) { "probe.exe" } else { "probe" });
  assert!(Command::new(command).arg("--help").output().unwrap().status.success());
  let result = f.uninstall(&[]);
  assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
  assert!(!f.root.exists());
  assert!(settings.join("user.yaml").exists());
}
#[test]
fn validate_and_bad_inputs_do_not_install() {
  let f = Fixture::new();
  f.good(&["--validate"]);
  assert!(!f.root.exists());
  for args in [
    &["--set", "unknown=x"][..],
    &["--set", "prune-old-versions=maybe"],
    &["--rollback"],
    &["--interactive"],
  ] {
    assert!(!f.run(args).status.success());
    assert!(!f.root.exists());
  }
}
#[cfg(windows)]
#[test]
fn retained_uninstaller_launched_through_junction_removes_owned_files() {
  let f = Fixture::new();
  f.good(&[]);
  let alias = f._temp.path().join("junction alias");
  let linked = Command::new("cmd.exe")
    .args(["/d", "/c", "mklink", "/J"])
    .arg(&alias)
    .arg(&f.root)
    .output()
    .unwrap();
  assert!(linked.status.success(), "{linked:?}");
  // Windows may report current_exe with the invocation's junction spelling,
  // while the installer canonicalizes its root. Both identify the same file.
  let result = Command::new(alias.join("installer/installer.exe"))
    .arg("uninstall")
    .arg("--non-interactive")
    .output()
    .unwrap();
  fs::remove_dir(&alias).unwrap();
  assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
  assert!(!f.root.exists());
  assert!(!f.bin.join("probe.exe").exists());
}
#[test]
fn modified_owned_files_and_unowned_commands_are_protected() {
  let f = Fixture::new();
  fs::create_dir(&f.bin).unwrap();
  let launcher = f.bin.join(if cfg!(windows) { "probe.exe" } else { "probe" });
  fs::write(&launcher, "unowned").unwrap();
  assert!(!f.run(&[]).status.success());
  assert!(!f.root.exists());
  assert_eq!(fs::read_to_string(&launcher).unwrap(), "unowned");
  fs::remove_file(launcher).unwrap();
  f.good(&[]);
  let active = f.receipt()["active"].as_str().unwrap().to_string();
  let executable = f
    .root
    .join("app")
    .join(active)
    .join(if cfg!(windows) { "probe.exe" } else { "probe" });
  fs::write(&executable, "modified").unwrap();
  assert!(!f.run(&[]).status.success());
  assert!(!f.uninstall(&[]).status.success());
  assert!(f.root.join("installer/installation.yaml").exists());
}
#[test]
fn uninstall_removes_settings_only_when_requested() {
  let f = Fixture::new();
  f.good(&[]);
  let settings = f._temp.path().join("settings");
  fs::create_dir(&settings).unwrap();
  fs::write(settings.join("preferences"), "remove").unwrap();
  let result = f.uninstall(&["--remove-settings"]);
  assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
  assert!(!settings.exists());
}
#[test]
fn zip_payload_preserves_permissions_and_refuses_traversal() {
  use std::io::Write;
  let f = Fixture::new();
  let archive = f._temp.path().join("payload.zip");
  let exe = if cfg!(windows) { "probe.exe" } else { "probe" };
  let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
  zip
    .start_file(exe, zip::write::SimpleFileOptions::default().unix_permissions(0o755))
    .unwrap();
  zip.write_all(&fs::read(f.payload.join(exe)).unwrap()).unwrap();
  zip.finish().unwrap();
  let mut package: Value = serde_saphyr::from_str(&fs::read_to_string(&f.config).unwrap()).unwrap();
  package["payload"] = "payload.zip".into();
  fs::write(&f.config, serde_saphyr::to_string(&package).unwrap()).unwrap();
  f.good(&[]);
  assert!(f.uninstall(&[]).status.success());
  let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
  zip.start_file("../escape", zip::write::SimpleFileOptions::default()).unwrap();
  zip.write_all(b"bad").unwrap();
  zip.finish().unwrap();
  assert!(!f.run(&[]).status.success());
  assert!(!f._temp.path().join("escape").exists());
}
#[test]
fn local_and_external_shortcuts_are_independent() {
  let f = Fixture::new();
  let desktop = f._temp.path().join("desktop");
  let arg = format!("desktop-dir={}", desktop.display());
  f.good(&["--local-shortcuts", "--desktop-shortcuts", "--set", &arg]);
  let extension = if cfg!(windows) {
    "lnk"
  } else if cfg!(target_os = "macos") {
    "command"
  } else {
    "desktop"
  };
  assert!(f.root.join(format!("Probe.{extension}")).exists());
  assert!(desktop.join(format!("Probe.{extension}")).exists());
  assert!(f.uninstall(&[]).status.success());
  assert!(!desktop.join(format!("Probe.{extension}")).exists());
}
#[test]
fn payload_links_are_rejected() {
  let f = Fixture::new();
  let link = f.payload.join("redirect");
  #[cfg(windows)]
  {
    if std::os::windows::fs::symlink_file(&f.config, &link).is_err() {
      return;
    }
  }
  #[cfg(unix)]
  std::os::unix::fs::symlink(&f.config, &link).unwrap();
  assert!(!f.run(&[]).status.success());
  assert!(!f.root.exists());
}
#[test]
fn retained_engine_uninstalls_without_original_package() {
  let f = Fixture::new();
  f.good(&[]);
  fs::remove_file(&f.config).unwrap();
  fs::remove_dir_all(&f.payload).unwrap();
  let engine = f.root.join("installer").join(if cfg!(windows) {
    "installer.exe"
  } else {
    "installer"
  });
  let output = Command::new(engine).args(["uninstall", "--non-interactive"]).output().unwrap();
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  assert!(!f.root.exists());
}

#[test]
fn inputs_require_values_and_follow_explicit_precedence() {
  let f = Fixture::new();
  let mut package: Value = serde_saphyr::from_str(&fs::read_to_string(&f.config).unwrap()).unwrap();
  package["inputs"] = json!({"channel":{"type":"choice","required":true,"choices":["stable","preview"]}});
  fs::write(&f.config, serde_saphyr::to_string(&package).unwrap()).unwrap();
  assert!(!f.run(&[]).status.success());
  assert!(!f.root.exists());
  package["defaults"] = json!({"common":{"channel":"preview"}});
  package["profiles"] = json!({"test":{"values":{"channel":"stable"}}});
  package["values"]["channel"] = "preview".into();
  fs::write(&f.config, serde_saphyr::to_string(&package).unwrap()).unwrap();
  f.good(&["--profile", "test"]);
  assert_eq!(f.receipt()["values"]["channel"], "preview");
  f.good(&["--profile", "test", "--set", "channel=stable"]);
  assert_eq!(f.receipt()["values"]["channel"], "stable");
}

#[test]
fn package_snapshot_runs_after_source_is_removed() {
  let f = Fixture::new();
  let output = f._temp.path().join("delivery");
  f.good(&["package", "--output", output.to_str().unwrap()]);
  fs::remove_dir_all(&f.payload).unwrap();
  fs::remove_file(&f.config).unwrap();
  let engine = output.join(if cfg!(windows) {
    "installer.exe"
  } else {
    "installer"
  });
  let result = Command::new(&engine)
    .arg("--non-interactive")
    .current_dir(f._temp.path())
    .output()
    .unwrap();
  assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
  assert!(f.root.join("installer/installation.yaml").exists());
}

#[test]
fn modified_older_build_is_retained_by_pruning() {
  let f = Fixture::new();
  f.good(&[]);
  let first = f.receipt()["active"].as_str().unwrap().to_string();
  fs::write(
    f.root
      .join("app")
      .join(&first)
      .join(if cfg!(windows) { "probe.exe" } else { "probe" }),
    "modified",
  )
  .unwrap();
  f.update("2.0.0");
  f.good(&[]);
  f.update("3.0.0");
  f.good(&["--prune-old-versions"]);
  assert!(f.receipt()["builds"].get(&first).is_some());
}

#[test]
fn previous_formats_and_migration_flags_are_rejected_without_mutation() {
  let f = Fixture::new();
  fs::create_dir(&f.root).unwrap();
  fs::create_dir(&f.bin).unwrap();
  let receipt = f.root.join(".dotask-install.json");
  let old = r#"{"Schema":1,"AppId":"probe","ActiveDirectory":"old-build"}"#;
  fs::write(&receipt, old).unwrap();
  let launcher = f.bin.join(if cfg!(windows) { "probe.exe" } else { "probe" });
  fs::write(&launcher, "previous launcher").unwrap();
  let result = f.run(&[]);
  assert!(!result.status.success());
  assert!(String::from_utf8_lossy(&result.stderr).contains("Choose a fresh install-dir"));
  let result = f.run(&["--migrate-legacy"]);
  assert!(!result.status.success());
  assert!(String::from_utf8_lossy(&result.stderr).contains("Unknown argument"));
  let result = f.uninstall(&[]);
  assert!(!result.status.success());
  assert_eq!(fs::read_to_string(&receipt).unwrap(), old);
  assert_eq!(fs::read_to_string(&launcher).unwrap(), "previous launcher");
  assert_eq!(fs::read_dir(&f.root).unwrap().count(), 1);
}

#[test]
fn yaml_shortcut_permissions_cannot_be_overridden() {
  let f = Fixture::new();
  let mut package: Value = serde_saphyr::from_str(&fs::read_to_string(&f.config).unwrap()).unwrap();
  package["shortcuts"] = json!([]);
  fs::write(&f.config, serde_saphyr::to_string(&package).unwrap()).unwrap();
  assert!(!f.run(&["--desktop-shortcuts"]).status.success());
  assert!(!f.root.exists());
}

#[test]
fn empty_payload_directories_survive_packaging_and_installation() {
  let f = Fixture::new();
  fs::create_dir(f.payload.join("empty working directory")).unwrap();
  f.good(&[]);
  let build = f.receipt()["active"].as_str().unwrap().to_string();
  assert!(f.root.join("app").join(build).join("empty working directory").is_dir());
  assert!(f.uninstall(&[]).status.success());
}

#[cfg(windows)]
#[test]
fn pruning_retains_locked_old_build_without_partial_deletion() {
  use std::os::windows::fs::OpenOptionsExt;
  let f = Fixture::new();
  fs::write(f.payload.join("a-file.txt"), "must survive").unwrap();
  f.good(&[]);
  let first = f.receipt()["active"].as_str().unwrap().to_string();
  let directory = f.root.join("app").join(&first);
  let held = fs::OpenOptions::new()
    .read(true)
    .share_mode(1)
    .open(directory.join("probe.exe"))
    .unwrap();
  f.update("2.0.0");
  f.good(&[]);
  f.update("3.0.0");
  f.good(&["--prune-old-versions"]);
  assert!(directory.join("a-file.txt").exists());
  assert!(f.receipt()["builds"].get(&first).is_some());
  drop(held);
  assert!(f.uninstall(&[]).status.success());
}

#[cfg(windows)]
#[test]
fn updating_while_stable_shim_is_in_use_only_changes_sidecar() {
  use std::os::windows::fs::OpenOptionsExt;
  let f = Fixture::new();
  f.good(&[]);
  let before = fs::read_to_string(f.bin.join("probe.shim")).unwrap();
  let held = fs::OpenOptions::new()
    .read(true)
    .share_mode(1)
    .open(f.bin.join("probe.exe"))
    .unwrap();
  f.update("2.0.0");
  f.good(&[]);
  assert_ne!(before, fs::read_to_string(f.bin.join("probe.shim")).unwrap());
  drop(held);
  assert!(f.uninstall(&[]).status.success());
}

#[test]
fn root_only_install_and_parent_command_directory_preserve_unowned_files() {
  for additional in [false, true] {
    let f = Fixture::new();
    let mut package: Value = serde_saphyr::from_str(&fs::read_to_string(&f.config).unwrap()).unwrap();
    package["values"]["additional-command"] = additional.into();
    package["values"].as_object_mut().unwrap().remove("bin-dir");
    fs::write(&f.config, serde_saphyr::to_string(&package).unwrap()).unwrap();
    let unrelated = f._temp.path().join("another application.exe");
    fs::write(&unrelated, "keep").unwrap();
    f.good(&[]);
    let name = if cfg!(windows) { "probe.exe" } else { "probe" };
    assert!(f.root.join(name).exists());
    assert_eq!(f._temp.path().join(name).exists(), additional);
    assert_eq!(f.receipt()["values"]["additional-command"], additional);
    f.good(&[]);
    f.update("2.0.0");
    f.good(&[]);
    assert!(Command::new(f.root.join(name)).arg("--help").output().unwrap().status.success());
    assert!(f.uninstall(&[]).status.success());
    assert!(!f.root.exists());
    assert!(!f._temp.path().join(name).exists());
    assert_eq!(fs::read_to_string(unrelated).unwrap(), "keep");
  }
}

#[test]
fn default_interaction_requires_terminal_and_yaml_can_select_unattended() {
  let f = Fixture::new();
  let output = Command::new(env!("CARGO_BIN_EXE_dotask-installer"))
    .args(["--config", f.config.to_str().unwrap()])
    .output()
    .unwrap();
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("--non-interactive"));
  assert!(!f.root.exists());
  let mut package: Value = serde_saphyr::from_str(&fs::read_to_string(&f.config).unwrap()).unwrap();
  package["interactive"] = false.into();
  fs::write(&f.config, serde_saphyr::to_string(&package).unwrap()).unwrap();
  let output = Command::new(env!("CARGO_BIN_EXE_dotask-installer"))
    .args(["--config", f.config.to_str().unwrap()])
    .output()
    .unwrap();
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  assert!(f.root.exists());
  assert!(f.uninstall(&[]).status.success());
}

#[test]
fn updating_can_move_disable_and_enable_an_owned_additional_command() {
  let f = Fixture::new();
  f.good(&[]);
  let first = f.receipt()["active"].clone();
  let receipt_path = f.root.join("installer/installation.yaml");
  let mut receipt = f.receipt();
  // Reproduce a current-schema receipt from before the optional command input.
  // Ownership comes from launcher inventory, not that preference's presence.
  receipt["values"].as_object_mut().unwrap().remove("additional-command");
  fs::write(&receipt_path, serde_saphyr::to_string(&receipt).unwrap()).unwrap();
  let name = if cfg!(windows) { "probe.exe" } else { "probe" };
  let parent = f._temp.path();
  fs::write(f.bin.join("user-owned.txt"), "keep").unwrap();
  f.good(&["--bin-dir", parent.to_str().unwrap()]);
  assert!(!f.bin.join(name).exists());
  assert!(parent.join(name).exists());
  assert_eq!(fs::read_to_string(f.bin.join("user-owned.txt")).unwrap(), "keep");
  assert_eq!(f.receipt()["active"], first);
  f.good(&["--set", "additional-command=false"]);
  assert!(!parent.join(name).exists());
  assert!(f.root.join(name).exists());
  f.good(&[]);
  assert!(f.bin.join(name).exists());
  assert!(f.uninstall(&[]).status.success());
}

#[test]
fn moving_additional_command_refuses_unowned_or_modified_launchers_before_mutation() {
  let f = Fixture::new();
  f.good(&[]);
  let before = fs::read(f.root.join("installer/installation.yaml")).unwrap();
  let name = if cfg!(windows) { "probe.exe" } else { "probe" };
  let destination = f._temp.path().join(name);
  fs::write(&destination, "unowned").unwrap();
  assert!(!f.run(&["--bin-dir", f._temp.path().to_str().unwrap()]).status.success());
  assert_eq!(fs::read(f.root.join("installer/installation.yaml")).unwrap(), before);
  assert_eq!(fs::read_to_string(&destination).unwrap(), "unowned");
  assert!(f.bin.join(name).exists());
  fs::remove_file(&destination).unwrap();
  fs::write(f.bin.join(name), "modified").unwrap();
  assert!(!f.run(&["--bin-dir", f._temp.path().to_str().unwrap()]).status.success());
  assert!(!destination.exists());
  assert_eq!(fs::read(f.root.join("installer/installation.yaml")).unwrap(), before);
}

#[cfg(windows)]
#[test]
fn interrupted_command_move_requires_same_destinations_and_recovers() {
  use std::os::windows::fs::OpenOptionsExt;
  let f = Fixture::new();
  f.good(&[]);
  let held = fs::OpenOptions::new()
    .read(true)
    .share_mode(1)
    .open(f.bin.join("probe.exe"))
    .unwrap();
  let parent = f._temp.path().to_str().unwrap();
  assert!(!f.run(&["--bin-dir", parent]).status.success());
  assert!(f.root.join("installer/pending.yaml").exists());
  let changed = f.run(&["--set", "additional-command=false"]);
  assert!(!changed.status.success());
  assert!(String::from_utf8_lossy(&changed.stderr).contains("original package and locations"));
  drop(held);
  f.good(&["--bin-dir", parent]);
  assert!(!f.root.join("installer/pending.yaml").exists());
  assert!(!f.bin.join("probe.exe").exists());
  assert!(f._temp.path().join("probe.exe").exists());
  assert!(f.uninstall(&[]).status.success());
}

#[cfg(windows)]
#[test]
fn named_start_menu_shortcuts_can_move_between_flat_and_owned_nested_folders() {
  let f = Fixture::new();
  let menu = f._temp.path().join("Start Menu 日本語");
  f.configure(|p| {
    p["shortcuts"] = json!([{"name":"${shortcut-name}","executable":"probe.exe","start-menu":true}]);
    p["values"]["start-menu-dir"] = menu.to_string_lossy().into_owned().into();
    p["values"]["start-menu-shortcuts"] = true.into();
  });
  let custom = "Build Tools 日本語";
  f.good(&["--shortcut-name", custom]);
  assert!(menu.join(format!("{custom}.lnk")).exists());
  f.good(&["--shortcut-name", custom, "--start-menu-nested"]);
  assert!(!menu.join(format!("{custom}.lnk")).exists());
  assert!(menu.join(custom).join(format!("{custom}.lnk")).exists());
  f.good(&["--shortcut-name", "Renamed", "--start-menu-nested"]);
  assert!(!menu.join(custom).exists());
  assert!(menu.join("Renamed/Renamed.lnk").exists());
  fs::write(menu.join("Renamed/user-owned.txt"), "keep").unwrap();
  assert!(f.uninstall(&[]).status.success());
  assert!(!menu.join("Renamed/Renamed.lnk").exists());
  assert!(menu.join("Renamed/user-owned.txt").exists());
}

#[cfg(windows)]
#[test]
fn uninstall_removes_only_installer_created_empty_shortcut_folders() {
  for existing in [false, true] {
    let f = Fixture::new();
    let menu = f._temp.path().join("menu");
    let directory = menu.join("Probe");
    if existing {
      fs::create_dir_all(&directory).unwrap();
    }
    f.configure(|p| {
      p["shortcuts"] = json!([{"name":"${shortcut-name}","executable":"probe.exe","start-menu":true}]);
      p["values"]["start-menu-dir"] = menu.to_string_lossy().into_owned().into();
      p["values"]["start-menu-shortcuts"] = true.into();
      p["values"]["start-menu-nested"] = true.into();
    });
    f.good(&[]);
    assert!(directory.join("Probe.lnk").exists());
    assert!(f.uninstall(&[]).status.success());
    assert_eq!(directory.exists(), existing);
    assert!(menu.exists());
  }
}

#[test]
fn named_package_and_installed_build_use_the_app_version_and_optional_revision() {
  for commit in [None, Some("abcdef71234567890123456789012345678901234")] {
    let f = Fixture::new();
    f.configure(|p| {
      p["application"]["version"] = "2.7.3-rc.1+build.5".into();
      p["application"]["build"] = json!({"stamp":"26279-0612","commit":commit,"dirty":false});
    });
    let packages = f._temp.path().join("packages");
    let result = f._temp.path().join("result.json");
    f.good(&[
      "package",
      "--output-parent",
      packages.to_str().unwrap(),
      "--result-file",
      result.to_str().unwrap(),
    ]);
    let result: Value = serde_json::from_str(&fs::read_to_string(&result).unwrap()).unwrap();
    let artifact = PathBuf::from(result["FilePath"].as_str().unwrap());
    let suffix = if commit.is_some() { "-abcdef7" } else { "" };
    let identity = format!("2.7.3-rc.1+build.5-26279-0612{suffix}");
    assert_eq!(
      artifact.parent().unwrap().file_name().unwrap(),
      format!("probe-{identity}").as_str()
    );
    let install = Command::new(&artifact).arg("--non-interactive").output().unwrap();
    assert!(install.status.success(), "{}", String::from_utf8_lossy(&install.stderr));
    assert_eq!(f.receipt()["active"], identity);
    assert!(f.root.join("app").join(identity).is_dir());
    f.good(&["package", "--output-parent", packages.to_str().unwrap()]);
    assert_eq!(fs::read_dir(&packages).unwrap().count(), 1);
    assert!(f.uninstall(&[]).status.success());
  }
}

#[test]
fn named_identity_collision_refuses_changed_contents_and_preserves_existing_build() {
  let f = Fixture::new();
  f.configure(|p| p["application"]["build"] = json!({"stamp":"26279-0612"}));
  let packages = f._temp.path().join("packages");
  f.good(&["package", "--output-parent", packages.to_str().unwrap()]);
  f.good(&[]);
  let before = fs::read(f.root.join("installer/installation.yaml")).unwrap();
  fs::write(f.payload.join("changed.txt"), "different payload").unwrap();
  assert!(!f
    .run(&["package", "--output-parent", packages.to_str().unwrap()])
    .status
    .success());
  let result = f.run(&[]);
  assert!(!result.status.success());
  assert!(String::from_utf8_lossy(&result.stderr).contains("different contents"));
  assert_eq!(fs::read(f.root.join("installer/installation.yaml")).unwrap(), before);
  assert!(!f.root.join("installer/pending.yaml").exists());
}

#[test]
fn arbitrary_version_text_is_preserved_and_unsafe_path_characters_are_encoded() {
  let f = Fixture::new();
  f.configure(|p| {
    p["application"]["version"] = "Release 2026/10:preview".into();
    p["application"]["build"] = json!({"stamp":"26279-0612"});
  });
  f.good(&[]);
  let identity = f.receipt()["active"].as_str().unwrap().to_string();
  assert_eq!(identity, "Release%202026%2F10%3Apreview-26279-0612");
  let snapshot: Value =
    serde_saphyr::from_str(&fs::read_to_string(f.root.join("app").join(identity).join("installer.yaml")).unwrap())
      .unwrap();
  assert_eq!(snapshot["application"]["version"], "Release 2026/10:preview");
  assert!(f.uninstall(&[]).status.success());
}

#[test]
fn creator_transport_reads_partial_yaml_without_installation_or_prompts() {
  let f = Fixture::new();
  fs::write(&f.config,"schema: 1\napplication: {id: probe, name: 'Custom Probe'}\ninteractive: false\ndefaults: {common: {additional-command: false}}\n").unwrap();
  let output = Command::new(env!("CARGO_BIN_EXE_dotask-installer"))
    .args(["__config", f.config.to_str().unwrap()])
    .output()
    .unwrap();
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let config: Value = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(config["application"]["name"], "Custom Probe");
  assert_eq!(config["interactive"], false);
  assert!(!f.root.exists());
  fs::write(&f.config, "- invalid\n").unwrap();
  let output = Command::new(env!("CARGO_BIN_EXE_dotask-installer"))
    .args(["__config", f.config.to_str().unwrap()])
    .output()
    .unwrap();
  assert!(!output.status.success());
  assert!(!f.root.exists());
}

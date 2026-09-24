use serde_json::{Value, json};
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
        fs::copy(
            env!("CARGO_BIN_EXE_dotask-installer"),
            payload.join(executable),
        )
        .unwrap();
        let config = temp.path().join("probe.yaml");
        let package = json!({ "schema":1, "application":{"id":"probe","name":"Probe","version":"1.0.0"},
            "platform":if cfg!(windows){"windows"}else if cfg!(target_os="macos"){"macos"}else{"linux"},
            "architecture":if cfg!(target_arch="aarch64"){"arm64"}else{"x64"}, "payload":"payload",
            "commands":[{"name":"probe","executable":executable}],
            "shortcuts":[{"name":"Probe","executable":executable,"terminal":true,"local":true,"desktop":true,"start-menu":true}],
            "values":{"install-dir":root,"bin-dir":bin},
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
        Command::new(env!("CARGO_BIN_EXE_dotask-installer"))
            .arg("--config")
            .arg(&self.config)
            .args(args)
            .output()
            .unwrap()
    }
    fn good(&self, args: &[&str]) {
        let result = self.run(args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fn receipt(&self) -> Value {
        serde_saphyr::from_str(
            &fs::read_to_string(self.root.join("installer/installation.yaml")).unwrap(),
        )
        .unwrap()
    }
    fn update(&self, version: &str) {
        let mut config: Value =
            serde_saphyr::from_str(&fs::read_to_string(&self.config).unwrap()).unwrap();
        config["application"]["version"] = version.into();
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
    let command = f
        .root
        .join(if cfg!(windows) { "probe.exe" } else { "probe" });
    assert!(
        Command::new(command)
            .arg("--help")
            .output()
            .unwrap()
            .status
            .success()
    );
    let result = f.uninstall(&[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
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
#[test]
fn modified_owned_files_and_unowned_commands_are_protected() {
    let f = Fixture::new();
    fs::create_dir(&f.bin).unwrap();
    let launcher = f
        .bin
        .join(if cfg!(windows) { "probe.exe" } else { "probe" });
    fs::write(&launcher, "unowned").unwrap();
    assert!(!f.run(&[]).status.success());
    assert!(!f.root.exists());
    assert_eq!(fs::read_to_string(&launcher).unwrap(), "unowned");
    fs::remove_file(launcher).unwrap();
    f.good(&[]);
    let active = f.receipt()["active"].as_str().unwrap().to_string();
    let executable =
        f.root
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
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!settings.exists());
}
#[test]
fn zip_payload_preserves_permissions_and_refuses_traversal() {
    use std::io::Write;
    let f = Fixture::new();
    let archive = f._temp.path().join("payload.zip");
    let exe = if cfg!(windows) { "probe.exe" } else { "probe" };
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    zip.start_file(
        exe,
        zip::write::SimpleFileOptions::default().unix_permissions(0o755),
    )
    .unwrap();
    zip.write_all(&fs::read(f.payload.join(exe)).unwrap())
        .unwrap();
    zip.finish().unwrap();
    let mut package: Value =
        serde_saphyr::from_str(&fs::read_to_string(&f.config).unwrap()).unwrap();
    package["payload"] = "payload.zip".into();
    fs::write(&f.config, serde_saphyr::to_string(&package).unwrap()).unwrap();
    f.good(&[]);
    assert!(f.uninstall(&[]).status.success());
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    zip.start_file("../escape", zip::write::SimpleFileOptions::default())
        .unwrap();
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
    let output = Command::new(engine).arg("uninstall").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!f.root.exists());
}

#[test]
fn inputs_require_values_and_follow_explicit_precedence() {
    let f = Fixture::new();
    let mut package: Value =
        serde_saphyr::from_str(&fs::read_to_string(&f.config).unwrap()).unwrap();
    package["inputs"] =
        json!({"channel":{"type":"choice","required":true,"choices":["stable","preview"]}});
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
        .current_dir(f._temp.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
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
fn legacy_import_requires_explicit_consent_and_preserves_identity() {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    fs::create_dir(&f.root).unwrap();
    fs::create_dir(&f.bin).unwrap();
    let fingerprint = "a".repeat(64);
    let build = format!("0.9.0-{}", &fingerprint[..16]);
    let directory = f.root.join(&build);
    fs::create_dir(&directory).unwrap();
    let exe = if cfg!(windows) { "probe.exe" } else { "probe" };
    fs::copy(f.payload.join(exe), directory.join(exe)).unwrap();
    let hash = format!(
        "{:x}",
        Sha256::digest(fs::read(directory.join(exe)).unwrap())
    );
    let mut mode = 0;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        mode = fs::metadata(directory.join(exe))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
    }
    #[cfg(windows)]
    {
        let _ = &mut mode;
    }
    fs::write(directory.join(".dotask-build.json"),serde_json::to_vec(&json!({"AppId":"probe","Version":"0.9.0","Fingerprint":fingerprint,"Files":[{"Path":exe,"Hash":hash,"Mode":mode}]})).unwrap()).unwrap();
    let launcher = f.bin.join(exe);
    fs::write(&launcher, "legacy launcher").unwrap();
    let commands = json!({exe:{"Kind":"file","Value":base64::prelude::BASE64_STANDARD.encode(b"legacy launcher")}});
    fs::write(f.root.join(".dotask-install.json"),serde_json::to_vec(&json!({"Schema":1,"AppId":"probe","BinDirectory":f.bin,"ActiveDirectory":directory,"Commands":commands})).unwrap()).unwrap();
    assert!(!f.run(&[]).status.success());
    assert!(!f.root.join("app").exists());
    f.good(&["--migrate-legacy"]);
    assert_eq!(f.receipt()["previous"], build);
    assert!(directory.exists());
    let result = f.uninstall(&[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!f.root.exists());
}

#[test]
fn yaml_shortcut_permissions_cannot_be_overridden() {
    let f = Fixture::new();
    let mut package: Value =
        serde_saphyr::from_str(&fs::read_to_string(&f.config).unwrap()).unwrap();
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
    assert!(
        f.root
            .join("app")
            .join(build)
            .join("empty working directory")
            .is_dir()
    );
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
    assert_ne!(
        before,
        fs::read_to_string(f.bin.join("probe.shim")).unwrap()
    );
    drop(held);
    assert!(f.uninstall(&[]).status.success());
}

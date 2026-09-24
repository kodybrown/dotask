//! Explicit import of the .NET engine's ownership format. No directory is
//! adopted by name alone. Old files remain owned until final uninstall.
use crate::{config, files, launchers, model::*};
use anyhow::{Context, Result, ensure};
use base64::Engine;
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct LegacyReceipt {
    schema: u32,
    app_id: String,
    bin_directory: String,
    active_directory: String,
    commands: BTreeMap<String, LegacyCommand>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct LegacyCommand {
    kind: String,
    value: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct LegacyBuild {
    app_id: String,
    version: String,
    fingerprint: String,
    files: Vec<LegacyFile>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct LegacyFile {
    path: String,
    hash: String,
    mode: u32,
}

pub fn import(
    root: &Path,
    package: &Package,
    values: &BTreeMap<String, serde_json::Value>,
) -> Result<Receipt> {
    ensure!(
        !root.join(".dotask-pending.json").exists(),
        "Complete legacy activation recovery with the old installer first"
    );
    let receipt_path = root.join(".dotask-install.json");
    files::safe_path(&receipt_path)?;
    let legacy: LegacyReceipt = serde_json::from_slice(&fs::read(&receipt_path)?)?;
    let bin = config::path(values, "bin-dir")?;
    ensure!(
        legacy.schema == 1
            && legacy.app_id == package.application.id
            && files::absolute(Path::new(&legacy.bin_directory), root)? == bin,
        "Legacy identity or bin directory mismatch; preserve the original locations"
    );
    let active_path = files::absolute(Path::new(&legacy.active_directory), root)?;
    let active = active_path.as_path();
    ensure!(
        active.parent() == Some(root),
        "Legacy active directory escapes installation"
    );
    let active_name = active
        .file_name()
        .context("Invalid legacy active build")?
        .to_string_lossy()
        .into_owned();
    files::name(&active_name)?;
    let mut launchers = Vec::new();
    for (name, command) in &legacy.commands {
        files::name(name)?;
        let path = bin.join(name);
        let file = match command.kind.as_str() {
            "file" => OwnedFile {
                path: path.to_string_lossy().into(),
                hash: Some(files::hash(
                    &base64::prelude::BASE64_STANDARD.decode(&command.value)?,
                )),
                link: None,
            },
            "link" => OwnedFile {
                path: path.to_string_lossy().into(),
                hash: None,
                link: Some(command.value.clone()),
            },
            _ => anyhow::bail!("Unknown legacy command kind"),
        };
        launchers::verify(&file)?;
        launchers.push(file);
    }
    let mut imports = Vec::new();
    let mut legacy_files = vec![launchers::record(&receipt_path)?];
    for entry in fs::read_dir(root)? {
        let directory = entry?.path();
        files::safe_path(&directory)?;
        if !directory.is_dir() || !directory.join(".dotask-build.json").exists() {
            continue;
        }
        let build_path = directory.join(".dotask-build.json");
        files::safe_path(&build_path)?;
        let old: LegacyBuild = serde_json::from_slice(&fs::read(&build_path)?)?;
        let id = directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        files::name(&id)?;
        ensure!(
            old.app_id == package.application.id
                && old.fingerprint.len() == 64
                && old.fingerprint.chars().all(|c| c.is_ascii_hexdigit()),
            "Legacy build identity is invalid"
        );
        ensure!(
            id == format!("{}-{}", old.version, &old.fingerprint[..16]),
            "Legacy build directory mismatch"
        );
        let mut inventory = BTreeMap::new();
        for file in old.files {
            files::relative(&file.path)?;
            let path = directory.join(&file.path);
            files::safe_path(&path)?;
            ensure!(
                files::file_hash(&path)?.eq_ignore_ascii_case(&file.hash),
                "Legacy build modified: {}",
                path.display()
            );
            legacy_files.push(launchers::record(&path)?);
            inventory.insert(
                file.path,
                FileRecord {
                    sha256: file.hash.to_lowercase(),
                    mode: file.mode,
                },
            );
        }
        legacy_files.push(launchers::record(&build_path)?);
        let mut snapshot = package.clone();
        snapshot.application.version = old.version;
        snapshot.payload = "payload".into();
        let yaml = serde_saphyr::to_string(&snapshot)?;
        imports.push((id, directory, inventory, yaml));
    }
    ensure!(
        imports.iter().any(|(id, _, _, _)| id == &active_name),
        "Missing legacy active build"
    );
    // All old ownership is checked before creating the first imported snapshot.
    let app = root.join("app");
    ensure!(
        !app.exists(),
        "Cannot import over an existing app directory"
    );
    fs::create_dir(&app)?;
    let mut builds = BTreeMap::new();
    for (id, source, inventory, yaml) in imports {
        let directory = app.join(&id);
        fs::create_dir(&directory)?;
        files::copy_payload(&source, &directory, &inventory)?;
        files::write_atomic(&directory.join("installer.yaml"), yaml.as_bytes())?;
        files::write_yaml(
            &directory.join("installation.yaml"),
            &BuildReceipt {
                schema: 1,
                app_id: package.application.id.clone(),
                build: id.clone(),
                files: inventory,
                config_hash: files::hash(yaml.as_bytes()),
            },
        )?;
        builds.insert(id, files::file_hash(&directory.join("installation.yaml"))?);
    }
    let result = Receipt {
        schema: 1,
        app_id: package.application.id.clone(),
        root: root.to_string_lossy().into(),
        active: active_name,
        previous: None,
        builds,
        launchers,
        settings: vec![],
        values: values.clone(),
        engine_hash: String::new(),
        config_hash: String::new(),
        uninstalling: false,
        legacy_files,
    };
    fs::create_dir(root.join("installer"))?;
    files::write_yaml(&root.join("installer/installation.yaml"), &result)?;
    Ok(result)
}

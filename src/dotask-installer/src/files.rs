use crate::model::FileRecord;
use anyhow::{Context, Result, bail, ensure};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn file_hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
pub fn temporary() -> Result<tempfile::TempDir> {
    // macOS commonly exposes TMPDIR through /var -> /private/var. Resolve that
    // explicit system location before enforcing no-links inside staging trees.
    Ok(tempfile::tempdir_in(fs::canonicalize(
        std::env::temp_dir(),
    )?)?)
}
pub fn read_yaml<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).with_context(|| format!("Read {}", path.display()))?;
    serde_saphyr::from_str(&text).with_context(|| format!("Invalid YAML: {}", path.display()))
}
pub fn write_yaml<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    write_atomic(path, serde_saphyr::to_string(value)?.as_bytes())
}
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    safe_path(path)?;
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().context("No parent directory")?)?;
    use std::io::Write;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
pub fn name(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 120
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            && !value.ends_with('.')
            && value != "."
            && value != "..",
        "Invalid portable name: {value}"
    );
    let stem = value.split('.').next().unwrap().to_ascii_uppercase();
    ensure!(
        ![
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        ]
        .contains(&stem.as_str()),
        "Reserved name: {value}"
    );
    Ok(())
}
pub fn relative(value: &str) -> Result<PathBuf> {
    ensure!(
        !value.is_empty() && !value.contains(['\\', ':']) && !value.starts_with('/'),
        "Expected portable relative path: {value}"
    );
    for part in value.split('/') {
        ensure!(
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
                && !part
                    .chars()
                    .any(|c| c.is_control() || "\"<>|?*".contains(c)),
            "Invalid portable path: {value}"
        );
    }
    Ok(PathBuf::from(value))
}
// Check every ancestor, not just the leaf: a directory junction could otherwise
// redirect an owned file operation into an unrelated installation.
pub fn safe_path(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute(),
        "Expected absolute path: {}",
        path.display()
    );
    let mut current = PathBuf::new();
    for part in path.components() {
        ensure!(
            !matches!(part, Component::ParentDir),
            "Parent traversal: {}",
            path.display()
        );
        current.push(part);
        // A Windows verbatim drive prefix (\\?\C:) is not a queryable path
        // until the following root separator has been appended.
        if matches!(part, Component::Prefix(_)) || !current.is_absolute() {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) => {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    ensure!(
                        meta.file_attributes() & 0x400 == 0,
                        "Refusing link/junction: {}",
                        current.display()
                    );
                }
                ensure!(
                    !meta.file_type().is_symlink(),
                    "Refusing link: {}",
                    current.display()
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e).with_context(|| format!("Inspect path {}", current.display())),
        }
    }
    Ok(())
}
pub fn overlaps(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        let a = left.to_string_lossy().replace('\\', "/").to_lowercase();
        let b = right.to_string_lossy().replace('\\', "/").to_lowercase();
        let a = a.trim_end_matches('/');
        let b = b.trim_end_matches('/');
        a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
    } else {
        left.starts_with(right) || right.starts_with(left)
    }
}
pub fn absolute(path: &Path, base: &Path) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    ensure!(
        !path.components().any(|p| matches!(p, Component::ParentDir)),
        "Parent traversal: {}",
        path.display()
    );
    let mut ancestor = path.as_path();
    let mut tail = Vec::new();
    while !ancestor.exists() {
        tail.push(
            ancestor
                .file_name()
                .context("Cannot resolve path")?
                .to_os_string(),
        );
        ancestor = ancestor.parent().context("Cannot resolve path parent")?;
    }
    let mut resolved = fs::canonicalize(ancestor)?;
    #[cfg(windows)]
    {
        let text = resolved.to_string_lossy();
        if let Some(unc) = text.strip_prefix("\\\\?\\UNC\\") {
            resolved = PathBuf::from(format!("\\\\{unc}"));
        } else if let Some(dos) = text.strip_prefix("\\\\?\\") {
            resolved = PathBuf::from(dos);
        }
    }
    for part in tail.into_iter().rev() {
        resolved.push(part);
    }
    safe_path(&resolved)?;
    Ok(resolved)
}
pub fn inventory(root: &Path) -> Result<BTreeMap<String, FileRecord>> {
    let mut result = BTreeMap::new();
    fn walk(
        root: &Path,
        directory: &Path,
        result: &mut BTreeMap<String, FileRecord>,
    ) -> Result<()> {
        safe_path(directory)?;
        for item in fs::read_dir(directory)? {
            let path = item?.path();
            safe_path(&path)?;
            if path.is_dir() {
                walk(root, &path, result)?;
            } else {
                ensure!(path.is_file(), "Not a regular file: {}", path.display());
                let rel = path
                    .strip_prefix(root)?
                    .to_str()
                    .context("Non UTF-8 filename")?
                    .replace('\\', "/");
                ensure!(
                    !matches!(rel.as_str(), "installer.yaml" | "installation.yaml"),
                    "Reserved payload filename: {rel}"
                );
                let mut mode = 0;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    mode = fs::metadata(&path)?.permissions().mode() & 0o777;
                }
                #[cfg(windows)]
                {
                    let _ = &mut mode;
                }
                result.insert(
                    rel,
                    FileRecord {
                        sha256: file_hash(&path)?,
                        mode,
                    },
                );
            }
        }
        Ok(())
    }
    walk(root, root, &mut result)?;
    Ok(result)
}
pub fn copy_payload(
    source: &Path,
    target: &Path,
    files: &BTreeMap<String, FileRecord>,
) -> Result<()> {
    fn directories(source: &Path, target: &Path) -> Result<()> {
        safe_path(source)?;
        safe_path(target)?;
        fs::create_dir_all(target)?;
        for item in fs::read_dir(source)? {
            let path = item?.path();
            safe_path(&path)?;
            if path.is_dir() {
                directories(&path, &target.join(path.file_name().unwrap()))?;
            }
        }
        Ok(())
    }
    // Empty working/configuration directories are meaningful payload content too.
    directories(source, target)?;
    for (rel, expected) in files {
        relative(rel)?;
        let from = source.join(rel);
        let to = target.join(rel);
        safe_path(&from)?;
        safe_path(&to)?;
        fs::create_dir_all(to.parent().unwrap())?;
        fs::copy(&from, &to)?;
        ensure!(
            file_hash(&to)? == expected.sha256,
            "Payload changed while copying: {rel}"
        );
    }
    Ok(())
}
pub fn extract(source: &Path, target: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(fs::File::open(source)?)?;
    let mut names = std::collections::HashSet::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let rel = entry.name().trim_end_matches('/');
        // Validate the original name, so normalization cannot hide traversal.
        let path = target.join(relative(rel)?);
        ensure!(
            names.insert(rel.to_ascii_lowercase()),
            "Duplicate ZIP path: {rel}"
        );
        let mode = entry.unix_mode().unwrap_or(0);
        ensure!(
            mode & 0o170000 != 0o120000,
            "ZIP links are unsupported: {rel}"
        );
        if entry.is_dir() {
            fs::create_dir_all(path)?;
        } else {
            safe_path(&path)?;
            fs::create_dir_all(path.parent().unwrap())?;
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)?;
            std::io::copy(&mut entry, &mut file)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    &path,
                    fs::Permissions::from_mode(if mode == 0 { 0o644 } else { mode & 0o777 }),
                )?;
            }
        }
    }
    Ok(())
}
pub fn remove_empty(directory: &Path) -> Result<()> {
    match fs::remove_dir(directory) {
        Ok(()) => Ok(()),
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
            ) =>
        {
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
pub fn verify_file(path: &Path, expected: &str) -> Result<()> {
    safe_path(path)?;
    if path.exists() && file_hash(path)? != expected {
        bail!("Owned file was modified: {}", path.display());
    }
    Ok(())
}

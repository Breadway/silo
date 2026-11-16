use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SiloManifest {
    pub exe_path: String,
    #[serde(default)]
    pub launch_args: String,
    pub proton_version: String,
}

#[derive(Debug)]
pub struct ExtractionResult {
    /// Root directory the archive was extracted into.
    pub extract_dir: PathBuf,
    /// Resolved absolute path to the game executable.
    pub exe_path: PathBuf,
    pub launch_args: String,
    pub proton_version: String,
    /// `true` when silo.toml was found; `false` if exe was auto-detected.
    pub from_manifest: bool,
    /// If `from_manifest` is false and multiple .exe files were found, this
    /// holds all candidates for the UI to surface to the user.
    pub exe_candidates: Vec<PathBuf>,
}

pub fn extract(archive_path: &Path, dest_dir: &Path) -> anyhow::Result<ExtractionResult> {
    let file = std::fs::File::open(archive_path)?;
    let name = archive_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with(".tar.xz") {
        let xz = xz2::read::XzDecoder::new(file);
        let mut archive = tar::Archive::new(xz);
        archive.unpack(dest_dir)?;
    } else {
        let gz = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz);
        archive.unpack(dest_dir)?;
    }

    // Find silo.toml anywhere in the extracted tree.
    let manifest = find_manifest(dest_dir)?;

    let result = match manifest {
        Some((manifest, _path)) => {
            let exe_abs = dest_dir.join(&manifest.exe_path);
            ExtractionResult {
                extract_dir: dest_dir.to_path_buf(),
                exe_path: exe_abs,
                launch_args: manifest.launch_args,
                proton_version: manifest.proton_version,
                from_manifest: true,
                exe_candidates: Vec::new(),
            }
        }
        None => {
            let candidates = find_exe_files(dest_dir)?;
            let (exe_path, proton_version) = if candidates.len() == 1 {
                (candidates[0].clone(), default_proton())
            } else {
                (
                    candidates.first().cloned().unwrap_or_else(|| dest_dir.join("game.exe")),
                    default_proton(),
                )
            };
            ExtractionResult {
                extract_dir: dest_dir.to_path_buf(),
                exe_path,
                launch_args: String::new(),
                proton_version,
                from_manifest: false,
                exe_candidates: candidates,
            }
        }
    };

    Ok(result)
}

fn find_manifest(root: &Path) -> anyhow::Result<Option<(SiloManifest, PathBuf)>> {
    for entry in walkdir::WalkDir::new(root).min_depth(1).max_depth(5) {
        let entry = entry?;
        if entry.file_name() == OsStr::new("silo.toml") {
            let content = std::fs::read_to_string(entry.path())?;
            let manifest: SiloManifest = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("malformed silo.toml: {e}"))?;
            return Ok(Some((manifest, entry.path().to_path_buf())));
        }
    }
    Ok(None)
}

fn find_exe_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut exes = Vec::new();
    for entry in walkdir::WalkDir::new(root).min_depth(1) {
        let entry = entry?;
        if entry.path().extension() == Some(OsStr::new("exe")) {
            exes.push(entry.path().to_path_buf());
        }
    }
    Ok(exes)
}

fn default_proton() -> String {
    std::env::var("SILO_DEFAULT_PROTON").unwrap_or_else(|_| "proton_9".into())
}

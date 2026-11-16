pub mod appid;
pub mod compat;
pub mod shortcuts;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use chrono::Utc;
use tracing::{info, warn};

use crate::config::Config;
use shortcuts::Shortcut;

// ── Steam root discovery ───────────────────────────────────────────────────

pub fn find_steam_root(cfg: &Config) -> anyhow::Result<PathBuf> {
    find_steam_root_at(cfg.steam_root.as_deref())
}

pub fn find_steam_root_at(override_path: Option<&Path>) -> anyhow::Result<PathBuf> {
    if let Some(p) = override_path {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        anyhow::bail!("SILO_STEAM_ROOT points to non-existent path: {}", p.display());
    }

    let candidates = [
        dirs_next::home_dir().map(|h| h.join(".steam/steam")),
        dirs_next::home_dir().map(|h| h.join(".local/share/Steam")),
    ];

    for candidate in candidates.iter().flatten() {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    anyhow::bail!("Steam installation not found; set SILO_STEAM_ROOT")
}

/// Returns all SteamID3 userdata directories found under `<steam_root>/userdata/`.
pub fn find_user_dirs(steam_root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let userdata = steam_root.join("userdata");
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(&userdata)
        .with_context(|| format!("read {}", userdata.display()))?
    {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            dirs.push(entry.path());
        }
    }
    Ok(dirs)
}

// ── Steam lifecycle ────────────────────────────────────────────────────────

/// steamwebhelper is only present when the Steam UI is actually open.
/// The background runtime processes (ubuntu12_32/steam -srt-logger-opened)
/// persist after the UI closes and are not a reliable signal.
pub fn is_steam_running() -> bool {
    std::process::Command::new("pgrep")
        .arg("-x")
        .arg("steamwebhelper")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Sends `steam -shutdown` and polls until steamwebhelper exits.
/// Falls back to SIGTERM after 15s if the graceful shutdown stalls.
pub async fn shutdown_steam() -> anyhow::Result<()> {
    if !is_steam_running() {
        return Ok(());
    }
    info!("sending steam -shutdown");
    tokio::process::Command::new("steam")
        .arg("-shutdown")
        .status()
        .await
        .context("failed to run steam -shutdown")?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while tokio::time::Instant::now() < deadline {
        if !is_steam_running() {
            info!("Steam has exited");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    info!("Steam did not exit gracefully; sending SIGTERM");
    let _ = tokio::process::Command::new("pkill")
        .args(["-TERM", "steamwebhelper"])
        .status()
        .await;

    tokio::time::sleep(Duration::from_secs(3)).await;
    if is_steam_running() {
        anyhow::bail!("Steam did not exit after SIGTERM");
    }
    info!("Steam has exited");
    Ok(())
}

pub fn launch_steam() {
    let _ = std::process::Command::new("steam").spawn();
}

// ── VDF file I/O ───────────────────────────────────────────────────────────

fn backup_file(path: &Path) -> anyhow::Result<PathBuf> {
    let ts = Utc::now().format("%Y%m%dT%H%M%S");
    let backup = path.with_extension(format!("vdf.bak.{ts}"));
    std::fs::copy(path, &backup)
        .with_context(|| format!("backup {} → {}", path.display(), backup.display()))?;
    Ok(backup)
}

/// Adds a new shortcut to shortcuts.vdf for every user directory found.
/// Backs up the file before each write.
pub fn add_shortcut(
    steam_root: &Path,
    shortcut: &Shortcut,
    proton_version: &str,
) -> anyhow::Result<()> {
    let user_dirs = find_user_dirs(steam_root)?;
    if user_dirs.is_empty() {
        anyhow::bail!("no Steam user directories found under {}", steam_root.display());
    }

    for user_dir in &user_dirs {
        let config_dir = user_dir.join("config");
        std::fs::create_dir_all(&config_dir)?;

        write_shortcut_for_user(&config_dir, shortcut)?;
        write_compat_for_user(&config_dir, shortcut.app_id, proton_version)?;
    }
    Ok(())
}

fn write_shortcut_for_user(config_dir: &Path, shortcut: &Shortcut) -> anyhow::Result<()> {
    let vdf_path = config_dir.join("shortcuts.vdf");

    let mut shortcuts = if vdf_path.exists() {
        backup_file(&vdf_path)?;
        let data = std::fs::read(&vdf_path)?;
        match shortcuts::parse(&data) {
            Ok(v) => v,
            Err(e) => {
                warn!("could not parse existing shortcuts.vdf ({e}), starting fresh");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // Replace if appid already present, otherwise append.
    if let Some(pos) = shortcuts.iter().position(|s| s.app_id == shortcut.app_id) {
        shortcuts[pos] = shortcut.clone();
    } else {
        shortcuts.push(shortcut.clone());
    }

    let bytes = shortcuts::serialize(&shortcuts);
    std::fs::write(&vdf_path, bytes)
        .with_context(|| format!("write {}", vdf_path.display()))?;
    Ok(())
}

const MINIMAL_CONFIG_VDF: &str = "\
\"InstallConfigStore\"\n\
{\n\
\t\"Software\"\n\
\t{\n\
\t\t\"Valve\"\n\
\t\t{\n\
\t\t\t\"Steam\"\n\
\t\t\t{\n\
\t\t\t\t\"CompatToolMapping\"\n\
\t\t\t\t{\n\
\t\t\t\t}\n\
\t\t\t}\n\
\t\t}\n\
\t}\n\
}\n";

fn write_compat_for_user(
    config_dir: &Path,
    appid: u32,
    proton_version: &str,
) -> anyhow::Result<()> {
    let vdf_path = config_dir.join("config.vdf");
    if !vdf_path.exists() {
        info!("config.vdf not found; creating minimal one at {}", vdf_path.display());
        std::fs::write(&vdf_path, MINIMAL_CONFIG_VDF)
            .with_context(|| format!("create {}", vdf_path.display()))?;
    } else {
        backup_file(&vdf_path)?;
    }
    compat::set_compat_tool(&vdf_path, appid, proton_version)
        .with_context(|| format!("update CompatToolMapping in {}", vdf_path.display()))
}

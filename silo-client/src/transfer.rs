use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::config::Config;

/// Transfers an archive from the server to a local staging path via rsync over SSH.
/// Returns the local path the archive was written to.
pub async fn rsync_archive(
    cfg: &Config,
    archive_filename: &str,
    progress_tx: tokio::sync::mpsc::Sender<String>,
) -> anyhow::Result<PathBuf> {
    let staging_dir = cfg.games_dir.join(".staging");
    tokio::fs::create_dir_all(&staging_dir).await?;

    let remote = format!(
        "{}:{}/{}",
        cfg.rsync_host, cfg.rsync_remote_path, archive_filename
    );
    let local = staging_dir.join(archive_filename);

    let _ = progress_tx.send(format!("Starting rsync from {remote}")).await;

    // --info=progress2 writes overall progress to stderr; we capture stdout+stderr
    let mut child = Command::new("rsync")
        .args([
            "--archive",
            "--partial",
            "--info=progress2",
            "--rsh=ssh",
            &remote,
            local.to_str().unwrap(),
        ])
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    // Stream rsync progress lines from stderr.
    if let Some(stderr) = child.stderr.take() {
        let tx = progress_tx.clone();
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let trimmed = line.trim().to_owned();
                        if !trimmed.is_empty() {
                            let _ = tx.send(trimmed).await;
                        }
                    }
                }
            }
        });
    }

    let status = child.wait().await?;
    if !status.success() {
        anyhow::bail!("rsync failed with exit code {:?}", status.code());
    }

    Ok(local)
}

/// Checks whether rsync is available on PATH.
pub async fn check_rsync() -> bool {
    Command::new("rsync").arg("--version").output().await.is_ok()
}

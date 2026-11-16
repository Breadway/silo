use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    /// URL of silo-server (e.g. http://100.x.x.x:7743)
    pub server_url: String,
    /// Tailscale hostname or IP of Hestia for rsync (e.g. "hestia" or "100.x.x.x")
    pub rsync_host: String,
    /// rsync source path on the remote host (absolute path to archive dir)
    pub rsync_remote_path: String,
    /// Where to extract game archives on this machine
    pub games_dir: PathBuf,
    /// Address for the local web UI server
    pub listen_addr: String,
    /// Hostname reported to the server in mark-pulled calls
    pub machine_name: String,
    /// Path to the Steam installation (auto-detected if not set)
    pub steam_root: Option<PathBuf>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let server_url = std::env::var("SILO_SERVER_URL")
            .map_err(|_| anyhow::anyhow!("SILO_SERVER_URL must be set (e.g. http://100.x.x.x:7743)"))?;

        let rsync_host = std::env::var("SILO_RSYNC_HOST")
            .map_err(|_| anyhow::anyhow!("SILO_RSYNC_HOST must be set (Hestia's Tailscale hostname or IP)"))?;

        let rsync_remote_path = std::env::var("SILO_RSYNC_PATH")
            .unwrap_or_else(|_| "~/.local/share/silo/archives".into());

        let games_dir = std::env::var("SILO_GAMES_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs_next::home_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join("Games/silo")
            });

        let machine_name = std::env::var("SILO_MACHINE_NAME")
            .unwrap_or_else(|_| {
                hostname::get()
                    .ok()
                    .and_then(|h| h.into_string().ok())
                    .unwrap_or_else(|| "unknown".into())
            });

        Ok(Config {
            server_url,
            rsync_host,
            rsync_remote_path,
            games_dir,
            listen_addr: std::env::var("SILO_LISTEN_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:7744".into()),
            machine_name,
            steam_root: std::env::var("SILO_STEAM_ROOT").ok().map(PathBuf::from),
        })
    }
}

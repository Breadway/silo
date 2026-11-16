use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_addr: String,
    pub db_path: PathBuf,
    pub archive_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        let data_dir = dirs_next::data_dir()
            .unwrap_or_else(|| PathBuf::from("/var/lib"))
            .join("silo");

        Config {
            listen_addr: std::env::var("SILO_LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:7743".into()),
            db_path: std::env::var("SILO_DB_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| data_dir.join("catalog.db")),
            archive_dir: std::env::var("SILO_ARCHIVE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| data_dir.join("archives")),
        }
    }
}

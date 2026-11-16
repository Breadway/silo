use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: i64,
    pub name: String,
    pub archive_filename: String,
    pub size_bytes: i64,
    pub added_at: String,
    pub notes: Option<String>,
    pub pulls: Vec<MachinePull>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachinePull {
    pub machine_name: String,
    pub pulled_at: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterGame {
    pub name: String,
    pub server_path: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchGame {
    pub name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MarkPulled {
    pub machine_name: String,
}

/// Parsed only for validation at upload time; fields are not used server-side.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SiloManifest {
    pub exe_path: String,
    pub launch_args: Option<String>,
    pub proton_version: String,
}

use std::{
    io::Read,
    path::Path,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Multipart, Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::Connection;
use serde_json::{json, Value};
use tracing::info;

use crate::{
    config::Config,
    db,
    error::{AppError, Result},
    models::{MarkPulled, PatchGame, RegisterGame, SiloManifest},
};

pub type DbConn = Arc<Mutex<Connection>>;

pub async fn list_games(State((conn, _)): State<(DbConn, Arc<Config>)>) -> Result<Json<Value>> {
    let games = {
        let conn = conn.lock().unwrap();
        db::list_games(&conn)
    }?;
    Ok(Json(json!(games)))
}

pub async fn get_game(
    State((conn, _)): State<(DbConn, Arc<Config>)>,
    AxumPath(id): AxumPath<i64>,
) -> Result<Json<Value>> {
    let game = {
        let conn = conn.lock().unwrap();
        db::get_game(&conn, id)
    }?;

    match game {
        Some(g) => Ok(Json(json!(g))),
        None => Err(AppError::not_found(format!("game {id} not found"))),
    }
}

/// Register an archive that already exists on the server's filesystem.
/// silo.toml validation is attempted but not required — missing silo.toml
/// is treated as a warning so existing archives can be catalogued.
pub async fn register_game(
    State((conn, cfg)): State<(DbConn, Arc<Config>)>,
    Json(body): Json<RegisterGame>,
) -> Result<(StatusCode, Json<Value>)> {
    let src = Path::new(&body.server_path);
    if !src.exists() {
        return Err(AppError::bad_request(format!(
            "file not found: {}",
            body.server_path
        )));
    }


    let filename = src
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::bad_request("could not determine filename"))?
        .to_owned();

    let dest = cfg.archive_dir.join(&filename);
    if dest != src {
        std::fs::copy(src, &dest).map_err(AppError::internal)?;
    }

    let size_bytes = std::fs::metadata(&dest)
        .map(|m| m.len() as i64)
        .map_err(AppError::internal)?;

    let added_at = Utc::now().to_rfc3339();
    let id = {
        let conn = conn.lock().unwrap();
        db::insert_game(&conn, &body.name, &filename, size_bytes, &added_at, body.notes.as_deref())
    }?;

    info!("registered game {} ({filename})", body.name);

    let game = {
        let conn = conn.lock().unwrap();
        db::get_game(&conn, id)
    }?
    .unwrap();

    Ok((StatusCode::CREATED, Json(json!(game))))
}

/// Upload an archive via multipart form data.
/// Form fields: `name` (text), `notes` (text, optional), `archive` (file).
pub async fn upload_game(
    State((conn, cfg)): State<(DbConn, Arc<Config>)>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Value>)> {
    let mut name: Option<String> = None;
    let mut notes: Option<String> = None;
    let mut archive_bytes: Option<(String, Vec<u8>)> = None;

    while let Some(field) = multipart.next_field().await.map_err(AppError::internal)? {
        let field_name = field.name().unwrap_or("").to_owned();
        match field_name.as_str() {
            "name" => {
                name = Some(field.text().await.map_err(AppError::internal)?);
            }
            "notes" => {
                let t = field.text().await.map_err(AppError::internal)?;
                if !t.is_empty() {
                    notes = Some(t);
                }
            }
            "archive" => {
                let filename = field
                    .file_name()
                    .ok_or_else(|| AppError::bad_request("archive field missing filename"))?
                    .to_owned();
                let bytes = field.bytes().await.map_err(AppError::internal)?;
                archive_bytes = Some((filename, bytes.to_vec()));
            }
            _ => {}
        }
    }

    let name = name.ok_or_else(|| AppError::bad_request("missing `name` field"))?;
    let (filename, bytes) =
        archive_bytes.ok_or_else(|| AppError::bad_request("missing `archive` field"))?;

    let dest = cfg.archive_dir.join(&filename);

    // Write to a temp file for validation, then rename.
    let tmp = dest.with_extension("tmp");
    std::fs::write(&tmp, &bytes).map_err(AppError::internal)?;

    if let Err(e) = validate_archive(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::bad_request(format!("invalid archive: {e}")));
    }

    std::fs::rename(&tmp, &dest).map_err(AppError::internal)?;

    let size_bytes = bytes.len() as i64;
    let added_at = Utc::now().to_rfc3339();
    let id = {
        let conn = conn.lock().unwrap();
        db::insert_game(&conn, &name, &filename, size_bytes, &added_at, notes.as_deref())
    }?;

    info!("uploaded game {} ({filename})", name);

    let game = {
        let conn = conn.lock().unwrap();
        db::get_game(&conn, id)
    }?
    .unwrap();

    Ok((StatusCode::CREATED, Json(json!(game))))
}

pub async fn patch_game(
    State((conn, _)): State<(DbConn, Arc<Config>)>,
    AxumPath(id): AxumPath<i64>,
    Json(body): Json<PatchGame>,
) -> Result<Json<Value>> {
    let updated = {
        let conn = conn.lock().unwrap();
        db::update_game(&conn, id, body.name.as_deref(), body.notes.as_deref())
    }?;

    if !updated {
        return Err(AppError::not_found(format!("game {id} not found")));
    }

    let game = {
        let conn = conn.lock().unwrap();
        db::get_game(&conn, id)
    }?
    .unwrap();

    Ok(Json(json!(game)))
}

pub async fn delete_game(
    State((conn, cfg)): State<(DbConn, Arc<Config>)>,
    AxumPath(id): AxumPath<i64>,
) -> Result<StatusCode> {
    let filename = {
        let conn = conn.lock().unwrap();
        db::delete_game(&conn, id)
    }?;

    match filename {
        None => Err(AppError::not_found(format!("game {id} not found"))),
        Some(f) => {
            let path = cfg.archive_dir.join(&f);
            if path.exists() {
                std::fs::remove_file(&path).map_err(AppError::internal)?;
                info!("deleted archive {f}");
            }
            Ok(StatusCode::NO_CONTENT)
        }
    }
}

pub async fn mark_pulled(
    State((conn, _)): State<(DbConn, Arc<Config>)>,
    AxumPath(id): AxumPath<i64>,
    Json(body): Json<MarkPulled>,
) -> Result<StatusCode> {
    // Verify game exists.
    {
        let conn = conn.lock().unwrap();
        if db::get_game(&conn, id)?.is_none() {
            return Err(AppError::not_found(format!("game {id} not found")));
        }
    }

    let pulled_at = Utc::now().to_rfc3339();
    {
        let conn = conn.lock().unwrap();
        db::upsert_pull(&conn, id, &body.machine_name, &pulled_at)
    }?;

    Ok(StatusCode::NO_CONTENT)
}

fn validate_archive(path: &Path) -> anyhow::Result<SiloManifest> {
    let file = std::fs::File::open(path)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_path_buf();
        if entry_path.file_name() == Some(std::ffi::OsStr::new("silo.toml")) {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            let manifest: SiloManifest = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("malformed silo.toml: {e}"))?;
            return Ok(manifest);
        }
    }

    anyhow::bail!("silo.toml not found in archive")
}

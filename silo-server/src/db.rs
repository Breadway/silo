use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{Game, MachinePull};

pub fn init(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA foreign_keys=ON;

         CREATE TABLE IF NOT EXISTS games (
             id               INTEGER PRIMARY KEY,
             name             TEXT    NOT NULL,
             archive_filename TEXT    NOT NULL UNIQUE,
             size_bytes       INTEGER NOT NULL,
             added_at         TEXT    NOT NULL,
             notes            TEXT
         );

         CREATE TABLE IF NOT EXISTS pulls (
             id           INTEGER PRIMARY KEY,
             game_id      INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
             machine_name TEXT    NOT NULL,
             pulled_at    TEXT    NOT NULL,
             UNIQUE(game_id, machine_name)
         );",
    )?;
    Ok(())
}

pub fn list_games(conn: &Connection) -> anyhow::Result<Vec<Game>> {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM games ORDER BY name")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;

    ids.iter()
        .map(|&id| {
            get_game(conn, id)?
                .ok_or_else(|| anyhow::anyhow!("game {id} disappeared between queries"))
        })
        .collect()
}

pub fn get_game(conn: &Connection, id: i64) -> anyhow::Result<Option<Game>> {
    let row = conn
        .query_row(
            "SELECT id, name, archive_filename, size_bytes, added_at, notes
             FROM games WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()?;

    let Some((id, name, archive_filename, size_bytes, added_at, notes)) = row else {
        return Ok(None);
    };

    let pulls = list_pulls(conn, id)?;
    Ok(Some(Game { id, name, archive_filename, size_bytes, added_at, notes, pulls }))
}

fn list_pulls(conn: &Connection, game_id: i64) -> anyhow::Result<Vec<MachinePull>> {
    let pulls = conn
        .prepare(
            "SELECT machine_name, pulled_at FROM pulls WHERE game_id = ?1
             ORDER BY pulled_at DESC",
        )?
        .query_map(params![game_id], |row| {
            Ok(MachinePull { machine_name: row.get(0)?, pulled_at: row.get(1)? })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(pulls)
}

pub fn insert_game(
    conn: &Connection,
    name: &str,
    archive_filename: &str,
    size_bytes: i64,
    added_at: &str,
    notes: Option<&str>,
) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO games (name, archive_filename, size_bytes, added_at, notes)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![name, archive_filename, size_bytes, added_at, notes],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Updates name and/or notes. `None` for a field means "leave unchanged".
pub fn update_game(
    conn: &Connection,
    id: i64,
    name: Option<&str>,
    notes: Option<&str>,
) -> anyhow::Result<bool> {
    let rows = match (name, notes) {
        (Some(n), Some(no)) => conn.execute(
            "UPDATE games SET name = ?1, notes = ?2 WHERE id = ?3",
            params![n, no, id],
        )?,
        (Some(n), None) => {
            conn.execute("UPDATE games SET name = ?1 WHERE id = ?2", params![n, id])?
        }
        (None, Some(no)) => {
            conn.execute("UPDATE games SET notes = ?1 WHERE id = ?2", params![no, id])?
        }
        (None, None) => return Ok(true),
    };
    Ok(rows > 0)
}

pub fn delete_game(conn: &Connection, id: i64) -> anyhow::Result<Option<String>> {
    let filename: Option<String> = conn
        .query_row(
            "SELECT archive_filename FROM games WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?;

    if filename.is_some() {
        conn.execute("DELETE FROM games WHERE id = ?1", params![id])?;
    }
    Ok(filename)
}

pub fn upsert_pull(
    conn: &Connection,
    game_id: i64,
    machine_name: &str,
    pulled_at: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO pulls (game_id, machine_name, pulled_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(game_id, machine_name) DO UPDATE SET pulled_at = excluded.pulled_at",
        params![game_id, machine_name, pulled_at],
    )?;
    Ok(())
}

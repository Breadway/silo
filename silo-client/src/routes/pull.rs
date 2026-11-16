use std::{convert::Infallible, path::PathBuf, sync::Arc, time::Duration};

use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    config::Config,
    error::{AppError, Result},
    extract,
    jobs::{JobRegistry, PullEvent, PullStage},
    server_api::ServerClient,
    steam,
    steam::shortcuts::Shortcut,
    transfer,
};

#[derive(Clone)]
pub struct PullState {
    pub config: Arc<Config>,
    pub client: Arc<ServerClient>,
    pub jobs: JobRegistry,
}

pub async fn start_pull(
    State(state): State<PullState>,
    Path(game_id): Path<i64>,
) -> Result<Json<serde_json::Value>> {
    let game = state
        .client
        .get_game(game_id)
        .await
        .map_err(AppError::internal)?;

    let job_id = Uuid::new_v4().to_string();
    let tx = state.jobs.insert(job_id.clone(), game_id);

    let cfg = state.config.clone();
    let client = state.client.clone();
    let jobs = state.jobs.clone();
    let jid = job_id.clone();

    tokio::spawn(async move {
        let send = |stage, msg: &str| {
            let _ = tx.send(PullEvent::new(stage, msg));
        };

        send(PullStage::Rsync, "Starting rsync transfer…");
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<String>(64);
        let tx2 = tx.clone();
        tokio::spawn(async move {
            while let Some(line) = progress_rx.recv().await {
                let _ = tx2.send(PullEvent::new(PullStage::Rsync, line));
            }
        });

        let archive_path = match transfer::rsync_archive(&cfg, &game.archive_filename, progress_tx).await {
            Ok(p) => p,
            Err(e) => {
                send(PullStage::Error, &format!("rsync failed: {e}"));
                jobs.remove(&jid);
                return;
            }
        };

        send(PullStage::Extract, "Extracting archive…");
        let game_dir = cfg.games_dir.join(&game.name);
        if let Err(e) = tokio::fs::create_dir_all(&game_dir).await {
            send(PullStage::Error, &format!("failed to create game dir: {e}"));
            jobs.remove(&jid);
            return;
        }

        let extract_result = match tokio::task::spawn_blocking({
            let ap = archive_path.clone();
            let gd = game_dir.clone();
            move || extract::extract(&ap, &gd)
        })
        .await
        {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                send(PullStage::Error, &format!("extraction failed: {e}"));
                jobs.remove(&jid);
                return;
            }
            Err(e) => {
                send(PullStage::Error, &format!("extraction task panicked: {e}"));
                jobs.remove(&jid);
                return;
            }
        };

        if !extract_result.from_manifest && extract_result.exe_candidates.len() > 1 {
            // Multiple .exe candidates - surface them and wait for user selection.
            // For now we pick the first and log a warning; a future revision can
            // pause and ask via the UI.
            tracing::warn!(
                "multiple .exe candidates found; using first: {}",
                extract_result.exe_path.display()
            );
        }

        send(PullStage::Steam, "Registering with Steam…");

        let steam_root = match steam::find_steam_root(&cfg) {
            Ok(r) => r,
            Err(e) => {
                send(PullStage::Error, &format!("Steam not found: {e}"));
                jobs.remove(&jid);
                return;
            }
        };

        let exe_str = extract_result.exe_path.to_string_lossy().to_string();
        let start_dir = extract_result
            .exe_path
            .parent()
            .unwrap_or(&game_dir)
            .to_string_lossy()
            .to_string();

        let appid = steam::appid::compute(&exe_str, &game.name);
        let shortcut = Shortcut {
            app_id: appid,
            app_name: game.name.clone(),
            exe: exe_str.clone(),
            start_dir,
            icon: String::new(),
            launch_options: extract_result.launch_args.clone(),
        };

        if steam::is_steam_running() {
            send(PullStage::Steam, "Closing Steam to update shortcuts…");
            if let Err(e) = steam::shutdown_steam().await {
                send(PullStage::Error, &format!("Steam shutdown failed: {e}"));
                jobs.remove(&jid);
                return;
            }
        }

        if let Err(e) = steam::add_shortcut(&steam_root, &shortcut, &extract_result.proton_version) {
            send(PullStage::Error, &format!("VDF write failed: {e}"));
            jobs.remove(&jid);
            return;
        }

        // Brief pause to let Steam finish cleaning up lock files before relaunch.
        tokio::time::sleep(Duration::from_secs(2)).await;
        info!("Relaunching Steam");
        steam::launch_steam();

        if let Err(e) = client.mark_pulled(game_id, &cfg.machine_name).await {
            tracing::warn!("mark-pulled failed: {e}");
        }

        // Cleanup staging file.
        let _ = tokio::fs::remove_file(&archive_path).await;

        let run_game_id = steam::appid::to_run_game_id(appid);
        let done_event = PullEvent {
            stage: PullStage::Done,
            message: format!("Done! steam://rungameid/{run_game_id}"),
            run_game_id: Some(run_game_id),
        };
        let _ = tx.send(done_event);

        // Keep the channel alive briefly so the client can receive Done.
        tokio::time::sleep(Duration::from_secs(5)).await;
        jobs.remove(&jid);
    });

    Ok(Json(json!({ "job_id": job_id })))
}

pub async fn pull_events(
    State(state): State<PullState>,
    Path(job_id): Path<String>,
) -> Result<Sse<impl tokio_stream::Stream<Item = std::result::Result<Event, Infallible>>>> {
    let rx = state
        .jobs
        .subscribe(&job_id)
        .ok_or_else(|| AppError::not_found(format!("job {job_id} not found")))?;

    let stream = BroadcastStream::new(rx)
        .filter_map(|result| match result {
            Ok(event) => {
                let data = serde_json::to_string(&event).unwrap_or_default();
                Some(Ok(Event::default().data(data)))
            }
            Err(_) => None,
        });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

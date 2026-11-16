mod config;
mod db;
mod error;
mod models;
mod normalize;
mod routes;

use std::sync::{Arc, Mutex};

use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use routes::games::{
    delete_game, get_game, list_games, mark_pulled, patch_game, register_game, upload_game,
};
use routes::health::health;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "silo_server=info".parse().unwrap()),
        )
        .init();

    let cfg = config::Config::from_env();

    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("normalize") {
        return normalize::run(args.collect(), &cfg);
    }

    let cfg = Arc::new(cfg);

    std::fs::create_dir_all(&cfg.archive_dir)?;
    if let Some(parent) = cfg.db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db_conn = rusqlite::Connection::open(&cfg.db_path)?;
    db::init(&db_conn)?;
    let conn = Arc::new(Mutex::new(db_conn));

    let state = (conn, cfg.clone());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health))
        .route("/games", get(list_games).post(register_game))
        .route("/games/upload", post(upload_game))
        .route("/games/:id", get(get_game).patch(patch_game).delete(delete_game))
        .route("/games/:id/mark-pulled", post(mark_pulled))
        .with_state(state)
        .layer(cors);

    info!("silo-server listening on {}", cfg.listen_addr);
    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

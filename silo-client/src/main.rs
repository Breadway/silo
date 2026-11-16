mod assets;
mod config;
mod error;
mod extract;
mod jobs;
mod register;
mod routes;
mod server_api;
mod steam;
mod transfer;

use std::sync::Arc;

use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use routes::{
    catalog,
    pull::{pull_events, start_pull, PullState},
};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "silo_client=info".parse().unwrap()),
        )
        .init();

    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("register") {
        return register::run(args.collect());
    }

    let cfg = Arc::new(config::Config::from_env()?);
    std::fs::create_dir_all(&cfg.games_dir)?;

    let client = Arc::new(server_api::ServerClient::new(cfg.server_url.clone()));
    let jobs = jobs::JobRegistry::default();

    let pull_state = PullState {
        config: cfg.clone(),
        client: client.clone(),
        jobs: jobs.clone(),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let catalog_router = Router::new()
        .route("/catalog", get(catalog::list_games))
        .route("/catalog/:id", get(catalog::get_game).patch(catalog::patch_game).delete(catalog::delete_game))
        .with_state(client.clone());

    let pull_router = Router::new()
        .route("/pull/:id", post(start_pull))
        .route("/pull/:job_id/events", get(pull_events))
        .with_state(pull_state);

    let app = Router::new()
        .nest("/api", catalog_router)
        .nest("/api", pull_router)
        .fallback(assets::serve_asset)
        .layer(cors);

    info!("silo-client listening on http://{}", cfg.listen_addr);
    info!("connecting to silo-server at {}", cfg.server_url);

    open_browser(&cfg.listen_addr);

    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn open_browser(addr: &str) {
    let url = format!("http://{addr}");
    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
}

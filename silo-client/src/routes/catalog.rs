/// Proxy routes that forward catalog CRUD to silo-server.
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::Value;

use crate::{
    error::{AppError, Result},
    server_api::ServerClient,
};

pub async fn list_games(State(client): State<Arc<ServerClient>>) -> Result<Json<Value>> {
    let games = client.list_games().await?;
    Ok(Json(serde_json::json!(games)))
}

pub async fn get_game(
    State(client): State<Arc<ServerClient>>,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let game = client.get_game(id).await?;
    Ok(Json(serde_json::json!(game)))
}

pub async fn patch_game(
    State(client): State<Arc<ServerClient>>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let game = client.patch_game(id, body).await?;
    Ok(Json(serde_json::json!(game)))
}

pub async fn delete_game(
    State(client): State<Arc<ServerClient>>,
    Path(id): Path<i64>,
) -> Result<StatusCode> {
    client.delete_game(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

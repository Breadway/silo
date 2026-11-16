use anyhow::Context;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Game {
    pub id: i64,
    pub name: String,
    pub archive_filename: String,
    pub size_bytes: i64,
    pub added_at: String,
    pub notes: Option<String>,
    pub pulls: Vec<MachinePull>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MachinePull {
    pub machine_name: String,
    pub pulled_at: String,
}

pub struct ServerClient {
    http: Client,
    base_url: String,
}

impl ServerClient {
    pub fn new(base_url: String) -> Self {
        ServerClient { http: Client::new(), base_url }
    }

    pub async fn list_games(&self) -> anyhow::Result<Vec<Game>> {
        let url = format!("{}/games", self.base_url);
        let games: Vec<Game> = self.http.get(&url).send().await?.json().await?;
        Ok(games)
    }

    pub async fn get_game(&self, id: i64) -> anyhow::Result<Game> {
        let url = format!("{}/games/{}", self.base_url, id);
        let game: Game = self.http.get(&url).send().await?.json().await?;
        Ok(game)
    }

    pub async fn patch_game(&self, id: i64, body: Value) -> anyhow::Result<Game> {
        let url = format!("{}/games/{}", self.base_url, id);
        let game: Game =
            self.http.patch(&url).json(&body).send().await?.json().await?;
        Ok(game)
    }

    pub async fn delete_game(&self, id: i64) -> anyhow::Result<()> {
        let url = format!("{}/games/{}", self.base_url, id);
        let resp = self.http.delete(&url).send().await?;
        if !resp.status().is_success() {
            let body: Value = resp.json().await.unwrap_or_default();
            anyhow::bail!("server error: {body}");
        }
        Ok(())
    }

    pub async fn mark_pulled(&self, id: i64, machine_name: &str) -> anyhow::Result<()> {
        let url = format!("{}/games/{}/mark-pulled", self.base_url, id);
        let resp = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "machine_name": machine_name }))
            .send()
            .await
            .context("failed to call mark-pulled")?;
        if !resp.status().is_success() {
            let body: Value = resp.json().await.unwrap_or_default();
            anyhow::bail!("server error: {body}");
        }
        Ok(())
    }
}

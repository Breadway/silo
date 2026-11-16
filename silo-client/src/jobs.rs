use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullStage {
    Queued,
    Rsync,
    Extract,
    Steam,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullEvent {
    pub stage: PullStage,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_game_id: Option<u64>,
}

impl PullEvent {
    pub fn new(stage: PullStage, message: impl Into<String>) -> Self {
        PullEvent { stage, message: message.into(), run_game_id: None }
    }
}

pub struct Job {
    pub game_id: i64,
    pub sender: broadcast::Sender<PullEvent>,
}

#[derive(Clone, Default)]
pub struct JobRegistry(Arc<Mutex<HashMap<String, Job>>>);

impl JobRegistry {
    pub fn insert(&self, job_id: String, game_id: i64) -> broadcast::Sender<PullEvent> {
        let (tx, _) = broadcast::channel(64);
        self.0.lock().unwrap().insert(job_id, Job { game_id, sender: tx.clone() });
        tx
    }

    pub fn subscribe(&self, job_id: &str) -> Option<broadcast::Receiver<PullEvent>> {
        self.0.lock().unwrap().get(job_id).map(|j| j.sender.subscribe())
    }

    pub fn remove(&self, job_id: &str) {
        self.0.lock().unwrap().remove(job_id);
    }
}

//! Axum HTTP admin API: pool stats, history, dynamic config, and logs.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::Deserialize;
use tcp_pool::PoolStats;

use crate::history::HistoryPoint;
use crate::logging::{CounterSnapshot, LogEntry};
use crate::state::AppState;

#[derive(serde::Serialize)]
pub struct StatsResponse {
    pub stats: PoolStats,
}

#[derive(serde::Serialize)]
pub struct LogsResponse {
    pub counters: CounterSnapshot,
    pub logs: Vec<LogEntry>,
}

#[derive(Deserialize)]
pub struct ConfigUpdate {
    pub max_connections: Option<usize>,
    pub idle_timeout_ms: Option<u64>,
    pub acquire_timeout_ms: Option<u64>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/pool/stats", get(get_stats))
        .route("/api/pool/history", get(get_history))
        .route("/api/pool/config", put(update_config))
        .route("/api/logs", get(get_logs))
}

async fn get_stats(State(state): State<Arc<AppState>>) -> Json<StatsResponse> {
    Json(StatsResponse {
        stats: state.pool.stats(),
    })
}

async fn get_history(State(state): State<Arc<AppState>>) -> Json<Vec<HistoryPoint>> {
    Json(state.history.snapshot())
}

async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<ConfigUpdate>,
) -> Json<StatsResponse> {
    if let Some(n) = cfg.max_connections {
        state.pool.set_max_connections(n);
        state.logs.push("info", format!("max_connections -> {n}"));
    }
    if let Some(ms) = cfg.idle_timeout_ms {
        state.pool.set_idle_timeout(Duration::from_millis(ms));
        state.logs.push("info", format!("idle_timeout_ms -> {ms}"));
    }
    if let Some(ms) = cfg.acquire_timeout_ms {
        state.pool.set_acquire_timeout(Duration::from_millis(ms));
        state.logs.push("info", format!("acquire_timeout_ms -> {ms}"));
    }
    Json(StatsResponse {
        stats: state.pool.stats(),
    })
}

async fn get_logs(State(state): State<Arc<AppState>>) -> Json<LogsResponse> {
    Json(LogsResponse {
        counters: state.counters.snapshot(),
        logs: state.logs.snapshot(),
    })
}

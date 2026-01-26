use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::sync::SyncStatus;
use crate::AppState;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullStatus {
    pub sync: SyncStatus,
    pub database: DatabaseStatus,
    pub system: SystemStatus,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    pub block_count: i64,
    pub tx_count: i64,
    pub box_count: i64,
    pub unspent_box_count: i64,
    pub token_count: i64,
    pub address_count: i64,
    pub size_bytes: Option<u64>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub version: String,
    pub uptime_seconds: u64,
    pub memory_usage_mb: Option<u64>,
    pub network: String,
}

static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// GET /status - Get full system status
pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<FullStatus>, (StatusCode, String)> {
    let start = START_TIME.get_or_init(std::time::Instant::now);

    let sync = state.sync_service.get_status().await;

    let db_stats = state
        .db
        .get_stats()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Try to get database file size
    let db_size = std::fs::metadata(&state.config.database)
        .ok()
        .map(|m| m.len());

    let database = DatabaseStatus {
        block_count: db_stats.block_count,
        tx_count: db_stats.tx_count,
        box_count: db_stats.box_count,
        unspent_box_count: db_stats.unspent_box_count,
        token_count: db_stats.token_count,
        address_count: db_stats.address_count,
        size_bytes: db_size,
    };

    let system = SystemStatus {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: start.elapsed().as_secs(),
        memory_usage_mb: get_memory_usage(),
        network: state.config.network.clone(),
    };

    Ok(Json(FullStatus {
        sync,
        database,
        system,
    }))
}

fn get_memory_usage() -> Option<u64> {
    // Try to read from /proc/self/status on Linux
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            return Some(kb / 1024);
                        }
                    }
                }
            }
        }
    }
    None
}

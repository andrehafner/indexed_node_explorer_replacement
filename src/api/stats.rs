use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use duckdb::params;
use std::sync::Arc;

use crate::models::{ApiInfo, Epoch, NetworkStats, PaginatedResponse, Pagination, TableSize};
use crate::AppState;

/// GET /api/v1/info - Get API info
#[utoipa::path(
    get,
    path = "/info",
    tag = "stats",
    responses(
        (status = 200, description = "API information", body = ApiInfo)
    )
)]
pub async fn get_info(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiInfo>, (StatusCode, String)> {
    let sync_status = state.sync_service.get_status().await;

    Ok(Json(ApiInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        network: state.config.network.clone(),
        indexed_height: sync_status.local_height,
        node_height: sync_status.node_height,
    }))
}

/// GET /api/v1/stats - Get explorer statistics
#[utoipa::path(
    get,
    path = "/stats",
    tag = "stats",
    responses(
        (status = 200, description = "Explorer statistics")
    )
)]
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let db_stats = state
        .db
        .get_stats()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let sync_status = state.sync_service.get_status().await;

    // Get latest network stats
    let network = state
        .db
        .query_one(
            "SELECT difficulty, hashrate, block_time_avg, total_coins
             FROM network_stats
             ORDER BY timestamp DESC
             LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, f64>(1)?,
                    row.get::<_, Option<f64>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (difficulty, hashrate, block_time, total_coins) = network.unwrap_or((0, 0.0, None, 0));

    Ok(Json(serde_json::json!({
        "blockCount": db_stats.block_count,
        "transactionCount": db_stats.tx_count,
        "boxCount": db_stats.box_count,
        "unspentBoxCount": db_stats.unspent_box_count,
        "tokenCount": db_stats.token_count,
        "addressCount": db_stats.address_count,
        "indexedHeight": sync_status.local_height,
        "nodeHeight": sync_status.node_height,
        "syncProgress": sync_status.sync_progress,
        "difficulty": difficulty,
        "hashrate": hashrate,
        "blockTimeAvg": block_time,
        "totalCoins": total_coins,
        "circulatingSupply": total_coins
    })))
}

/// GET /api/v1/stats/network - Get network statistics
#[utoipa::path(
    get,
    path = "/networkState",
    tag = "stats",
    responses(
        (status = 200, description = "Network state", body = NetworkStats)
    )
)]
pub async fn get_network_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<NetworkStats>, (StatusCode, String)> {
    let db_stats = state
        .db
        .get_stats()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let network = state
        .db
        .query_one(
            "SELECT difficulty, hashrate, block_time_avg, total_coins
             FROM network_stats
             ORDER BY timestamp DESC
             LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, f64>(1)?,
                    row.get::<_, Option<f64>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (difficulty, hashrate, block_time, total_coins) = network.unwrap_or((0, 0.0, None, 0));

    Ok(Json(NetworkStats {
        version: env!("CARGO_PKG_VERSION").to_string(),
        supply: total_coins,
        transaction_count: db_stats.tx_count,
        circulating_supply: total_coins,
        block_count: db_stats.block_count,
        hash_rate: hashrate,
        difficulty,
        block_time_avg: block_time.unwrap_or(120.0),
    }))
}

/// GET /api/v1/epochs - Get epochs
pub async fn get_epochs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<Epoch>>, (StatusCode, String)> {
    // Calculate epochs based on block height (1024 blocks per epoch)
    let epoch_length = 1024i64;
    let current_height = state
        .db
        .get_sync_height()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total_epochs = (current_height / epoch_length) + 1;

    let mut items = Vec::new();
    let start_epoch = (params.offset as i64).max(0);
    let end_epoch = ((params.offset + params.limit) as i64).min(total_epochs);

    for epoch_idx in start_epoch..end_epoch {
        let height_start = epoch_idx * epoch_length;
        let height_end = ((epoch_idx + 1) * epoch_length - 1).min(current_height);

        // Get timestamps for this epoch
        let timestamps = state
            .db
            .query_one(
                "SELECT MIN(timestamp), MAX(timestamp), COUNT(*)
                 FROM blocks
                 WHERE height >= ? AND height <= ? AND main_chain = TRUE",
                params![height_start, height_end],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if let Some((start_ts, end_ts, block_count)) = timestamps {
            items.push(Epoch {
                index: epoch_idx as i32,
                height_start,
                height_end,
                timestamp_start: start_ts.unwrap_or(0),
                timestamp_end: end_ts,
                block_count: block_count as i32,
            });
        }
    }

    Ok(Json(PaginatedResponse {
        items,
        total: total_epochs,
    }))
}

/// GET /api/v1/epochs/:epochIndex - Get specific epoch
pub async fn get_epoch(
    State(state): State<Arc<AppState>>,
    Path(epoch_index): Path<i32>,
) -> Result<Json<Epoch>, (StatusCode, String)> {
    let epoch_length = 1024i64;
    let current_height = state
        .db
        .get_sync_height()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let height_start = epoch_index as i64 * epoch_length;
    let height_end = ((epoch_index as i64 + 1) * epoch_length - 1).min(current_height);

    if height_start > current_height {
        return Err((StatusCode::NOT_FOUND, "Epoch not found".to_string()));
    }

    let timestamps = state
        .db
        .query_one(
            "SELECT MIN(timestamp), MAX(timestamp), COUNT(*)
             FROM blocks
             WHERE height >= ? AND height <= ? AND main_chain = TRUE",
            params![height_start, height_end],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Epoch not found".to_string()))?;

    Ok(Json(Epoch {
        index: epoch_index,
        height_start,
        height_end,
        timestamp_start: timestamps.0.unwrap_or(0),
        timestamp_end: timestamps.1,
        block_count: timestamps.2 as i32,
    }))
}

/// GET /api/v1/stats/tables - Get table sizes
#[utoipa::path(
    get,
    path = "/stats/tables",
    tag = "stats",
    responses(
        (status = 200, description = "Table sizes", body = Vec<TableSize>)
    )
)]
pub async fn get_table_sizes(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TableSize>>, (StatusCode, String)> {
    // Query DuckDB for table storage info
    let tables = state
        .db
        .query_all(
            "SELECT table_name, estimated_size, column_count
             FROM duckdb_tables()
             WHERE database_name = 'main' AND schema_name = 'main'
             ORDER BY estimated_size DESC",
            [],
            |row| {
                let name: String = row.get(0)?;
                let size: i64 = row.get(1)?;
                Ok((name, size))
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Get row counts for each table
    let mut result = Vec::new();
    for (name, size) in tables {
        let row_count = state
            .db
            .query_one(
                &format!("SELECT COUNT(*) FROM \"{}\"", name),
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .unwrap_or(0);

        result.push(TableSize {
            name,
            row_count,
            size_bytes: size,
        });
    }

    Ok(Json(result))
}

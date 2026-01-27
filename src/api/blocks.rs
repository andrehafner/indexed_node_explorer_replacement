use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use duckdb::params;
use serde::Deserialize;
use std::sync::Arc;

use crate::models::{Block, BlockSummary, PaginatedResponse, Pagination};
use crate::AppState;

#[derive(Deserialize)]
pub struct BlocksQuery {
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(rename = "sortBy")]
    pub sort_by: Option<String>,
    #[serde(rename = "sortDirection")]
    pub sort_direction: Option<String>,
}

fn default_limit() -> i64 { 20 }

/// GET /api/v1/blocks - Get list of blocks
#[utoipa::path(
    get,
    path = "/blocks",
    tag = "blocks",
    params(
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
        ("limit" = Option<i64>, Query, description = "Results per page (max 100)"),
        ("sortBy" = Option<String>, Query, description = "Sort field: height, timestamp, difficulty"),
        ("sortDirection" = Option<String>, Query, description = "Sort direction: asc, desc")
    ),
    responses(
        (status = 200, description = "List of blocks", body = PaginatedResponse<BlockSummary>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_blocks(
    State(state): State<Arc<AppState>>,
    Query(params_query): Query<BlocksQuery>,
) -> Result<Json<PaginatedResponse<BlockSummary>>, (StatusCode, String)> {
    let sort_dir = params_query.sort_direction.as_deref().unwrap_or("desc");
    let order = if sort_dir == "asc" { "ASC" } else { "DESC" };

    let total: i64 = state
        .db
        .query_one(
            "SELECT COUNT(*) FROM blocks WHERE main_chain = TRUE",
            [],
            |row| row.get(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let sql = format!(
        "SELECT block_id, height, timestamp, tx_count, miner_address, difficulty, block_size
         FROM blocks
         WHERE main_chain = TRUE
         ORDER BY height {}
         LIMIT ? OFFSET ?",
        order
    );

    let items = state
        .db
        .query_all(&sql, params![params_query.limit, params_query.offset], |row| {
            Ok(BlockSummary {
                id: row.get(0)?,
                height: row.get(1)?,
                timestamp: row.get(2)?,
                tx_count: row.get(3)?,
                miner_address: row.get(4)?,
                difficulty: row.get(5)?,
                block_size: row.get(6)?,
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PaginatedResponse { items, total }))
}

/// GET /api/v1/blocks/:id - Get block by ID or height
#[utoipa::path(
    get,
    path = "/blocks/{id}",
    tag = "blocks",
    params(
        ("id" = String, Path, description = "Block ID (hex) or height (number)")
    ),
    responses(
        (status = 200, description = "Block details", body = Block),
        (status = 404, description = "Block not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_block(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Block>, (StatusCode, String)> {
    // Try to parse as height first
    let sql = if id.parse::<i64>().is_ok() {
        "SELECT block_id, parent_id, height, timestamp, difficulty, block_size,
                block_coins, block_mining_time, tx_count, miner_address, miner_reward,
                miner_name, main_chain
         FROM blocks WHERE height = ? AND main_chain = TRUE"
    } else {
        "SELECT block_id, parent_id, height, timestamp, difficulty, block_size,
                block_coins, block_mining_time, tx_count, miner_address, miner_reward,
                miner_name, main_chain
         FROM blocks WHERE block_id = ?"
    };

    let block = state
        .db
        .query_one(sql, [&id], |row| {
            Ok(Block {
                id: row.get(0)?,
                parent_id: row.get(1)?,
                height: row.get(2)?,
                timestamp: row.get(3)?,
                difficulty: row.get(4)?,
                block_size: row.get(5)?,
                block_coins: row.get(6)?,
                block_mining_time: row.get(7)?,
                tx_count: row.get(8)?,
                miner_address: row.get(9)?,
                miner_reward: row.get(10)?,
                miner_name: row.get(11)?,
                main_chain: row.get(12)?,
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Block not found".to_string()))?;

    Ok(Json(block))
}

/// GET /api/v1/blocks/headers - Get recent block headers
#[utoipa::path(
    get,
    path = "/blocks/headers",
    tag = "blocks",
    params(
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
        ("limit" = Option<i64>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Block headers", body = Vec<BlockSummary>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_headers(
    State(state): State<Arc<AppState>>,
    Query(pag): Query<Pagination>,
) -> Result<Json<Vec<BlockSummary>>, (StatusCode, String)> {
    let items = state
        .db
        .query_all(
            "SELECT block_id, height, timestamp, tx_count, miner_address, difficulty, block_size
             FROM blocks
             WHERE main_chain = TRUE
             ORDER BY height DESC
             LIMIT ? OFFSET ?",
            params![pag.limit, pag.offset],
            |row| {
                Ok(BlockSummary {
                    id: row.get(0)?,
                    height: row.get(1)?,
                    timestamp: row.get(2)?,
                    tx_count: row.get(3)?,
                    miner_address: row.get(4)?,
                    difficulty: row.get(5)?,
                    block_size: row.get(6)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(items))
}

/// GET /api/v1/blocks/at/:height - Get block at specific height
#[utoipa::path(
    get,
    path = "/blocks/at/{height}",
    tag = "blocks",
    params(
        ("height" = i64, Path, description = "Block height")
    ),
    responses(
        (status = 200, description = "Block at height", body = Block),
        (status = 404, description = "Block not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_block_at_height(
    State(state): State<Arc<AppState>>,
    Path(height): Path<i64>,
) -> Result<Json<Block>, (StatusCode, String)> {
    let block = state
        .db
        .query_one(
            "SELECT block_id, parent_id, height, timestamp, difficulty, block_size,
                    block_coins, block_mining_time, tx_count, miner_address, miner_reward,
                    miner_name, main_chain
             FROM blocks WHERE height = ? AND main_chain = TRUE",
            [height],
            |row| {
                Ok(Block {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    height: row.get(2)?,
                    timestamp: row.get(3)?,
                    difficulty: row.get(4)?,
                    block_size: row.get(5)?,
                    block_coins: row.get(6)?,
                    block_mining_time: row.get(7)?,
                    tx_count: row.get(8)?,
                    miner_address: row.get(9)?,
                    miner_reward: row.get(10)?,
                    miner_name: row.get(11)?,
                    main_chain: row.get(12)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Block not found".to_string()))?;

    Ok(Json(block))
}

/// GET /api/v1/blocks/byMiner/:address - Get blocks by miner address
#[utoipa::path(
    get,
    path = "/blocks/byMiner/{address}",
    tag = "blocks",
    params(
        ("address" = String, Path, description = "Miner address"),
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
        ("limit" = Option<i64>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Blocks by miner", body = PaginatedResponse<BlockSummary>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_blocks_by_miner(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pag): Query<Pagination>,
) -> Result<Json<PaginatedResponse<BlockSummary>>, (StatusCode, String)> {
    let total: i64 = state
        .db
        .query_one(
            "SELECT COUNT(*) FROM blocks WHERE miner_address = ? AND main_chain = TRUE",
            [&address],
            |row| row.get(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let items = state
        .db
        .query_all(
            "SELECT block_id, height, timestamp, tx_count, miner_address, difficulty, block_size
             FROM blocks
             WHERE miner_address = ? AND main_chain = TRUE
             ORDER BY height DESC
             LIMIT ? OFFSET ?",
            params![address, pag.limit, pag.offset],
            |row| {
                Ok(BlockSummary {
                    id: row.get(0)?,
                    height: row.get(1)?,
                    timestamp: row.get(2)?,
                    tx_count: row.get(3)?,
                    miner_address: row.get(4)?,
                    difficulty: row.get(5)?,
                    block_size: row.get(6)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PaginatedResponse { items, total }))
}

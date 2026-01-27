use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use duckdb::params;
use serde::Deserialize;
use std::sync::Arc;

use crate::models::{AddressInfo, Balance, BlockSummary, SearchResult, TokenSummary, TransactionSummary};
use crate::utils::ergo_tree;
use crate::AppState;

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(rename = "query")]
    pub q: String,
}

/// GET /api/v1/search - Universal search
pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<SearchResult>>, (StatusCode, String)> {
    let query = params.q.trim();

    if query.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let mut results = Vec::new();

    // Try to match as block height
    if let Ok(height) = query.parse::<i64>() {
        if let Some(block) = find_block_by_height(&state, height).await? {
            results.push(SearchResult {
                entity_type: "block".to_string(),
                entity_id: block.id.clone(),
                block: Some(block),
                transaction: None,
                address: None,
                token: None,
            });
        }
    }

    // Try to match as block ID
    if query.len() == 64 {
        if let Some(block) = find_block_by_id(&state, query).await? {
            results.push(SearchResult {
                entity_type: "block".to_string(),
                entity_id: block.id.clone(),
                block: Some(block),
                transaction: None,
                address: None,
                token: None,
            });
        }

        // Try as transaction ID
        if let Some(tx) = find_transaction_by_id(&state, query).await? {
            results.push(SearchResult {
                entity_type: "transaction".to_string(),
                entity_id: tx.id.clone(),
                block: None,
                transaction: Some(tx),
                address: None,
                token: None,
            });
        }

        // Try as token ID
        if let Some(token) = find_token_by_id(&state, query).await? {
            results.push(SearchResult {
                entity_type: "token".to_string(),
                entity_id: token.id.clone(),
                block: None,
                transaction: None,
                address: None,
                token: Some(token),
            });
        }

        // Try as box ID
        if let Some(box_info) = find_box_by_id(&state, query).await? {
            // Return the transaction that created this box
            if let Some(tx) = find_transaction_by_id(&state, &box_info.tx_id).await? {
                results.push(SearchResult {
                    entity_type: "box".to_string(),
                    entity_id: query.to_string(),
                    block: None,
                    transaction: Some(tx),
                    address: None,
                    token: None,
                });
            }
        }
    }

    // Try to match as address (starts with 9 for mainnet P2PK)
    if query.starts_with('9') || query.starts_with('2') || query.starts_with('3') {
        if ergo_tree::validate_address(query) {
            if let Some(addr_info) = find_address(&state, query).await? {
                results.push(SearchResult {
                    entity_type: "address".to_string(),
                    entity_id: query.to_string(),
                    block: None,
                    transaction: None,
                    address: Some(addr_info),
                    token: None,
                });
            }
        }
    }

    // Search tokens by name
    let tokens = search_tokens_by_name(&state, query, 5).await?;
    for token in tokens {
        results.push(SearchResult {
            entity_type: "token".to_string(),
            entity_id: token.id.clone(),
            block: None,
            transaction: None,
            address: None,
            token: Some(token),
        });
    }

    Ok(Json(results))
}

/// GET /api/v1/utils/ergoTreeToAddress/:ergoTree - Convert ErgoTree to address
pub async fn ergo_tree_to_address(
    Path(ergo_tree_hex): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let address = ergo_tree::ergo_tree_to_address(&ergo_tree_hex)
        .ok_or((StatusCode::BAD_REQUEST, "Invalid ErgoTree".to_string()))?;

    Ok(Json(serde_json::json!({
        "address": address
    })))
}

// Helper functions

async fn find_block_by_height(
    state: &Arc<AppState>,
    height: i64,
) -> Result<Option<BlockSummary>, (StatusCode, String)> {
    state
        .db
        .query_one(
            "SELECT block_id, height, timestamp, tx_count, miner_address, difficulty, block_size
             FROM blocks WHERE height = ? AND main_chain = TRUE",
            [height],
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn find_block_by_id(
    state: &Arc<AppState>,
    id: &str,
) -> Result<Option<BlockSummary>, (StatusCode, String)> {
    state
        .db
        .query_one(
            "SELECT block_id, height, timestamp, tx_count, miner_address, difficulty, block_size
             FROM blocks WHERE block_id = ?",
            [id],
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn find_transaction_by_id(
    state: &Arc<AppState>,
    id: &str,
) -> Result<Option<TransactionSummary>, (StatusCode, String)> {
    state
        .db
        .query_one(
            "SELECT tx_id, timestamp, inclusion_height, input_count, output_count, size
             FROM transactions WHERE tx_id = ?",
            [id],
            |row| {
                Ok(TransactionSummary {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    inclusion_height: row.get(2)?,
                    input_count: row.get(3)?,
                    output_count: row.get(4)?,
                    size: row.get(5)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn find_token_by_id(
    state: &Arc<AppState>,
    id: &str,
) -> Result<Option<TokenSummary>, (StatusCode, String)> {
    state
        .db
        .query_one(
            "SELECT token_id, name, decimals, emission_amount
             FROM tokens WHERE token_id = ?",
            [id],
            |row| {
                Ok(TokenSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    decimals: row.get(2)?,
                    emission_amount: row.get(3)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

#[derive(Debug)]
struct BoxInfo {
    tx_id: String,
}

async fn find_box_by_id(
    state: &Arc<AppState>,
    id: &str,
) -> Result<Option<BoxInfo>, (StatusCode, String)> {
    state
        .db
        .query_one(
            "SELECT tx_id FROM boxes WHERE box_id = ?",
            [id],
            |row| Ok(BoxInfo { tx_id: row.get(0)? }),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn find_address(
    state: &Arc<AppState>,
    address: &str,
) -> Result<Option<AddressInfo>, (StatusCode, String)> {
    let stats = state
        .db
        .query_one(
            "SELECT tx_count, first_seen_height, last_seen_height
             FROM address_stats WHERE address = ?",
            [address],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some((tx_count, first_seen, last_seen)) = stats {
        // Get balance
        let nano_ergs: i64 = state
            .db
            .query_one(
                "SELECT COALESCE(SUM(value), 0) FROM boxes
                 WHERE address = ? AND spent_tx_id IS NULL",
                [address],
                |row| row.get(0),
            )
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .unwrap_or(0);

        return Ok(Some(AddressInfo {
            address: address.to_string(),
            tx_count,
            balance: Balance {
                nano_ergs,
                tokens: Vec::new(), // Simplified for search
            },
            first_seen_height: first_seen,
            last_seen_height: last_seen,
        }));
    }

    // Check if address has any boxes (might not be in stats yet)
    let has_boxes: Option<i32> = state
        .db
        .query_one(
            "SELECT 1 FROM boxes WHERE address = ? LIMIT 1",
            [address],
            |row| row.get(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if has_boxes.is_some() {
        let nano_ergs: i64 = state
            .db
            .query_one(
                "SELECT COALESCE(SUM(value), 0) FROM boxes
                 WHERE address = ? AND spent_tx_id IS NULL",
                [address],
                |row| row.get(0),
            )
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .unwrap_or(0);

        return Ok(Some(AddressInfo {
            address: address.to_string(),
            tx_count: 0,
            balance: Balance {
                nano_ergs,
                tokens: Vec::new(),
            },
            first_seen_height: None,
            last_seen_height: None,
        }));
    }

    Ok(None)
}

async fn search_tokens_by_name(
    state: &Arc<AppState>,
    query: &str,
    limit: i64,
) -> Result<Vec<TokenSummary>, (StatusCode, String)> {
    let pattern = format!("%{}%", query);

    state
        .db
        .query_all(
            "SELECT token_id, name, decimals, emission_amount
             FROM tokens
             WHERE name LIKE ?
             ORDER BY emission_amount DESC
             LIMIT ?",
            params![pattern, limit],
            |row| {
                Ok(TokenSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    decimals: row.get(2)?,
                    emission_amount: row.get(3)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

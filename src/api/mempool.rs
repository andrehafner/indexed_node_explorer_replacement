use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::models::Pagination;
use crate::AppState;

/// GET /api/v1/mempool/transactions - Get mempool transactions
pub async fn get_mempool_transactions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<Pagination>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    let txs = node
        .get_mempool_transactions(params.limit as i32, params.offset as i32)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "items": txs,
        "total": txs.len()
    })))
}

/// GET /api/v1/mempool/transactions/:txId - Get specific mempool transaction
pub async fn get_mempool_transaction(
    State(state): State<Arc<AppState>>,
    Path(tx_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    // Get all mempool transactions and find the one we want
    let txs = node
        .get_mempool_transactions(1000, 0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let tx = txs
        .into_iter()
        .find(|t| t.id == tx_id)
        .ok_or((StatusCode::NOT_FOUND, "Transaction not found in mempool".to_string()))?;

    Ok(Json(serde_json::to_value(tx).unwrap()))
}

/// GET /api/v1/mempool/transactions/byAddress/:address - Get mempool transactions for address
pub async fn get_mempool_by_address(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    let txs = node
        .get_mempool_transactions(1000, 0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Filter transactions that involve the address
    // This is a simplified check - a full implementation would derive addresses from ErgoTrees
    let filtered: Vec<serde_json::Value> = txs
        .into_iter()
        .filter(|tx| {
            // Check if any output goes to this address
            tx.outputs.iter().any(|o| {
                // Simple check: if the ergo_tree contains patterns that might match
                // A full implementation would properly derive the address
                o.ergo_tree.contains(&address) ||
                crate::utils::ergo_tree::ergo_tree_to_address(&o.ergo_tree)
                    .map(|a| a == address)
                    .unwrap_or(false)
            })
        })
        .map(|tx| serde_json::to_value(tx).unwrap())
        .collect();

    Ok(Json(filtered))
}

/// GET /api/v1/mempool/transactions/byErgoTree/:ergoTree - Get mempool transactions for ErgoTree
pub async fn get_mempool_by_ergo_tree(
    State(state): State<Arc<AppState>>,
    Path(ergo_tree): Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    let txs = node
        .get_mempool_transactions(1000, 0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let filtered: Vec<serde_json::Value> = txs
        .into_iter()
        .filter(|tx| tx.outputs.iter().any(|o| o.ergo_tree == ergo_tree))
        .map(|tx| serde_json::to_value(tx).unwrap())
        .collect();

    Ok(Json(filtered))
}

/// GET /api/v1/mempool/size - Get mempool size
pub async fn get_mempool_size(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    let size = node
        .get_mempool_size()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "size": size })))
}

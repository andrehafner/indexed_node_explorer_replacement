use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use duckdb::params;
use serde::Deserialize;
use std::sync::Arc;

use crate::models::{DataInput, Input, Output, BoxAsset, PaginatedResponse, Pagination, Transaction, TransactionSummary};
use crate::AppState;

#[derive(Deserialize)]
pub struct TxQuery {
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(rename = "sortDirection")]
    pub sort_direction: Option<String>,
}

#[derive(Deserialize)]
pub struct StreamQuery {
    #[serde(rename = "minGix")]
    pub min_gix: Option<i64>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 { 20 }

/// GET /api/v1/transactions - Get list of transactions
#[utoipa::path(
    get,
    path = "/transactions",
    tag = "transactions",
    params(
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
        ("limit" = Option<i64>, Query, description = "Results per page"),
        ("sortDirection" = Option<String>, Query, description = "Sort direction: asc, desc")
    ),
    responses(
        (status = 200, description = "Transaction list", body = PaginatedTransactions),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_transactions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TxQuery>,
) -> Result<Json<PaginatedResponse<TransactionSummary>>, (StatusCode, String)> {
    let total: i64 = state
        .db
        .query_one("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let order = if params.sort_direction.as_deref() == Some("asc") { "ASC" } else { "DESC" };

    let sql = format!(
        "SELECT tx_id, timestamp, inclusion_height, input_count, output_count, size
         FROM transactions
         ORDER BY inclusion_height {}
         LIMIT ? OFFSET ?",
        order
    );

    let items = state
        .db
        .query_all(&sql, params![params.limit, params.offset], |row| {
            Ok(TransactionSummary {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                inclusion_height: row.get(2)?,
                input_count: row.get(3)?,
                output_count: row.get(4)?,
                size: row.get(5)?,
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PaginatedResponse { items, total }))
}

/// GET /api/v1/transactions/:id - Get transaction by ID
#[utoipa::path(
    get,
    path = "/transactions/{id}",
    tag = "transactions",
    params(
        ("id" = String, Path, description = "Transaction ID")
    ),
    responses(
        (status = 200, description = "Transaction details", body = Transaction),
        (status = 404, description = "Transaction not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_transaction(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Transaction>, (StatusCode, String)> {
    // Get transaction base info
    let tx = state
        .db
        .query_one(
            "SELECT tx_id, block_id, inclusion_height, timestamp, index_in_block,
                    global_index, coinbase, size
             FROM transactions WHERE tx_id = ?",
            [&id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,  // tx_id
                    row.get::<_, String>(1)?,  // block_id
                    row.get::<_, i64>(2)?,     // inclusion_height
                    row.get::<_, i64>(3)?,     // timestamp
                    row.get::<_, i32>(4)?,     // index_in_block
                    row.get::<_, i64>(5)?,     // global_index
                    row.get::<_, bool>(6)?,    // coinbase
                    row.get::<_, i32>(7)?,     // size
                ))
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Transaction not found".to_string()))?;

    // Get inputs
    let inputs = state
        .db
        .query_all(
            "SELECT i.box_id, b.value, b.address, b.tx_id, b.output_index
             FROM inputs i
             LEFT JOIN boxes b ON i.box_id = b.box_id
             WHERE i.tx_id = ?
             ORDER BY i.input_index",
            [&id],
            |row| {
                Ok(Input {
                    box_id: row.get(0)?,
                    value: row.get(1)?,
                    address: row.get(2)?,
                    tx_id: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    output_index: row.get::<_, Option<i32>>(4)?.unwrap_or(0),
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Get outputs
    let outputs = get_outputs_for_tx(&state, &id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Get data inputs
    let data_inputs = state
        .db
        .query_all(
            "SELECT box_id FROM data_inputs WHERE tx_id = ? ORDER BY input_index",
            [&id],
            |row| Ok(DataInput { box_id: row.get(0)? }),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(Transaction {
        id: tx.0,
        block_id: tx.1,
        inclusion_height: tx.2,
        timestamp: tx.3,
        index: tx.4,
        global_index: tx.5,
        coinbase: tx.6,
        size: tx.7,
        inputs,
        outputs,
        data_inputs,
    }))
}

/// GET /api/v1/transactions/byBlock/:blockId - Get transactions in a block
#[utoipa::path(
    get,
    path = "/transactions/byBlockId/{blockId}",
    tag = "transactions",
    params(
        ("blockId" = String, Path, description = "Block ID"),
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
        ("limit" = Option<i64>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Transactions in block", body = PaginatedTransactions)
    )
)]
pub async fn get_transactions_by_block(
    State(state): State<Arc<AppState>>,
    Path(block_id): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<TransactionSummary>>, (StatusCode, String)> {
    let total: i64 = state
        .db
        .query_one(
            "SELECT COUNT(*) FROM transactions WHERE block_id = ?",
            [&block_id],
            |row| row.get(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let items = state
        .db
        .query_all(
            "SELECT tx_id, timestamp, inclusion_height, input_count, output_count, size
             FROM transactions
             WHERE block_id = ?
             ORDER BY index_in_block ASC
             LIMIT ? OFFSET ?",
            params![block_id, params.limit, params.offset],
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PaginatedResponse { items, total }))
}

/// GET /api/v1/transactions/byAddress/:address - Get transactions for an address
#[utoipa::path(
    get,
    path = "/addresses/{address}/transactions",
    tag = "addresses",
    params(
        ("address" = String, Path, description = "Ergo address"),
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
        ("limit" = Option<i64>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Address transactions", body = PaginatedTransactions)
    )
)]
pub async fn get_transactions_by_address(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<TransactionSummary>>, (StatusCode, String)> {
    // Count unique transactions for this address (input or output)
    let total: i64 = state
        .db
        .query_one(
            "SELECT COUNT(DISTINCT t.tx_id)
             FROM transactions t
             WHERE t.tx_id IN (
                 SELECT tx_id FROM boxes WHERE address = ?
                 UNION
                 SELECT i.tx_id FROM inputs i
                 JOIN boxes b ON i.box_id = b.box_id
                 WHERE b.address = ?
             )",
            params![address, address],
            |row| row.get(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let items = state
        .db
        .query_all(
            "SELECT DISTINCT t.tx_id, t.timestamp, t.inclusion_height, t.input_count, t.output_count, t.size
             FROM transactions t
             WHERE t.tx_id IN (
                 SELECT tx_id FROM boxes WHERE address = ?
                 UNION
                 SELECT i.tx_id FROM inputs i
                 JOIN boxes b ON i.box_id = b.box_id
                 WHERE b.address = ?
             )
             ORDER BY t.inclusion_height DESC
             LIMIT ? OFFSET ?",
            params![address, address, params.limit, params.offset],
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PaginatedResponse { items, total }))
}

/// GET /api/v1/transactions/byInputsScriptTemplateHash/:hash
pub async fn get_transactions_by_template(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<TransactionSummary>>, (StatusCode, String)> {
    let total: i64 = state
        .db
        .query_one(
            "SELECT COUNT(DISTINCT i.tx_id)
             FROM inputs i
             JOIN boxes b ON i.box_id = b.box_id
             WHERE b.ergo_tree_template_hash = ?",
            [&hash],
            |row| row.get(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let items = state
        .db
        .query_all(
            "SELECT DISTINCT t.tx_id, t.timestamp, t.inclusion_height, t.input_count, t.output_count, t.size
             FROM transactions t
             JOIN inputs i ON t.tx_id = i.tx_id
             JOIN boxes b ON i.box_id = b.box_id
             WHERE b.ergo_tree_template_hash = ?
             ORDER BY t.inclusion_height DESC
             LIMIT ? OFFSET ?",
            params![hash, params.limit, params.offset],
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PaginatedResponse { items, total }))
}

/// GET /api/v1/transactions/byGlobalIndex/stream
pub async fn stream_transactions_by_gix(
    State(state): State<Arc<AppState>>,
    Query(params): Query<StreamQuery>,
) -> Result<Json<Vec<TransactionSummary>>, (StatusCode, String)> {
    let min_gix = params.min_gix.unwrap_or(0);

    let items = state
        .db
        .query_all(
            "SELECT tx_id, timestamp, inclusion_height, input_count, output_count, size
             FROM transactions
             WHERE global_index >= ?
             ORDER BY global_index ASC
             LIMIT ?",
            params![min_gix, params.limit],
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(items))
}

/// POST /api/v1/transactions/submit - Submit a transaction
pub async fn submit_transaction(
    State(state): State<Arc<AppState>>,
    Json(tx): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    let tx_id = node
        .submit_transaction(&tx)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(serde_json::json!({ "id": tx_id })))
}

// Helper function to get outputs for a transaction
fn get_outputs_for_tx(state: &Arc<AppState>, tx_id: &str) -> anyhow::Result<Vec<Output>> {
    let outputs = state.db.query_all(
        "SELECT box_id, tx_id, output_index, ergo_tree, address, value,
                creation_height, settlement_height, additional_registers, spent_tx_id
         FROM boxes WHERE tx_id = ? ORDER BY output_index",
        [tx_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,    // box_id
                row.get::<_, String>(1)?,    // tx_id
                row.get::<_, i32>(2)?,       // output_index
                row.get::<_, String>(3)?,    // ergo_tree
                row.get::<_, String>(4)?,    // address
                row.get::<_, i64>(5)?,       // value
                row.get::<_, i64>(6)?,       // creation_height
                row.get::<_, i64>(7)?,       // settlement_height
                row.get::<_, Option<String>>(8)?, // additional_registers
                row.get::<_, Option<String>>(9)?, // spent_tx_id
            ))
        },
    )?;

    let mut result = Vec::new();
    for output in outputs {
        // Get assets for this box
        let assets = state.db.query_all(
            "SELECT ba.token_id, ba.amount, ba.asset_index, t.name, t.decimals
             FROM box_assets ba
             LEFT JOIN tokens t ON ba.token_id = t.token_id
             WHERE ba.box_id = ?
             ORDER BY ba.asset_index",
            [&output.0],
            |row| {
                Ok(BoxAsset {
                    token_id: row.get(0)?,
                    amount: row.get(1)?,
                    index: row.get(2)?,
                    name: row.get(3)?,
                    decimals: row.get(4)?,
                })
            },
        )?;

        let additional_registers = output.8
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());

        result.push(Output {
            box_id: output.0,
            tx_id: output.1,
            index: output.2,
            ergo_tree: output.3,
            address: output.4,
            value: output.5,
            creation_height: output.6,
            settlement_height: output.7,
            assets,
            additional_registers,
            spent_tx_id: output.9,
            main_chain: true,
        });
    }

    Ok(result)
}

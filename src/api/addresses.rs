use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use duckdb::params;
use std::sync::Arc;

use crate::models::{AddressInfo, Balance, PaginatedResponse, Pagination, TokenBalance, TransactionSummary};
use crate::AppState;

/// GET /api/v1/addresses/:address - Get address info
#[utoipa::path(
    get,
    path = "/addresses/{address}",
    tag = "addresses",
    params(
        ("address" = String, Path, description = "Ergo address")
    ),
    responses(
        (status = 200, description = "Address information with balance", body = AddressInfo),
        (status = 404, description = "Address not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_address(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Result<Json<AddressInfo>, (StatusCode, String)> {
    // Get address stats
    let stats = state
        .db
        .query_one(
            "SELECT tx_count, balance, first_seen_height, last_seen_height
             FROM address_stats WHERE address = ?",
            [&address],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Calculate current balance from unspent boxes
    let nano_ergs: i64 = state
        .db
        .query_one(
            "SELECT COALESCE(SUM(value), 0) FROM boxes
             WHERE address = ? AND spent_tx_id IS NULL",
            [&address],
            |row| row.get(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    // Get token balances
    let tokens = state
        .db
        .query_all(
            "SELECT ba.token_id, SUM(ba.amount) as total, t.name, t.decimals
             FROM box_assets ba
             JOIN boxes b ON ba.box_id = b.box_id
             LEFT JOIN tokens t ON ba.token_id = t.token_id
             WHERE b.address = ? AND b.spent_tx_id IS NULL
             GROUP BY ba.token_id, t.name, t.decimals
             ORDER BY total DESC",
            [&address],
            |row| {
                Ok(TokenBalance {
                    token_id: row.get(0)?,
                    amount: row.get(1)?,
                    name: row.get(2)?,
                    decimals: row.get(3)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (tx_count, _, first_seen, last_seen) = stats.unwrap_or((0, 0, None, None));

    Ok(Json(AddressInfo {
        address,
        tx_count,
        balance: Balance {
            nano_ergs,
            tokens,
        },
        first_seen_height: first_seen,
        last_seen_height: last_seen,
    }))
}

/// GET /api/v1/addresses/:address/balance/total - Get total balance
pub async fn get_balance_total(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Result<Json<Balance>, (StatusCode, String)> {
    let nano_ergs: i64 = state
        .db
        .query_one(
            "SELECT COALESCE(SUM(value), 0) FROM boxes
             WHERE address = ? AND spent_tx_id IS NULL",
            [&address],
            |row| row.get(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let tokens = state
        .db
        .query_all(
            "SELECT ba.token_id, SUM(ba.amount) as total, t.name, t.decimals
             FROM box_assets ba
             JOIN boxes b ON ba.box_id = b.box_id
             LEFT JOIN tokens t ON ba.token_id = t.token_id
             WHERE b.address = ? AND b.spent_tx_id IS NULL
             GROUP BY ba.token_id, t.name, t.decimals
             ORDER BY total DESC",
            [&address],
            |row| {
                Ok(TokenBalance {
                    token_id: row.get(0)?,
                    amount: row.get(1)?,
                    name: row.get(2)?,
                    decimals: row.get(3)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(Balance { nano_ergs, tokens }))
}

/// GET /api/v1/addresses/:address/balance/confirmed - Get confirmed balance
pub async fn get_balance_confirmed(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Result<Json<Balance>, (StatusCode, String)> {
    // For now, same as total (we're not tracking mempool separately)
    get_balance_total(State(state), Path(address)).await
}

/// GET /api/v1/addresses/:address/transactions - Get address transactions
pub async fn get_address_transactions(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<TransactionSummary>>, (StatusCode, String)> {
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

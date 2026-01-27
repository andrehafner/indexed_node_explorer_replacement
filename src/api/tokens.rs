use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use duckdb::params;
use serde::Deserialize;
use std::sync::Arc;

use crate::models::{PaginatedResponse, Pagination, Token, TokenBalance, TokenSummary};
use crate::AppState;

#[derive(Deserialize)]
pub struct TokenSearchQuery {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 { 20 }

/// GET /api/v1/tokens - Get list of tokens
#[utoipa::path(
    get,
    path = "/tokens",
    tag = "tokens",
    params(
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
        ("limit" = Option<i64>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Token list", body = PaginatedResponse<TokenSummary>)
    )
)]
pub async fn get_tokens(
    State(state): State<Arc<AppState>>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<TokenSummary>>, (StatusCode, String)> {
    let total: i64 = state
        .db
        .query_one("SELECT COUNT(*) FROM tokens", [], |row| row.get(0))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let items = state
        .db
        .query_all(
            "SELECT token_id, name, decimals, emission_amount
             FROM tokens
             ORDER BY creation_height DESC
             LIMIT ? OFFSET ?",
            params![params.limit, params.offset],
            |row| {
                Ok(TokenSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    decimals: row.get(2)?,
                    emission_amount: row.get(3)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PaginatedResponse { items, total }))
}

/// GET /api/v1/tokens/:tokenId - Get token by ID
#[utoipa::path(
    get,
    path = "/tokens/{tokenId}",
    tag = "tokens",
    params(
        ("tokenId" = String, Path, description = "Token ID")
    ),
    responses(
        (status = 200, description = "Token details", body = Token),
        (status = 404, description = "Token not found")
    )
)]
pub async fn get_token(
    State(state): State<Arc<AppState>>,
    Path(token_id): Path<String>,
) -> Result<Json<Token>, (StatusCode, String)> {
    let token = state
        .db
        .query_one(
            "SELECT token_id, box_id, emission_amount, name, description, token_type, decimals, creation_height
             FROM tokens WHERE token_id = ?",
            [&token_id],
            |row| {
                Ok(Token {
                    id: row.get(0)?,
                    box_id: row.get(1)?,
                    emission_amount: row.get(2)?,
                    name: row.get(3)?,
                    description: row.get(4)?,
                    token_type: row.get(5)?,
                    decimals: row.get(6)?,
                    creation_height: row.get(7)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Token not found".to_string()))?;

    Ok(Json(token))
}

/// GET /api/v1/tokens/search - Search tokens by name
#[utoipa::path(
    get,
    path = "/tokens/search",
    tag = "tokens",
    params(
        ("query" = Option<String>, Query, description = "Search query"),
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
        ("limit" = Option<i64>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Search results", body = PaginatedResponse<TokenSummary>)
    )
)]
pub async fn search_tokens(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TokenSearchQuery>,
) -> Result<Json<PaginatedResponse<TokenSummary>>, (StatusCode, String)> {
    let search_pattern = format!("%{}%", params.query);

    let total: i64 = state
        .db
        .query_one(
            "SELECT COUNT(*) FROM tokens WHERE name LIKE ? OR token_id LIKE ?",
            params![search_pattern, search_pattern],
            |row| row.get(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let items = state
        .db
        .query_all(
            "SELECT token_id, name, decimals, emission_amount
             FROM tokens
             WHERE name LIKE ? OR token_id LIKE ?
             ORDER BY creation_height DESC
             LIMIT ? OFFSET ?",
            params![search_pattern, search_pattern, params.limit, params.offset],
            |row| {
                Ok(TokenSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    decimals: row.get(2)?,
                    emission_amount: row.get(3)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PaginatedResponse { items, total }))
}

/// GET /api/v1/tokens/:tokenId/holders - Get token holders
#[utoipa::path(
    get,
    path = "/tokens/{tokenId}/holders",
    tag = "tokens",
    params(
        ("tokenId" = String, Path, description = "Token ID"),
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
        ("limit" = Option<i64>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Token holders", body = PaginatedResponse<TokenHolder>)
    )
)]
pub async fn get_token_holders(
    State(state): State<Arc<AppState>>,
    Path(token_id): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<TokenHolder>>, (StatusCode, String)> {
    let total: i64 = state
        .db
        .query_one(
            "SELECT COUNT(DISTINCT b.address)
             FROM boxes b
             JOIN box_assets ba ON b.box_id = ba.box_id
             WHERE ba.token_id = ? AND b.spent_tx_id IS NULL",
            [&token_id],
            |row| row.get(0),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let items = state
        .db
        .query_all(
            "SELECT b.address, SUM(ba.amount) as balance
             FROM boxes b
             JOIN box_assets ba ON b.box_id = ba.box_id
             WHERE ba.token_id = ? AND b.spent_tx_id IS NULL
             GROUP BY b.address
             ORDER BY balance DESC
             LIMIT ? OFFSET ?",
            params![token_id, params.limit, params.offset],
            |row| {
                Ok(TokenHolder {
                    address: row.get(0)?,
                    balance: row.get(1)?,
                })
            },
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PaginatedResponse { items, total }))
}

/// GET /api/v1/tokens/byAddress/:address - Get tokens held by address
/// GET /api/v1/assets/byAddress/:address - Alias
pub async fn get_tokens_by_address(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Result<Json<Vec<TokenBalance>>, (StatusCode, String)> {
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

    Ok(Json(tokens))
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenHolder {
    pub address: String,
    pub balance: i64,
}

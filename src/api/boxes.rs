use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use duckdb::params;
use serde::Deserialize;
use std::sync::Arc;

use crate::models::{BoxAsset, BoxSearchQuery, Output, PaginatedResponse, Pagination};
use crate::AppState;

#[derive(Deserialize)]
pub struct StreamQuery {
    #[serde(rename = "minGix")]
    pub min_gix: Option<i64>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Deserialize)]
pub struct EpochStreamQuery {
    #[serde(rename = "lastEpochs")]
    pub last_epochs: Option<i32>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 { 100 }

/// GET /api/v1/boxes/:boxId - Get box by ID
#[utoipa::path(
    get,
    path = "/boxes/{boxId}",
    tag = "boxes",
    params(
        ("boxId" = String, Path, description = "Box ID")
    ),
    responses(
        (status = 200, description = "Box details", body = Output),
        (status = 404, description = "Box not found")
    )
)]
pub async fn get_box(
    State(state): State<Arc<AppState>>,
    Path(box_id): Path<String>,
) -> Result<Json<Output>, (StatusCode, String)> {
    get_box_by_id(&state, &box_id).await
}

/// GET /api/v1/boxes/byAddress/:address - Get all boxes by address
#[utoipa::path(
    get,
    path = "/boxes/byAddress/{address}",
    tag = "boxes",
    params(
        ("address" = String, Path, description = "Ergo address"),
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
        ("limit" = Option<i64>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Boxes by address", body = PaginatedResponse<Output>)
    )
)]
pub async fn get_boxes_by_address(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    get_boxes_with_filter(&state, "address = ?", &address, &params, false).await
}

/// GET /api/v1/boxes/unspent/byAddress/:address - Get unspent boxes by address
#[utoipa::path(
    get,
    path = "/boxes/unspent/byAddress/{address}",
    tag = "boxes",
    params(
        ("address" = String, Path, description = "Ergo address"),
        ("offset" = Option<i64>, Query, description = "Pagination offset"),
        ("limit" = Option<i64>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Unspent boxes by address", body = PaginatedResponse<Output>)
    )
)]
pub async fn get_unspent_boxes_by_address(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    get_boxes_with_filter(&state, "address = ?", &address, &params, true).await
}

/// GET /api/v1/boxes/unspent/unconfirmed/byAddress/:address
pub async fn get_unspent_unconfirmed_by_address(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    // For now, same as unspent (mempool boxes would be added from mempool table)
    get_boxes_with_filter(&state, "address = ?", &address, &params, true).await
}

/// GET /api/v1/boxes/unspent/all/byAddress/:address
pub async fn get_all_unspent_by_address(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    get_boxes_with_filter(&state, "address = ?", &address, &params, true).await
}

/// GET /api/v1/boxes/byErgoTree/:ergoTree - Get boxes by ErgoTree
pub async fn get_boxes_by_ergo_tree(
    State(state): State<Arc<AppState>>,
    Path(ergo_tree): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    get_boxes_with_filter(&state, "ergo_tree = ?", &ergo_tree, &params, false).await
}

/// GET /api/v1/boxes/unspent/byErgoTree/:ergoTree
pub async fn get_unspent_by_ergo_tree(
    State(state): State<Arc<AppState>>,
    Path(ergo_tree): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    get_boxes_with_filter(&state, "ergo_tree = ?", &ergo_tree, &params, true).await
}

/// GET /api/v1/boxes/byErgoTreeTemplateHash/:hash
pub async fn get_boxes_by_template_hash(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    get_boxes_with_filter(&state, "ergo_tree_template_hash = ?", &hash, &params, false).await
}

/// GET /api/v1/boxes/unspent/byErgoTreeTemplateHash/:hash
pub async fn get_unspent_by_template_hash(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    get_boxes_with_filter(&state, "ergo_tree_template_hash = ?", &hash, &params, true).await
}

/// GET /api/v1/boxes/byTokenId/:tokenId - Get boxes containing token
pub async fn get_boxes_by_token(
    State(state): State<Arc<AppState>>,
    Path(token_id): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    get_boxes_by_token_id(&state, &token_id, &params, false).await
}

/// GET /api/v1/boxes/unspent/byTokenId/:tokenId
pub async fn get_unspent_by_token(
    State(state): State<Arc<AppState>>,
    Path(token_id): Path<String>,
    Query(params): Query<Pagination>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    get_boxes_by_token_id(&state, &token_id, &params, true).await
}

/// GET /api/v1/boxes/unspent/stream
pub async fn stream_unspent(
    State(state): State<Arc<AppState>>,
    Query(params): Query<StreamQuery>,
) -> Result<Json<Vec<Output>>, (StatusCode, String)> {
    let min_gix = params.min_gix.unwrap_or(0);
    stream_boxes(&state, min_gix, params.limit, true, None).await
}

/// GET /api/v1/boxes/unspent/byLastEpochs/stream
pub async fn stream_unspent_by_epochs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EpochStreamQuery>,
) -> Result<Json<Vec<Output>>, (StatusCode, String)> {
    let epochs = params.last_epochs.unwrap_or(1);
    let epoch_length = 1024i64; // Ergo epoch length

    // Get current height
    let current_height = state.db.get_sync_height()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let min_height = (current_height - (epochs as i64 * epoch_length)).max(0);

    let items = state
        .db
        .query_all(
            "SELECT box_id, tx_id, output_index, ergo_tree, address, value,
                    creation_height, settlement_height, additional_registers, spent_tx_id
             FROM boxes
             WHERE spent_tx_id IS NULL AND creation_height >= ?
             ORDER BY global_index ASC
             LIMIT ?",
            params![min_height, params.limit],
            |row| box_from_row(row),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut result = Vec::new();
    for output in items {
        let box_with_assets = enrich_box_with_assets(&state, output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        result.push(box_with_assets);
    }

    Ok(Json(result))
}

/// GET /api/v1/boxes/unspent/byGlobalIndex/stream
pub async fn stream_unspent_by_gix(
    State(state): State<Arc<AppState>>,
    Query(params): Query<StreamQuery>,
) -> Result<Json<Vec<Output>>, (StatusCode, String)> {
    let min_gix = params.min_gix.unwrap_or(0);
    stream_boxes(&state, min_gix, params.limit, true, None).await
}

/// GET /api/v1/boxes/byGlobalIndex/stream
pub async fn stream_by_gix(
    State(state): State<Arc<AppState>>,
    Query(params): Query<StreamQuery>,
) -> Result<Json<Vec<Output>>, (StatusCode, String)> {
    let min_gix = params.min_gix.unwrap_or(0);
    stream_boxes(&state, min_gix, params.limit, false, None).await
}

/// GET /api/v1/boxes/byErgoTreeTemplateHash/:hash/stream
pub async fn stream_by_template_hash(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Query(params): Query<StreamQuery>,
) -> Result<Json<Vec<Output>>, (StatusCode, String)> {
    let min_gix = params.min_gix.unwrap_or(0);
    stream_boxes(&state, min_gix, params.limit, false, Some(("ergo_tree_template_hash", hash))).await
}

/// GET /api/v1/boxes/unspent/byErgoTreeTemplateHash/:hash/stream
pub async fn stream_unspent_by_template_hash(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Query(params): Query<StreamQuery>,
) -> Result<Json<Vec<Output>>, (StatusCode, String)> {
    let min_gix = params.min_gix.unwrap_or(0);
    stream_boxes(&state, min_gix, params.limit, true, Some(("ergo_tree_template_hash", hash))).await
}

/// POST /api/v1/boxes/search - Search boxes
pub async fn search_boxes(
    State(state): State<Arc<AppState>>,
    Query(params): Query<Pagination>,
    Json(query): Json<BoxSearchQuery>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    search_boxes_impl(&state, &params, &query, false).await
}

/// POST /api/v1/boxes/unspent/search - Search unspent boxes
pub async fn search_unspent_boxes(
    State(state): State<Arc<AppState>>,
    Query(params): Query<Pagination>,
    Json(query): Json<BoxSearchQuery>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    search_boxes_impl(&state, &params, &query, true).await
}

/// POST /api/v1/boxes/unspent/search/union - Search unspent boxes by multiple assets
pub async fn search_unspent_union(
    State(state): State<Arc<AppState>>,
    Query(params): Query<Pagination>,
    Json(query): Json<BoxSearchQuery>,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    search_boxes_impl(&state, &params, &query, true).await
}

// Helper functions

async fn get_box_by_id(state: &Arc<AppState>, box_id: &str) -> Result<Json<Output>, (StatusCode, String)> {
    let output = state
        .db
        .query_one(
            "SELECT box_id, tx_id, output_index, ergo_tree, address, value,
                    creation_height, settlement_height, additional_registers, spent_tx_id
             FROM boxes WHERE box_id = ?",
            [box_id],
            |row| box_from_row(row),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Box not found".to_string()))?;

    let result = enrich_box_with_assets(state, output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(result))
}

async fn get_boxes_with_filter(
    state: &Arc<AppState>,
    filter: &str,
    value: &str,
    params: &Pagination,
    unspent_only: bool,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    let spent_filter = if unspent_only { " AND spent_tx_id IS NULL" } else { "" };

    let count_sql = format!("SELECT COUNT(*) FROM boxes WHERE {}{}", filter, spent_filter);
    let total: i64 = state
        .db
        .query_one(&count_sql, [value], |row| row.get(0))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let sql = format!(
        "SELECT box_id, tx_id, output_index, ergo_tree, address, value,
                creation_height, settlement_height, additional_registers, spent_tx_id
         FROM boxes WHERE {}{}
         ORDER BY creation_height DESC
         LIMIT ? OFFSET ?",
        filter, spent_filter
    );

    let boxes = state
        .db
        .query_all(&sql, params![value, params.limit, params.offset], |row| box_from_row(row))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut items = Vec::new();
    for output in boxes {
        let enriched = enrich_box_with_assets(state, output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        items.push(enriched);
    }

    Ok(Json(PaginatedResponse { items, total }))
}

async fn get_boxes_by_token_id(
    state: &Arc<AppState>,
    token_id: &str,
    params: &Pagination,
    unspent_only: bool,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    let spent_filter = if unspent_only { " AND b.spent_tx_id IS NULL" } else { "" };

    let count_sql = format!(
        "SELECT COUNT(DISTINCT b.box_id)
         FROM boxes b
         JOIN box_assets ba ON b.box_id = ba.box_id
         WHERE ba.token_id = ?{}",
        spent_filter
    );

    let total: i64 = state
        .db
        .query_one(&count_sql, [token_id], |row| row.get(0))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let sql = format!(
        "SELECT DISTINCT b.box_id, b.tx_id, b.output_index, b.ergo_tree, b.address, b.value,
                b.creation_height, b.settlement_height, b.additional_registers, b.spent_tx_id
         FROM boxes b
         JOIN box_assets ba ON b.box_id = ba.box_id
         WHERE ba.token_id = ?{}
         ORDER BY b.creation_height DESC
         LIMIT ? OFFSET ?",
        spent_filter
    );

    let boxes = state
        .db
        .query_all(&sql, params![token_id, params.limit, params.offset], |row| box_from_row(row))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut items = Vec::new();
    for output in boxes {
        let enriched = enrich_box_with_assets(state, output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        items.push(enriched);
    }

    Ok(Json(PaginatedResponse { items, total }))
}

async fn stream_boxes(
    state: &Arc<AppState>,
    min_gix: i64,
    limit: i64,
    unspent_only: bool,
    extra_filter: Option<(&str, String)>,
) -> Result<Json<Vec<Output>>, (StatusCode, String)> {
    let spent_filter = if unspent_only { " AND spent_tx_id IS NULL" } else { "" };
    let extra = match &extra_filter {
        Some((col, _)) => format!(" AND {} = ?", col),
        None => String::new(),
    };

    let sql = format!(
        "SELECT box_id, tx_id, output_index, ergo_tree, address, value,
                creation_height, settlement_height, additional_registers, spent_tx_id
         FROM boxes
         WHERE global_index >= ?{}{}
         ORDER BY global_index ASC
         LIMIT ?",
        spent_filter, extra
    );

    let boxes = if let Some((_, ref val)) = extra_filter {
        state
            .db
            .query_all(&sql, params![min_gix, val, limit], |row| box_from_row(row))
    } else {
        state
            .db
            .query_all(&sql, params![min_gix, limit], |row| box_from_row(row))
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut result = Vec::new();
    for output in boxes {
        let enriched = enrich_box_with_assets(state, output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        result.push(enriched);
    }

    Ok(Json(result))
}

async fn search_boxes_impl(
    state: &Arc<AppState>,
    params: &Pagination,
    query: &BoxSearchQuery,
    unspent_only: bool,
) -> Result<Json<PaginatedResponse<Output>>, (StatusCode, String)> {
    let mut conditions: Vec<String> = Vec::new();
    let mut _values: Vec<String> = Vec::new();

    if let Some(ref hash) = query.ergo_tree_template_hash {
        conditions.push("ergo_tree_template_hash = ?".to_string());
        _values.push(hash.clone());
    }

    if let Some(ref assets) = query.assets {
        if !assets.is_empty() {
            let placeholders: Vec<&str> = assets.iter().map(|_| "?").collect();
            conditions.push(format!(
                "box_id IN (SELECT box_id FROM box_assets WHERE token_id IN ({}))",
                placeholders.join(",")
            ));
            _values.extend(assets.iter().cloned());
        }
    }

    if unspent_only {
        conditions.push("spent_tx_id IS NULL".to_string());
    }

    let where_clause = if conditions.is_empty() {
        "1=1".to_string()
    } else {
        conditions.join(" AND ")
    };

    let count_sql = format!("SELECT COUNT(*) FROM boxes WHERE {}", where_clause);

    // For simplicity, execute with no params for now (would need dynamic param handling)
    let total: i64 = state
        .db
        .query_one(&count_sql, [], |row| row.get(0))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let sql = format!(
        "SELECT box_id, tx_id, output_index, ergo_tree, address, value,
                creation_height, settlement_height, additional_registers, spent_tx_id
         FROM boxes WHERE {}
         ORDER BY creation_height DESC
         LIMIT ? OFFSET ?",
        where_clause
    );

    let boxes = state
        .db
        .query_all(&sql, params![params.limit, params.offset], |row| box_from_row(row))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut items = Vec::new();
    for output in boxes {
        let enriched = enrich_box_with_assets(state, output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        items.push(enriched);
    }

    Ok(Json(PaginatedResponse { items, total }))
}

fn box_from_row(row: &duckdb::Row<'_>) -> Result<Output, duckdb::Error> {
    Ok(Output {
        box_id: row.get(0)?,
        tx_id: row.get(1)?,
        index: row.get(2)?,
        ergo_tree: row.get(3)?,
        address: row.get(4)?,
        value: row.get(5)?,
        creation_height: row.get(6)?,
        settlement_height: row.get(7)?,
        additional_registers: row.get::<_, Option<String>>(8)?
            .and_then(|s| serde_json::from_str(&s).ok()),
        spent_tx_id: row.get(9)?,
        assets: Vec::new(), // Will be populated by enrich_box_with_assets
        main_chain: true,
    })
}

fn enrich_box_with_assets(state: &Arc<AppState>, mut output: Output) -> anyhow::Result<Output> {
    let assets = state.db.query_all(
        "SELECT ba.token_id, ba.amount, ba.asset_index, t.name, t.decimals
         FROM box_assets ba
         LEFT JOIN tokens t ON ba.token_id = t.token_id
         WHERE ba.box_id = ?
         ORDER BY ba.asset_index",
        [&output.box_id],
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

    output.assets = assets;
    Ok(output)
}

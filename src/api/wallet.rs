use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::models::{WalletBalance, WalletStatus};
use crate::AppState;

/// GET /api/v1/wallet/status - Get wallet status
#[utoipa::path(
    get,
    path = "/wallet/status",
    tag = "wallet",
    responses(
        (status = 200, description = "Wallet status", body = WalletStatus),
        (status = 503, description = "Node unavailable")
    )
)]
pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<WalletStatus>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    match node.wallet_status().await {
        Ok(status) => {
            let initialized = status.get("isInitialized").and_then(|v| v.as_bool()).unwrap_or(false);
            let unlocked = status.get("isUnlocked").and_then(|v| v.as_bool()).unwrap_or(false);
            let change_address = status.get("changeAddress").and_then(|v| v.as_str()).map(|s| s.to_string());
            let wallet_height = status.get("walletHeight").and_then(|v| v.as_i64());

            Ok(Json(WalletStatus {
                initialized,
                unlocked,
                change_address,
                wallet_height,
                error: None,
            }))
        }
        Err(e) => {
            Ok(Json(WalletStatus {
                initialized: false,
                unlocked: false,
                change_address: None,
                wallet_height: None,
                error: Some(e.to_string()),
            }))
        }
    }
}

/// GET /api/v1/wallet/addresses - Get wallet addresses
#[utoipa::path(
    get,
    path = "/wallet/addresses",
    tag = "wallet",
    responses(
        (status = 200, description = "List of wallet addresses", body = Vec<String>),
        (status = 503, description = "Node unavailable")
    )
)]
pub async fn get_addresses(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    let addresses = node
        .wallet_addresses()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(addresses))
}

/// GET /api/v1/wallet/balances - Get wallet balances
#[utoipa::path(
    get,
    path = "/wallet/balances",
    tag = "wallet",
    responses(
        (status = 200, description = "Wallet balances", body = WalletBalance),
        (status = 503, description = "Node unavailable")
    )
)]
pub async fn get_balances(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    let balances = node
        .wallet_balances()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(balances))
}

#[derive(Deserialize)]
pub struct UnlockRequest {
    pub pass: String,
}

/// POST /api/v1/wallet/unlock - Unlock wallet
pub async fn unlock(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UnlockRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    node.wallet_unlock(&req.pass)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

/// POST /api/v1/wallet/lock - Lock wallet
pub async fn lock(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    node.wallet_lock()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

/// POST /api/v1/wallet/transaction/generate - Generate unsigned transaction
pub async fn generate_transaction(
    State(state): State<Arc<AppState>>,
    Json(requests): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    let tx = node
        .wallet_transaction_generate(&requests)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(tx))
}

/// POST /api/v1/wallet/transaction/send - Send transaction
pub async fn send_transaction(
    State(state): State<Arc<AppState>>,
    Json(requests): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = state
        .sync_service
        .get_primary_node()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No node available".to_string()))?;

    let tx_id = node
        .wallet_transaction_send(&requests)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(serde_json::json!({ "id": tx_id })))
}

//! OpenAPI/Swagger documentation

use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::models::*;
use crate::AppState;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Ergo Index API",
        version = "1.0.0",
        description = "Lightweight Ergo blockchain indexer and explorer API.

This API provides full Explorer-compatible endpoints for querying the Ergo blockchain.

## Features
- Full block, transaction, and UTXO querying
- Address balance and transaction history
- Token registry and holder information
- Mempool transaction tracking
- Universal search
- Node wallet integration

## Authentication
Most endpoints are public. Wallet endpoints require the node API key to be configured.",
        license(name = "MIT", url = "https://opensource.org/licenses/MIT")
    ),
    servers(
        (url = "/api/v1", description = "API v1")
    ),
    tags(
        (name = "blocks", description = "Block operations"),
        (name = "transactions", description = "Transaction operations"),
        (name = "addresses", description = "Address operations"),
        (name = "boxes", description = "Box (UTXO) operations"),
        (name = "tokens", description = "Token operations"),
        (name = "mempool", description = "Mempool operations"),
        (name = "stats", description = "Statistics and network info"),
        (name = "search", description = "Search functionality"),
        (name = "wallet", description = "Node wallet operations")
    ),
    components(
        schemas(
            Block,
            BlockSummary,
            Transaction,
            TransactionSummary,
            Input,
            Output,
            DataInput,
            BoxAsset,
            Token,
            TokenSummary,
            AddressInfo,
            Balance,
            TokenBalance,
            MempoolTransaction,
            NetworkStats,
            SearchResult,
            ApiInfo,
            Pagination,
            BoxSearchQuery,
            Epoch,
            WalletStatus,
            WalletBalance,
            PaymentRequest,
        )
    )
)]
pub struct ApiDoc;

pub fn swagger_routes() -> Router<Arc<AppState>> {
    Router::new()
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
}

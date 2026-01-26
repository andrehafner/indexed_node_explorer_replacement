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
    paths(
        // Blocks
        crate::api::blocks::get_blocks,
        crate::api::blocks::get_block,
        crate::api::blocks::get_headers,
        crate::api::blocks::get_block_at_height,
        crate::api::blocks::get_blocks_by_miner,
        // Transactions
        crate::api::transactions::get_transactions,
        crate::api::transactions::get_transaction,
        crate::api::transactions::get_transactions_by_block,
        crate::api::transactions::get_transactions_by_address,
        crate::api::transactions::submit_transaction,
        // Addresses
        crate::api::addresses::get_address,
        crate::api::addresses::get_balance_total,
        crate::api::addresses::get_balance_confirmed,
        crate::api::addresses::get_address_transactions,
        // Boxes
        crate::api::boxes::get_box,
        crate::api::boxes::get_boxes_by_address,
        crate::api::boxes::get_unspent_boxes_by_address,
        crate::api::boxes::get_boxes_by_token,
        crate::api::boxes::get_unspent_by_token,
        // Tokens
        crate::api::tokens::get_tokens,
        crate::api::tokens::get_token,
        crate::api::tokens::search_tokens,
        crate::api::tokens::get_token_holders,
        crate::api::tokens::get_tokens_by_address,
        // Mempool
        crate::api::mempool::get_mempool_transactions,
        crate::api::mempool::get_mempool_size,
        // Stats
        crate::api::stats::get_info,
        crate::api::stats::get_stats,
        crate::api::stats::get_network_stats,
        crate::api::stats::get_epochs,
        // Search
        crate::api::search::search,
        crate::api::search::ergo_tree_to_address,
        // Wallet
        crate::api::wallet::get_status,
        crate::api::wallet::get_addresses,
        crate::api::wallet::get_balances,
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
            PaginatedResponse<BlockSummary>,
            PaginatedResponse<TransactionSummary>,
            PaginatedResponse<Output>,
            PaginatedResponse<TokenSummary>,
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

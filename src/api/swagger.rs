//! OpenAPI/Swagger documentation

use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::{addresses, blocks, boxes, search, stats, tokens, transactions, wallet};
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

## 🌟 Exclusive Endpoints (Not in standard Explorer API)

### Statistics
- `GET /stats/tables` - Database table sizes and row counts

### Tokens
- `GET /tokens/{tokenId}/holders` - Token holder list with balances

### Boxes (UTXOs)
- `GET /boxes/unspent/all/byAddress/{address}` - Combined confirmed + mempool UTXOs
- `GET /boxes/unspent/unconfirmed/byAddress/{address}` - Mempool-only UTXOs
- `GET /boxes/byErgoTree/{ergoTree}` - Boxes by ErgoTree script
- `GET /boxes/unspent/byErgoTree/{ergoTree}` - Unspent boxes by ErgoTree
- `GET /boxes/byErgoTreeTemplateHash/{hash}` - Boxes by template hash
- `GET /boxes/unspent/byErgoTreeTemplateHash/{hash}` - Unspent by template hash
- `GET /boxes/byTokenId/{tokenId}` - All boxes containing a token
- `GET /boxes/unspent/byTokenId/{tokenId}` - Unspent boxes with token
- `POST /boxes/search` - Advanced box search with filters
- `POST /boxes/unspent/search` - Advanced unspent box search
- `POST /boxes/unspent/search/union` - Multi-criteria box search

### Streaming Endpoints (Efficient pagination)
- `GET /boxes/unspent/stream` - Stream all unspent boxes
- `GET /boxes/unspent/byLastEpochs/stream` - Stream by epochs
- `GET /boxes/unspent/byGlobalIndex/stream` - Stream by global index
- `GET /boxes/byGlobalIndex/stream` - Stream all boxes by index
- `GET /boxes/byErgoTreeTemplateHash/{hash}/stream` - Stream by template
- `GET /boxes/unspent/byErgoTreeTemplateHash/{hash}/stream` - Stream unspent by template
- `GET /transactions/byGlobalIndex/stream` - Stream transactions

### Utilities
- `GET /utils/ergoTreeToAddress/{ergoTree}` - Convert ErgoTree to address

### Wallet Integration
- `GET /wallet/status` - Wallet lock/unlock status
- `GET /wallet/addresses` - Wallet addresses
- `GET /wallet/balances` - Wallet balances
- `POST /wallet/unlock` - Unlock wallet
- `POST /wallet/lock` - Lock wallet
- `POST /wallet/transaction/generate` - Generate transaction
- `POST /wallet/transaction/send` - Send transaction

## Authentication
Most endpoints are public. Wallet endpoints require the node API key to be configured.",
        license(name = "MIT", url = "https://opensource.org/licenses/MIT")
    ),
    servers(
        (url = "/api/v1", description = "API v1")
    ),
    tags(
        (name = "info", description = "API information and statistics"),
        (name = "blocks", description = "Block operations"),
        (name = "transactions", description = "Transaction operations"),
        (name = "addresses", description = "Address operations"),
        (name = "boxes", description = "Box (UTXO) operations"),
        (name = "tokens", description = "Token operations"),
        (name = "search", description = "Search functionality"),
        (name = "wallet", description = "Node wallet operations (🌟 EXCLUSIVE)")
    ),
    paths(
        // Info & Stats
        stats::get_info,
        stats::get_stats,
        stats::get_network_stats,
        stats::get_table_sizes,
        // Blocks
        blocks::get_blocks,
        blocks::get_block,
        blocks::get_headers,
        blocks::get_block_at_height,
        blocks::get_blocks_by_miner,
        // Transactions
        transactions::get_transactions,
        transactions::get_transaction,
        transactions::get_transactions_by_block,
        transactions::get_transactions_by_address,
        // Addresses
        addresses::get_address,
        // Boxes
        boxes::get_box,
        boxes::get_boxes_by_address,
        boxes::get_unspent_boxes_by_address,
        // Tokens
        tokens::get_tokens,
        tokens::get_token,
        tokens::search_tokens,
        tokens::get_token_holders,
        // Search
        search::search,
        // Wallet
        wallet::get_status,
        wallet::get_addresses,
        wallet::get_balances,
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
            TableSize,
            tokens::TokenHolder,
            tokens::PaginatedTokenHolders,
            PaginatedBlocks,
            PaginatedTransactions,
            PaginatedOutputs,
            PaginatedTokens,
            PaginatedEpochs,
        )
    )
)]
pub struct ApiDoc;

pub fn swagger_routes() -> Router<Arc<AppState>> {
    Router::new()
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
}

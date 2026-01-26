pub mod addresses;
pub mod blocks;
pub mod boxes;
pub mod mempool;
pub mod search;
pub mod stats;
pub mod status;
pub mod swagger;
pub mod tokens;
pub mod transactions;
pub mod wallet;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::AppState;

/// Build the API v1 router with all endpoints
pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Info
        .route("/info", get(stats::get_info))
        // Blocks
        .route("/blocks", get(blocks::get_blocks))
        .route("/blocks/:id", get(blocks::get_block))
        .route("/blocks/headers", get(blocks::get_headers))
        .route("/blocks/at/:height", get(blocks::get_block_at_height))
        .route("/blocks/byMiner/:address", get(blocks::get_blocks_by_miner))
        // Transactions
        .route("/transactions", get(transactions::get_transactions))
        .route("/transactions/:id", get(transactions::get_transaction))
        .route("/transactions/byBlock/:blockId", get(transactions::get_transactions_by_block))
        .route("/transactions/byAddress/:address", get(transactions::get_transactions_by_address))
        .route("/transactions/byInputsScriptTemplateHash/:hash", get(transactions::get_transactions_by_template))
        .route("/transactions/byGlobalIndex/stream", get(transactions::stream_transactions_by_gix))
        .route("/transactions/submit", post(transactions::submit_transaction))
        // Addresses
        .route("/addresses/:address", get(addresses::get_address))
        .route("/addresses/:address/balance/total", get(addresses::get_balance_total))
        .route("/addresses/:address/balance/confirmed", get(addresses::get_balance_confirmed))
        .route("/addresses/:address/transactions", get(addresses::get_address_transactions))
        // Boxes
        .route("/boxes/:boxId", get(boxes::get_box))
        .route("/boxes/byAddress/:address", get(boxes::get_boxes_by_address))
        .route("/boxes/unspent/byAddress/:address", get(boxes::get_unspent_boxes_by_address))
        .route("/boxes/unspent/unconfirmed/byAddress/:address", get(boxes::get_unspent_unconfirmed_by_address))
        .route("/boxes/unspent/all/byAddress/:address", get(boxes::get_all_unspent_by_address))
        .route("/boxes/byErgoTree/:ergoTree", get(boxes::get_boxes_by_ergo_tree))
        .route("/boxes/unspent/byErgoTree/:ergoTree", get(boxes::get_unspent_by_ergo_tree))
        .route("/boxes/byErgoTreeTemplateHash/:hash", get(boxes::get_boxes_by_template_hash))
        .route("/boxes/unspent/byErgoTreeTemplateHash/:hash", get(boxes::get_unspent_by_template_hash))
        .route("/boxes/byTokenId/:tokenId", get(boxes::get_boxes_by_token))
        .route("/boxes/unspent/byTokenId/:tokenId", get(boxes::get_unspent_by_token))
        .route("/boxes/unspent/stream", get(boxes::stream_unspent))
        .route("/boxes/unspent/byLastEpochs/stream", get(boxes::stream_unspent_by_epochs))
        .route("/boxes/unspent/byGlobalIndex/stream", get(boxes::stream_unspent_by_gix))
        .route("/boxes/byGlobalIndex/stream", get(boxes::stream_by_gix))
        .route("/boxes/byErgoTreeTemplateHash/:hash/stream", get(boxes::stream_by_template_hash))
        .route("/boxes/unspent/byErgoTreeTemplateHash/:hash/stream", get(boxes::stream_unspent_by_template_hash))
        .route("/boxes/search", post(boxes::search_boxes))
        .route("/boxes/unspent/search", post(boxes::search_unspent_boxes))
        .route("/boxes/unspent/search/union", post(boxes::search_unspent_union))
        // Tokens
        .route("/tokens", get(tokens::get_tokens))
        .route("/tokens/:tokenId", get(tokens::get_token))
        .route("/tokens/search", get(tokens::search_tokens))
        .route("/tokens/:tokenId/holders", get(tokens::get_token_holders))
        .route("/tokens/byAddress/:address", get(tokens::get_tokens_by_address))
        // Assets (alias for tokens by address)
        .route("/assets/byAddress/:address", get(tokens::get_tokens_by_address))
        // Mempool
        .route("/mempool/transactions", get(mempool::get_mempool_transactions))
        .route("/mempool/transactions/:txId", get(mempool::get_mempool_transaction))
        .route("/mempool/transactions/byAddress/:address", get(mempool::get_mempool_by_address))
        .route("/mempool/transactions/byErgoTree/:ergoTree", get(mempool::get_mempool_by_ergo_tree))
        .route("/mempool/size", get(mempool::get_mempool_size))
        // Stats
        .route("/stats", get(stats::get_stats))
        .route("/stats/network", get(stats::get_network_stats))
        // Epochs
        .route("/epochs", get(stats::get_epochs))
        .route("/epochs/:epochIndex", get(stats::get_epoch))
        // Search
        .route("/search", get(search::search))
        // ErgoTree utilities
        .route("/utils/ergoTreeToAddress/:ergoTree", get(search::ergo_tree_to_address))
        // Wallet (proxied to node)
        .route("/wallet/status", get(wallet::get_status))
        .route("/wallet/addresses", get(wallet::get_addresses))
        .route("/wallet/balances", get(wallet::get_balances))
        .route("/wallet/unlock", post(wallet::unlock))
        .route("/wallet/lock", post(wallet::lock))
        .route("/wallet/transaction/generate", post(wallet::generate_transaction))
        .route("/wallet/transaction/send", post(wallet::send_transaction))
}

//! Data models for the explorer API

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Block information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub id: String,
    pub parent_id: String,
    pub height: i64,
    pub timestamp: i64,
    pub difficulty: i64,
    pub block_size: i32,
    pub block_coins: i64,
    pub block_mining_time: Option<i64>,
    pub tx_count: i32,
    pub miner_address: Option<String>,
    pub miner_reward: i64,
    pub miner_name: Option<String>,
    pub main_chain: bool,
}

/// Block summary (lighter than full block)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BlockSummary {
    pub id: String,
    pub height: i64,
    pub timestamp: i64,
    pub tx_count: i32,
    pub miner_address: Option<String>,
    pub difficulty: i64,
    pub block_size: i32,
}

/// Transaction information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: String,
    pub block_id: String,
    pub inclusion_height: i64,
    pub timestamp: i64,
    pub index: i32,
    pub global_index: i64,
    pub coinbase: bool,
    pub size: i32,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub data_inputs: Vec<DataInput>,
}

/// Transaction summary
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSummary {
    pub id: String,
    pub timestamp: i64,
    pub inclusion_height: i64,
    pub input_count: i32,
    pub output_count: i32,
    pub size: i32,
}

/// Input (spent box reference)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub box_id: String,
    pub value: Option<i64>,
    pub address: Option<String>,
    pub tx_id: String,
    pub output_index: i32,
}

/// Data input (read-only box reference)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DataInput {
    pub box_id: String,
}

/// Box (UTXO) information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub box_id: String,
    pub tx_id: String,
    pub index: i32,
    pub value: i64,
    pub address: String,
    pub creation_height: i64,
    pub settlement_height: i64,
    pub ergo_tree: String,
    pub assets: Vec<BoxAsset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_registers: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spent_tx_id: Option<String>,
    pub main_chain: bool,
}

/// Asset in a box
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BoxAsset {
    pub token_id: String,
    pub amount: i64,
    pub index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<i32>,
}

/// Token information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub id: String,
    pub box_id: String,
    pub emission_amount: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<i32>,
    pub creation_height: i64,
}

/// Token summary
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenSummary {
    pub id: String,
    pub name: Option<String>,
    pub decimals: Option<i32>,
    pub emission_amount: i64,
}

/// Address information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddressInfo {
    pub address: String,
    pub tx_count: i64,
    pub balance: Balance,
    pub first_seen_height: Option<i64>,
    pub last_seen_height: Option<i64>,
}

/// Balance information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub nano_ergs: i64,
    pub tokens: Vec<TokenBalance>,
}

/// Token balance
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenBalance {
    pub token_id: String,
    pub amount: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<i32>,
}

/// Mempool transaction
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MempoolTransaction {
    pub id: String,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub data_inputs: Vec<DataInput>,
    pub size: i32,
    pub creation_timestamp: i64,
}

/// Network statistics
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStats {
    pub version: String,
    pub supply: i64,
    pub transaction_count: i64,
    pub circulating_supply: i64,
    pub block_count: i64,
    pub hash_rate: f64,
    pub difficulty: i64,
    pub block_time_avg: f64,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub entity_type: String,
    pub entity_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block: Option<BlockSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction: Option<TransactionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<AddressInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<TokenSummary>,
}

/// API info
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiInfo {
    pub version: String,
    pub network: String,
    pub indexed_height: i64,
    pub node_height: i64,
}

/// Pagination parameters
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    #[serde(default = "default_offset")]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_offset() -> i64 { 0 }
fn default_limit() -> i64 { 20 }

impl Default for Pagination {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 20,
        }
    }
}

/// Paginated response wrapper
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[aliases(
    PaginatedBlocks = PaginatedResponse<BlockSummary>,
    PaginatedTransactions = PaginatedResponse<TransactionSummary>,
    PaginatedOutputs = PaginatedResponse<Output>,
    PaginatedTokens = PaginatedResponse<TokenSummary>,
    PaginatedEpochs = PaginatedResponse<Epoch>
)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: i64,
}

/// Box search query
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BoxSearchQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ergo_tree_template_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registers: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constants: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<Vec<String>>,
}

/// Epoch info
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Epoch {
    pub index: i32,
    pub height_start: i64,
    pub height_end: i64,
    pub timestamp_start: i64,
    pub timestamp_end: Option<i64>,
    pub block_count: i32,
}

/// Table size info
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TableSize {
    pub name: String,
    pub row_count: i64,
    pub size_bytes: i64,
}

/// Node info (from connected node)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    pub name: String,
    pub app_version: String,
    pub full_height: Option<i64>,
    pub headers_height: Option<i64>,
    pub best_full_header_id: Option<String>,
    pub state_type: Option<String>,
    pub is_mining: Option<bool>,
    pub peers_count: Option<i32>,
    pub unconfirmed_count: Option<i32>,
    pub difficulty: Option<i64>,
}

/// Wallet status
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WalletStatus {
    pub initialized: bool,
    pub unlocked: bool,
    pub change_address: Option<String>,
    pub wallet_height: Option<i64>,
    pub error: Option<String>,
}

/// Wallet balance
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WalletBalance {
    pub height: i64,
    pub balance: i64,
    pub assets: Vec<WalletAsset>,
}

/// Wallet asset
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WalletAsset {
    pub token_id: String,
    pub amount: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<i32>,
}

/// Payment request for wallet
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequest {
    pub address: String,
    pub value: i64,
    #[serde(default)]
    pub assets: Vec<AssetAmount>,
}

/// Asset amount
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetAmount {
    pub token_id: String,
    pub amount: i64,
}

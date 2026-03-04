//! Synchronization service for indexing blockchain data

mod node_client;
mod processor;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, Semaphore};

use crate::db::Database;
pub use node_client::NodeClient;
use processor::BlockProcessor;

/// Maximum concurrent HTTP requests to nodes (configurable via SYNC_CONCURRENT_FETCHES)
fn max_concurrent_fetches() -> usize {
    std::env::var("SYNC_CONCURRENT_FETCHES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub is_syncing: bool,
    pub is_repairing: bool,
    pub local_height: i64,
    pub node_height: i64,
    pub sync_progress: f64,
    pub blocks_per_second: f64,
    pub eta_seconds: Option<i64>,
    pub last_block_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair_height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair_total_height: Option<i64>,
    pub connected_nodes: Vec<NodeStatus>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatus {
    pub url: String,
    pub connected: bool,
    pub height: Option<i64>,
    pub headers_height: Option<i64>,
    pub latency_ms: Option<u64>,
    pub last_used: Option<i64>,
    // Extended node info
    pub app_version: Option<String>,
    pub state_type: Option<String>,
    pub is_mining: Option<bool>,
    pub peers_count: Option<i32>,
    pub unconfirmed_count: Option<i32>,
    pub difficulty: Option<String>,
    pub max_peer_height: Option<i64>,
}

pub struct SyncService {
    nodes: Vec<NodeClient>,
    db: Database,
    batch_size: u32,
    processor: Mutex<BlockProcessor>,

    // Sync state
    is_syncing: AtomicBool,
    is_repairing: AtomicBool,
    local_height: AtomicI64,
    node_height: AtomicI64,
    blocks_synced: AtomicU64,
    sync_start_time: AtomicU64,
    repair_height: AtomicI64,
    repair_total_height: AtomicI64,
    last_error: RwLock<Option<String>>,
    node_statuses: RwLock<Vec<NodeStatus>>,
}

impl SyncService {
    pub fn new(
        node_urls: Vec<String>,
        db: Database,
        batch_size: u32,
        api_key: Option<String>,
    ) -> Self {
        let nodes: Vec<NodeClient> = node_urls
            .iter()
            .map(|url| NodeClient::new(url.clone(), api_key.clone()))
            .collect();

        let node_statuses: Vec<NodeStatus> = node_urls
            .iter()
            .map(|url| NodeStatus {
                url: url.clone(),
                connected: false,
                height: None,
                headers_height: None,
                latency_ms: None,
                last_used: None,
                app_version: None,
                state_type: None,
                is_mining: None,
                peers_count: None,
                unconfirmed_count: None,
                difficulty: None,
                max_peer_height: None,
            })
            .collect();

        Self {
            processor: Mutex::new(BlockProcessor::new(db.clone())),
            nodes,
            db,
            batch_size,
            is_syncing: AtomicBool::new(false),
            is_repairing: AtomicBool::new(false),
            local_height: AtomicI64::new(-1),
            repair_height: AtomicI64::new(0),
            repair_total_height: AtomicI64::new(0),
            node_height: AtomicI64::new(0),
            blocks_synced: AtomicU64::new(0),
            sync_start_time: AtomicU64::new(0),
            last_error: RwLock::new(None),
            node_statuses: RwLock::new(node_statuses),
        }
    }

    pub async fn run(&self, interval_secs: u64) {
        tracing::info!("Starting sync service with {} node(s)", self.nodes.len());

        loop {
            if let Err(e) = self.sync_once().await {
                tracing::error!("Sync error: {}", e);
                *self.last_error.write().await = Some(e.to_string());
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
        }
    }

    async fn sync_once(&self) -> Result<()> {
        // Skip normal sync while repair is running
        if self.is_repairing.load(Ordering::SeqCst) {
            return Ok(());
        }

        // Check node health and get best height
        let (_best_node_idx, node_height) = self.find_best_node().await?;

        self.node_height.store(node_height, Ordering::SeqCst);

        // Get local height
        let local_height = self.db.get_sync_height()?;
        self.local_height.store(local_height, Ordering::SeqCst);

        if local_height >= node_height {
            tracing::debug!("Already synced to height {}", local_height);
            return Ok(());
        }

        // Start syncing
        self.is_syncing.store(true, Ordering::SeqCst);
        self.sync_start_time.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Ordering::SeqCst,
        );
        self.blocks_synced.store(0, Ordering::SeqCst);
        *self.last_error.write().await = None;

        let start_height = local_height + 1;
        let end_height = node_height;
        let total_blocks = (end_height - start_height + 1) as u64;

        tracing::info!(
            "Syncing blocks {} to {} ({} blocks)",
            start_height,
            end_height,
            total_blocks
        );

        // Checkpoint frequency: checkpoint every N batches (configurable via SYNC_CHECKPOINT_INTERVAL)
        let checkpoint_interval: usize = std::env::var("SYNC_CHECKPOINT_INTERVAL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        // Sync in batches with parallel fetching across nodes
        let mut current_height = start_height;
        let mut batch_count: usize = 0;

        while current_height <= end_height {
            let batch_end = std::cmp::min(current_height + self.batch_size as i64 - 1, end_height);
            let batch_size = (batch_end - current_height + 1) as usize;

            // Parallel fetch using multiple nodes
            let blocks = self
                .fetch_blocks_parallel(current_height, batch_end)
                .await?;

            // Process blocks sequentially (must maintain order)
            let mut processor = self.processor.lock().await;
            for block in blocks {
                processor.process_block(&block)?;
            }
            drop(processor);

            batch_count += 1;

            // Checkpoint periodically to flush to disk and free memory
            if batch_count % checkpoint_interval == 0 {
                if let Err(e) = self.db.checkpoint() {
                    tracing::warn!("Checkpoint failed: {}", e);
                }
            }

            self.blocks_synced
                .fetch_add(batch_size as u64, Ordering::SeqCst);
            self.local_height.store(batch_end, Ordering::SeqCst);

            let progress = self.blocks_synced.load(Ordering::SeqCst) as f64 / total_blocks as f64;
            tracing::info!(
                "Synced to height {} ({:.1}%)",
                batch_end,
                progress * 100.0
            );

            current_height = batch_end + 1;
        }

        // Final checkpoint at end of sync
        if let Err(e) = self.db.checkpoint() {
            tracing::warn!("Final checkpoint failed: {}", e);
        }

        self.is_syncing.store(false, Ordering::SeqCst);
        tracing::info!("Sync complete at height {}", end_height);

        Ok(())
    }

    async fn find_best_node(&self) -> Result<(usize, i64)> {
        let mut best_idx = 0;
        let mut best_height: i64 = 0;
        let mut statuses = self.node_statuses.write().await;

        for (idx, node) in self.nodes.iter().enumerate() {
            let start = std::time::Instant::now();
            match node.get_info().await {
                Ok(info) => {
                    let latency = start.elapsed().as_millis() as u64;
                    let height = info.full_height.unwrap_or(0);

                    statuses[idx].connected = true;
                    statuses[idx].height = Some(height);
                    statuses[idx].headers_height = info.headers_height;
                    statuses[idx].latency_ms = Some(latency);
                    statuses[idx].last_used = Some(chrono::Utc::now().timestamp());
                    // Extended info
                    statuses[idx].app_version = info.app_version.clone();
                    statuses[idx].state_type = info.state_type.clone();
                    statuses[idx].is_mining = info.is_mining;
                    statuses[idx].peers_count = info.peers_count;
                    statuses[idx].unconfirmed_count = info.unconfirmed_count;
                    statuses[idx].difficulty = info.difficulty.map(|d| d.to_string());
                    statuses[idx].max_peer_height = info.max_peer_height;

                    if height > best_height {
                        best_height = height;
                        best_idx = idx;
                    }
                }
                Err(e) => {
                    tracing::warn!("Node {} unreachable: {}", node.url, e);
                    statuses[idx].connected = false;
                    statuses[idx].height = None;
                    statuses[idx].headers_height = None;
                    statuses[idx].latency_ms = None;
                    statuses[idx].app_version = None;
                    statuses[idx].state_type = None;
                    statuses[idx].is_mining = None;
                    statuses[idx].peers_count = None;
                    statuses[idx].unconfirmed_count = None;
                    statuses[idx].difficulty = None;
                    statuses[idx].max_peer_height = None;
                }
            }
        }

        if best_height == 0 {
            anyhow::bail!("No nodes available");
        }

        Ok((best_idx, best_height))
    }

    async fn fetch_blocks_parallel(
        &self,
        start_height: i64,
        end_height: i64,
    ) -> Result<Vec<serde_json::Value>> {
        let heights: Vec<i64> = (start_height..=end_height).collect();
        let num_nodes = self.nodes.len();

        // Use a semaphore to limit concurrent requests
        let semaphore = Arc::new(Semaphore::new(max_concurrent_fetches()));

        // Create tasks with concurrency control
        let tasks: Vec<_> = heights
            .iter()
            .enumerate()
            .map(|(i, &height)| {
                let node_idx = i % num_nodes;
                let node = self.nodes[node_idx].clone();
                let sem = semaphore.clone();

                async move {
                    // Acquire semaphore permit before making request
                    let _permit = sem.acquire().await.map_err(|e| anyhow::anyhow!("Semaphore error: {}", e))?;

                    // Retry logic for transient failures
                    let mut last_error = None;
                    for attempt in 0..3 {
                        if attempt > 0 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(500 * (1 << attempt))).await;
                        }

                        match async {
                            let header_ids = node.get_block_ids_at_height(height).await?;
                            if header_ids.is_empty() {
                                anyhow::bail!("No block at height {}", height);
                            }
                            node.get_block(&header_ids[0]).await
                        }.await {
                            Ok(block) => return Ok::<(i64, serde_json::Value), anyhow::Error>((height, block)),
                            Err(e) => {
                                last_error = Some(e);
                                continue;
                            }
                        }
                    }

                    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown fetch error")))
                }
            })
            .collect();

        // Execute with controlled concurrency using buffered stream
        let results: Vec<Result<(i64, serde_json::Value), anyhow::Error>> =
            stream::iter(tasks)
                .buffer_unordered(max_concurrent_fetches())
                .collect()
                .await;

        // Collect and sort by height
        let mut blocks: Vec<(i64, serde_json::Value)> = Vec::new();
        for result in results {
            match result {
                Ok((height, block)) => blocks.push((height, block)),
                Err(e) => {
                    tracing::error!("Failed to fetch block: {}", e);
                    return Err(e);
                }
            }
        }

        blocks.sort_by_key(|(h, _)| *h);
        Ok(blocks.into_iter().map(|(_, b)| b).collect())
    }

    pub async fn get_status(&self) -> SyncStatus {
        let is_syncing = self.is_syncing.load(Ordering::SeqCst);
        let local_height = self.local_height.load(Ordering::SeqCst);
        let node_height = self.node_height.load(Ordering::SeqCst);
        let blocks_synced = self.blocks_synced.load(Ordering::SeqCst);
        let sync_start = self.sync_start_time.load(Ordering::SeqCst);

        let sync_progress = if node_height > 0 {
            (local_height as f64 / node_height as f64).min(1.0)
        } else {
            0.0
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let (blocks_per_second, eta_seconds) = if is_syncing && sync_start > 0 {
            let elapsed = (now - sync_start).max(1);
            let bps = blocks_synced as f64 / elapsed as f64;
            let remaining = node_height - local_height;
            let eta = if bps > 0.0 {
                Some((remaining as f64 / bps) as i64)
            } else {
                None
            };
            (bps, eta)
        } else {
            (0.0, None)
        };

        let error = self.last_error.read().await.clone();
        let connected_nodes = self.node_statuses.read().await.clone();

        let is_repairing = self.is_repairing.load(Ordering::SeqCst);

        let (repair_height, repair_total_height) = if is_repairing {
            (
                Some(self.repair_height.load(Ordering::SeqCst)),
                Some(self.repair_total_height.load(Ordering::SeqCst)),
            )
        } else {
            (None, None)
        };

        SyncStatus {
            is_syncing,
            is_repairing,
            local_height,
            node_height,
            sync_progress,
            blocks_per_second,
            eta_seconds,
            repair_height,
            repair_total_height,
            last_block_time: None, // TODO: track this
            connected_nodes,
            error,
        }
    }

    pub fn get_primary_node(&self) -> Option<&NodeClient> {
        self.nodes.first()
    }

    /// Repair box_assets and tokens tables by clearing and re-extracting from all blocks.
    /// Uses a lightweight extraction that ONLY processes assets and tokens — skips all
    /// block/tx/box/input processing since those tables are intact.
    pub async fn repair_assets(&self) -> Result<()> {
        if self.is_syncing.load(Ordering::SeqCst) {
            anyhow::bail!("Cannot repair while sync is in progress");
        }
        if self.is_repairing.load(Ordering::SeqCst) {
            anyhow::bail!("Repair is already in progress");
        }

        self.is_repairing.store(true, Ordering::SeqCst);
        self.is_syncing.store(true, Ordering::SeqCst);
        self.blocks_synced.store(0, Ordering::SeqCst);
        self.sync_start_time.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Ordering::SeqCst,
        );

        let max_height = self.db.get_sync_height()?;
        self.repair_height.store(0, Ordering::SeqCst);
        self.repair_total_height.store(max_height, Ordering::SeqCst);
        tracing::info!("Starting asset repair for {} blocks", max_height);

        // Delete only the affected tables
        self.db.execute_batch(
            "DELETE FROM box_assets;
             DELETE FROM tokens;"
        )?;
        self.db.checkpoint()?;
        tracing::info!("Cleared box_assets and tokens tables. Re-extracting from blocks...");

        // Use large batches for repair — 100 blocks per batch
        let repair_batch_size: i64 = std::env::var("REPAIR_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);

        let mut current_height: i64 = 1;
        let mut box_asset_id: i64 = 0;
        let mut batch_count: usize = 0;
        let mut total_assets: i64 = 0;
        let mut total_tokens: i64 = 0;

        while current_height <= max_height {
            let batch_end = std::cmp::min(current_height + repair_batch_size - 1, max_height);

            let blocks = self
                .fetch_blocks_parallel(current_height, batch_end)
                .await?;

            // Lightweight extraction: only box_assets and tokens
            // Collect all inserts first, then batch-execute in a transaction
            let mut asset_inserts: Vec<(i64, String, String, i64, i32)> = Vec::new();
            let mut token_inserts: Vec<(String, String, i64, Option<String>, Option<String>, Option<String>, Option<i32>, i64)> = Vec::new();

            for block in &blocks {
                let header = match block.get("header") {
                    Some(h) => h,
                    None => continue,
                };
                let height = header.get("height").and_then(|v| v.as_i64()).unwrap_or(0);
                let block_txs = match block.get("blockTransactions") {
                    Some(bt) => bt,
                    None => continue,
                };
                let transactions = match block_txs.get("transactions").and_then(|t| t.as_array()) {
                    Some(txs) => txs,
                    None => continue,
                };

                for tx in transactions {
                    let inputs = tx.get("inputs").and_then(|v| v.as_array());
                    let first_input_box_id = inputs
                        .and_then(|inp| inp.first())
                        .and_then(|i| i.get("boxId"))
                        .and_then(|v| v.as_str());

                    let outputs = match tx.get("outputs").and_then(|v| v.as_array()) {
                        Some(o) => o,
                        None => continue,
                    };
                    for output in outputs {
                        let box_id = output.get("boxId").and_then(|v| v.as_str()).unwrap_or("");
                        let additional_registers = output.get("additionalRegisters");
                        let assets = output.get("assets").and_then(|v| v.as_array());

                        if let Some(assets) = assets {
                            for (asset_idx, asset) in assets.iter().enumerate() {
                                let token_id = match asset.get("tokenId").and_then(|v| v.as_str()) {
                                    Some(id) => id,
                                    None => continue,
                                };
                                let amount = asset.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);

                                box_asset_id += 1;
                                asset_inserts.push((
                                    box_asset_id,
                                    box_id.to_string(),
                                    token_id.to_string(),
                                    amount,
                                    asset_idx as i32,
                                ));
                                total_assets += 1;

                                // Check for minting
                                if asset_idx == 0 {
                                    if let Some(first_box) = first_input_box_id {
                                        if first_box == token_id {
                                            let (name, description, token_type, decimals) =
                                                processor::extract_token_metadata_pub(additional_registers);
                                            token_inserts.push((
                                                token_id.to_string(),
                                                box_id.to_string(),
                                                amount,
                                                name,
                                                description,
                                                token_type,
                                                decimals,
                                                height,
                                            ));
                                            total_tokens += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Batch insert in a single transaction
            self.db.execute_transaction(|conn| {
                for (id, box_id, token_id, amount, asset_index) in &asset_inserts {
                    conn.execute(
                        "INSERT INTO box_assets (id, box_id, token_id, amount, asset_index) VALUES (?, ?, ?, ?, ?)",
                        duckdb::params![id, box_id, token_id, amount, asset_index],
                    )?;
                }
                for (token_id, box_id, amount, name, description, token_type, decimals, height) in &token_inserts {
                    conn.execute(
                        "INSERT INTO tokens (token_id, box_id, emission_amount, name, description, token_type, decimals, creation_height) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                        duckdb::params![token_id, box_id, amount, name, description, token_type, decimals, height],
                    )?;
                }
                Ok(())
            })?;

            batch_count += 1;
            if batch_count % 10 == 0 {
                if let Err(e) = self.db.checkpoint() {
                    tracing::warn!("Checkpoint failed during repair: {}", e);
                }
            }

            let blocks_done = (batch_end - current_height + 1) as u64;
            self.blocks_synced.fetch_add(blocks_done, Ordering::SeqCst);
            self.repair_height.store(batch_end, Ordering::SeqCst);

            let progress = batch_end as f64 / max_height as f64;
            if batch_count % 50 == 0 {
                tracing::info!(
                    "Repair: {}/{} ({:.1}%) - {} assets, {} tokens so far",
                    batch_end, max_height, progress * 100.0, total_assets, total_tokens
                );
            }

            current_height = batch_end + 1;
        }

        if let Err(e) = self.db.checkpoint() {
            tracing::warn!("Final checkpoint failed during repair: {}", e);
        }

        // Reset processor counters from the now-populated tables
        {
            let mut processor = self.processor.lock().await;
            *processor = BlockProcessor::new(self.db.clone());
        }

        self.is_repairing.store(false, Ordering::SeqCst);
        self.is_syncing.store(false, Ordering::SeqCst);
        self.local_height.store(max_height, Ordering::SeqCst);
        tracing::info!(
            "Asset repair complete. {} blocks, {} assets, {} tokens.",
            max_height, total_assets, total_tokens
        );

        Ok(())
    }
}

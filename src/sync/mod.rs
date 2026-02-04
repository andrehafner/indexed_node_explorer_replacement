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
    pub local_height: i64,
    pub node_height: i64,
    pub sync_progress: f64,
    pub blocks_per_second: f64,
    pub eta_seconds: Option<i64>,
    pub last_block_time: Option<i64>,
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
    local_height: AtomicI64,
    node_height: AtomicI64,
    blocks_synced: AtomicU64,
    sync_start_time: AtomicU64,
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
            local_height: AtomicI64::new(-1),
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

        SyncStatus {
            is_syncing,
            local_height,
            node_height,
            sync_progress,
            blocks_per_second,
            eta_seconds,
            last_block_time: None, // TODO: track this
            connected_nodes,
            error,
        }
    }

    pub fn get_primary_node(&self) -> Option<&NodeClient> {
        self.nodes.first()
    }
}

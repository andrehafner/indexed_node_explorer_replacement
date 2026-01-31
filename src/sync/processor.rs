//! Block processor for indexing blockchain data

use anyhow::{Context, Result};
use duckdb::{params, Connection};
use serde_json::Value;

use crate::db::Database;
use crate::utils::ergo_tree;

pub struct BlockProcessor {
    db: Database,
    global_tx_index: i64,
    global_box_index: i64,
    global_block_index: i64,
    box_asset_id: i64,
    input_id: i64,
    data_input_id: i64,
}

impl BlockProcessor {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            global_tx_index: 0,
            global_box_index: 0,
            global_block_index: 0,
            box_asset_id: 0,
            input_id: 0,
            data_input_id: 0,
        }
    }

    pub fn process_block(&mut self, block: &Value) -> Result<()> {
        let header = block.get("header").context("Missing header")?;
        let block_txs = block.get("blockTransactions").context("Missing blockTransactions")?;
        let transactions = block_txs
            .get("transactions")
            .and_then(|t| t.as_array())
            .context("Missing transactions array")?;

        // Extract header data
        let block_id = header.get("id").and_then(|v| v.as_str()).context("Missing block id")?.to_string();
        let parent_id = header.get("parentId").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let height = header.get("height").and_then(|v| v.as_i64()).context("Missing height")?;
        let timestamp = header.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
        let difficulty = header
            .get("difficulty")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let miner_pk = header.get("minerPk").and_then(|v| v.as_str()).unwrap_or("");

        // Calculate block metrics
        let tx_count = transactions.len() as i32;
        let block_size: i32 = transactions
            .iter()
            .filter_map(|tx| tx.get("size").and_then(|s| s.as_i64()))
            .sum::<i64>() as i32;

        let block_coins: i64 = transactions
            .iter()
            .map(|tx| {
                tx.get("outputs")
                    .and_then(|o| o.as_array())
                    .map(|outputs| {
                        outputs
                            .iter()
                            .filter_map(|o| o.get("value").and_then(|v| v.as_i64()))
                            .sum::<i64>()
                    })
                    .unwrap_or(0)
            })
            .sum();

        // Derive miner address from minerPk
        let miner_address = if !miner_pk.is_empty() {
            ergo_tree::miner_pk_to_address(miner_pk)
        } else {
            None
        };

        // Get miner reward from first transaction (coinbase)
        let miner_reward = transactions
            .first()
            .and_then(|tx| tx.get("outputs"))
            .and_then(|o| o.as_array())
            .and_then(|outputs| outputs.first())
            .and_then(|o| o.get("value"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        self.global_block_index += 1;
        let global_block_index = self.global_block_index;

        // Collect all operations for this block
        let mut collected = CollectedOps::new();

        // Collect block data
        collected.block = Some(BlockData {
            block_id: block_id.clone(),
            parent_id,
            height,
            timestamp,
            difficulty,
            block_size,
            block_coins,
            tx_count,
            miner_address,
            miner_reward,
            global_index: global_block_index,
        });

        // Process transactions and collect operations
        for (tx_idx, tx) in transactions.iter().enumerate() {
            self.collect_transaction_ops(
                tx,
                &block_id,
                height,
                timestamp,
                tx_idx as i32,
                &mut collected,
            )?;
        }

        // Execute all operations in a single transaction
        let update_stats = height % 100 == 0;
        self.db.execute_transaction(|conn| {
            // Insert block
            if let Some(ref b) = collected.block {
                conn.execute(
                    "INSERT INTO blocks (
                        block_id, parent_id, height, timestamp, difficulty, block_size,
                        block_coins, tx_count, miner_address, miner_reward, main_chain, global_index
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, TRUE, ?)
                    ON CONFLICT (block_id) DO UPDATE SET main_chain = TRUE",
                    params![
                        b.block_id,
                        b.parent_id,
                        b.height,
                        b.timestamp,
                        b.difficulty,
                        b.block_size,
                        b.block_coins,
                        b.tx_count,
                        b.miner_address,
                        b.miner_reward,
                        b.global_index
                    ],
                )?;
            }

            // Insert all transactions
            for tx in &collected.transactions {
                conn.execute(
                    "INSERT INTO transactions (
                        tx_id, block_id, inclusion_height, timestamp, index_in_block,
                        global_index, coinbase, size, input_count, output_count
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT (tx_id) DO NOTHING",
                    params![
                        tx.tx_id,
                        tx.block_id,
                        tx.inclusion_height,
                        tx.timestamp,
                        tx.index_in_block,
                        tx.global_index,
                        tx.coinbase,
                        tx.size,
                        tx.input_count,
                        tx.output_count
                    ],
                )?;
            }

            // Insert all boxes
            for b in &collected.boxes {
                conn.execute(
                    "INSERT INTO boxes (
                        box_id, tx_id, output_index, ergo_tree, ergo_tree_template_hash,
                        address, value, creation_height, settlement_height, global_index,
                        additional_registers
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT (box_id) DO NOTHING",
                    params![
                        b.box_id,
                        b.tx_id,
                        b.output_index,
                        b.ergo_tree,
                        b.template_hash,
                        b.address,
                        b.value,
                        b.creation_height,
                        b.settlement_height,
                        b.global_index,
                        b.registers_json
                    ],
                )?;
            }

            // Update spent boxes and insert inputs
            for input in &collected.inputs {
                conn.execute(
                    "UPDATE boxes SET spent_tx_id = ?, spent_index = ?, spent_height = ? WHERE box_id = ?",
                    params![input.tx_id, input.input_index, input.height, input.box_id],
                )?;
                conn.execute(
                    "INSERT INTO inputs (id, tx_id, box_id, input_index, proof_bytes)
                     VALUES (?, ?, ?, ?, ?)
                     ON CONFLICT DO NOTHING",
                    params![input.id, input.tx_id, input.box_id, input.input_index, input.proof_bytes],
                )?;
            }

            // Insert data inputs
            for di in &collected.data_inputs {
                conn.execute(
                    "INSERT INTO data_inputs (id, tx_id, box_id, input_index)
                     VALUES (?, ?, ?, ?)
                     ON CONFLICT DO NOTHING",
                    params![di.id, di.tx_id, di.box_id, di.input_index],
                )?;
            }

            // Insert box assets
            for asset in &collected.box_assets {
                conn.execute(
                    "INSERT INTO box_assets (id, box_id, token_id, amount, asset_index)
                     VALUES (?, ?, ?, ?, ?)
                     ON CONFLICT DO NOTHING",
                    params![asset.id, asset.box_id, asset.token_id, asset.amount, asset.asset_index],
                )?;
            }

            // Insert tokens (new mints only)
            for token in &collected.tokens {
                conn.execute(
                    "INSERT INTO tokens (token_id, box_id, emission_amount, name, description, decimals, creation_height)
                     VALUES (?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT (token_id) DO NOTHING",
                    params![
                        token.token_id,
                        token.box_id,
                        token.emission_amount,
                        token.name,
                        token.description,
                        token.decimals,
                        token.creation_height
                    ],
                )?;
            }

            // Update address stats
            let now = chrono::Utc::now().timestamp();
            for addr in &collected.addresses {
                conn.execute(
                    "INSERT INTO address_stats (address, tx_count, first_seen_height, last_seen_height, updated_at)
                     VALUES (?, 1, ?, ?, ?)
                     ON CONFLICT (address) DO UPDATE SET
                        tx_count = address_stats.tx_count + 1,
                        last_seen_height = EXCLUDED.last_seen_height,
                        updated_at = EXCLUDED.updated_at",
                    params![addr.address, addr.height, addr.height, now],
                )?;
            }

            // Update network stats periodically (every 100 blocks)
            if update_stats {
                if let Some(ref b) = collected.block {
                    update_network_stats_sync(conn, b.height, b.timestamp, b.difficulty)?;
                }
            }

            Ok(())
        })?;

        Ok(())
    }

    fn collect_transaction_ops(
        &mut self,
        tx: &Value,
        block_id: &str,
        height: i64,
        timestamp: i64,
        tx_idx: i32,
        collected: &mut CollectedOps,
    ) -> Result<()> {
        let tx_id = tx.get("id").and_then(|v| v.as_str()).context("Missing tx id")?.to_string();
        let inputs = tx.get("inputs").and_then(|v| v.as_array());
        let outputs = tx.get("outputs").and_then(|v| v.as_array()).context("Missing outputs")?;
        let data_inputs = tx.get("dataInputs").and_then(|v| v.as_array());
        let size = tx.get("size").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        let input_count = inputs.map(|i| i.len()).unwrap_or(0) as i32;
        let output_count = outputs.len() as i32;
        let coinbase = input_count == 0 || tx_idx == 0;

        // Get first input's box_id for minting detection
        let first_input_box_id = inputs
            .and_then(|i| i.first())
            .and_then(|inp| inp.get("boxId"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        self.global_tx_index += 1;

        collected.transactions.push(TransactionData {
            tx_id: tx_id.clone(),
            block_id: block_id.to_string(),
            inclusion_height: height,
            timestamp,
            index_in_block: tx_idx,
            global_index: self.global_tx_index,
            coinbase,
            size,
            input_count,
            output_count,
        });

        // Collect inputs
        if let Some(inputs) = inputs {
            for (input_idx, input) in inputs.iter().enumerate() {
                self.collect_input(input, &tx_id, height, input_idx as i32, collected)?;
            }
        }

        // Collect data inputs
        if let Some(data_inputs) = data_inputs {
            for (di_idx, data_input) in data_inputs.iter().enumerate() {
                self.collect_data_input(data_input, &tx_id, di_idx as i32, collected)?;
            }
        }

        // Collect outputs
        for (output_idx, output) in outputs.iter().enumerate() {
            self.collect_output(output, &tx_id, height, output_idx as i32, first_input_box_id.as_deref(), collected)?;
        }

        Ok(())
    }

    fn collect_input(
        &mut self,
        input: &Value,
        tx_id: &str,
        height: i64,
        input_idx: i32,
        collected: &mut CollectedOps,
    ) -> Result<()> {
        let box_id = input.get("boxId").and_then(|v| v.as_str()).context("Missing boxId")?;
        let proof_bytes = input
            .get("spendingProof")
            .and_then(|sp| sp.get("proofBytes"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        self.input_id += 1;

        collected.inputs.push(InputData {
            id: self.input_id,
            tx_id: tx_id.to_string(),
            box_id: box_id.to_string(),
            height,
            input_index: input_idx,
            proof_bytes: proof_bytes.to_string(),
        });

        Ok(())
    }

    fn collect_data_input(
        &mut self,
        data_input: &Value,
        tx_id: &str,
        input_idx: i32,
        collected: &mut CollectedOps,
    ) -> Result<()> {
        let box_id = data_input.get("boxId").and_then(|v| v.as_str()).context("Missing boxId")?;

        self.data_input_id += 1;

        collected.data_inputs.push(DataInputData {
            id: self.data_input_id,
            tx_id: tx_id.to_string(),
            box_id: box_id.to_string(),
            input_index: input_idx,
        });

        Ok(())
    }

    fn collect_output(
        &mut self,
        output: &Value,
        tx_id: &str,
        height: i64,
        output_idx: i32,
        first_input_box_id: Option<&str>,
        collected: &mut CollectedOps,
    ) -> Result<()> {
        let box_id = output.get("boxId").and_then(|v| v.as_str()).context("Missing boxId")?;
        let value = output.get("value").and_then(|v| v.as_i64()).unwrap_or(0);
        let ergo_tree_hex = output.get("ergoTree").and_then(|v| v.as_str()).unwrap_or("");
        let creation_height = output.get("creationHeight").and_then(|v| v.as_i64()).unwrap_or(height);
        let additional_registers = output.get("additionalRegisters");
        let assets = output.get("assets").and_then(|v| v.as_array());

        let address = ergo_tree::ergo_tree_to_address(ergo_tree_hex).unwrap_or_else(|| ergo_tree_hex.to_string());
        let template_hash = Some(ergo_tree::ergo_tree_template_hash(ergo_tree_hex));

        self.global_box_index += 1;

        let registers_json = additional_registers.map(|r| r.to_string());

        collected.boxes.push(BoxData {
            box_id: box_id.to_string(),
            tx_id: tx_id.to_string(),
            output_index: output_idx,
            ergo_tree: ergo_tree_hex.to_string(),
            template_hash,
            address: address.clone(),
            value,
            creation_height,
            settlement_height: height,
            global_index: self.global_box_index,
            registers_json,
        });

        // Collect address for stats update
        collected.addresses.push(AddressData {
            address,
            height,
        });

        // Collect assets
        if let Some(assets) = assets {
            for (asset_idx, asset) in assets.iter().enumerate() {
                self.collect_asset(
                    asset,
                    box_id,
                    height,
                    asset_idx as i32,
                    first_input_box_id,
                    additional_registers,
                    collected,
                )?;
            }
        }

        Ok(())
    }

    fn collect_asset(
        &mut self,
        asset: &Value,
        box_id: &str,
        height: i64,
        asset_idx: i32,
        first_input_box_id: Option<&str>,
        registers: Option<&Value>,
        collected: &mut CollectedOps,
    ) -> Result<()> {
        let token_id = asset.get("tokenId").and_then(|v| v.as_str()).context("Missing tokenId")?;
        let amount = asset.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);

        self.box_asset_id += 1;

        collected.box_assets.push(BoxAssetData {
            id: self.box_asset_id,
            box_id: box_id.to_string(),
            token_id: token_id.to_string(),
            amount,
            asset_index: asset_idx,
        });

        // Check if this is a minting transaction
        if asset_idx == 0 {
            let is_minting = first_input_box_id.map(|id| id == token_id).unwrap_or(false);
            if is_minting {
                let (name, description, decimals) = extract_token_metadata(registers);
                collected.tokens.push(TokenData {
                    token_id: token_id.to_string(),
                    box_id: box_id.to_string(),
                    emission_amount: amount,
                    name,
                    description,
                    decimals,
                    creation_height: height,
                });
            }
        }

        Ok(())
    }
}

// Helper function to update network stats within a transaction
fn update_network_stats_sync(conn: &Connection, height: i64, timestamp: i64, difficulty: i64) -> Result<()> {
    // Use a simpler calculation that doesn't require full table scan
    // Just get the block's values directly
    let block_info: Option<(i64, i64)> = conn
        .query_row(
            "SELECT COALESCE(block_size, 0), COALESCE(block_coins, 0) FROM blocks WHERE height = ?",
            [height],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    let (block_size, block_coins) = block_info.unwrap_or((0, 0));

    // Estimate total coins from recent blocks rather than full scan
    let estimated_supply = height * 75_000_000_000i64; // ~75 ERG average reward per block

    // Simple hashrate estimate
    let hashrate = difficulty as f64 / 120.0;

    conn.execute(
        "INSERT INTO network_stats (
            timestamp, height, difficulty, block_size, block_coins, total_coins, hashrate
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (timestamp) DO UPDATE SET
            height = EXCLUDED.height, difficulty = EXCLUDED.difficulty, hashrate = EXCLUDED.hashrate",
        params![timestamp, height, difficulty, block_size, block_coins, estimated_supply, hashrate],
    )?;

    Ok(())
}

// Data structures for collecting batch operations

struct CollectedOps {
    block: Option<BlockData>,
    transactions: Vec<TransactionData>,
    boxes: Vec<BoxData>,
    inputs: Vec<InputData>,
    data_inputs: Vec<DataInputData>,
    box_assets: Vec<BoxAssetData>,
    tokens: Vec<TokenData>,
    addresses: Vec<AddressData>,
}

impl CollectedOps {
    fn new() -> Self {
        Self {
            block: None,
            transactions: Vec::with_capacity(16),
            boxes: Vec::with_capacity(64),
            inputs: Vec::with_capacity(64),
            data_inputs: Vec::with_capacity(8),
            box_assets: Vec::with_capacity(32),
            tokens: Vec::new(),
            addresses: Vec::with_capacity(64),
        }
    }
}

struct BlockData {
    block_id: String,
    parent_id: String,
    height: i64,
    timestamp: i64,
    difficulty: i64,
    block_size: i32,
    block_coins: i64,
    tx_count: i32,
    miner_address: Option<String>,
    miner_reward: i64,
    global_index: i64,
}

struct TransactionData {
    tx_id: String,
    block_id: String,
    inclusion_height: i64,
    timestamp: i64,
    index_in_block: i32,
    global_index: i64,
    coinbase: bool,
    size: i32,
    input_count: i32,
    output_count: i32,
}

struct BoxData {
    box_id: String,
    tx_id: String,
    output_index: i32,
    ergo_tree: String,
    template_hash: Option<String>,
    address: String,
    value: i64,
    creation_height: i64,
    settlement_height: i64,
    global_index: i64,
    registers_json: Option<String>,
}

struct InputData {
    id: i64,
    tx_id: String,
    box_id: String,
    height: i64,
    input_index: i32,
    proof_bytes: String,
}

struct DataInputData {
    id: i64,
    tx_id: String,
    box_id: String,
    input_index: i32,
}

struct BoxAssetData {
    id: i64,
    box_id: String,
    token_id: String,
    amount: i64,
    asset_index: i32,
}

struct TokenData {
    token_id: String,
    box_id: String,
    emission_amount: i64,
    name: Option<String>,
    description: Option<String>,
    decimals: Option<i32>,
    creation_height: i64,
}

struct AddressData {
    address: String,
    height: i64,
}

/// Extract token metadata from box registers
fn extract_token_metadata(registers: Option<&Value>) -> (Option<String>, Option<String>, Option<i32>) {
    let registers = match registers {
        Some(r) => r,
        None => return (None, None, None),
    };

    let name = registers
        .get("R4")
        .and_then(|v| v.as_str())
        .and_then(decode_sigma_string);

    let description = registers
        .get("R5")
        .and_then(|v| v.as_str())
        .and_then(decode_sigma_string);

    let decimals = registers
        .get("R6")
        .and_then(|v| v.as_str())
        .and_then(decode_sigma_int);

    (name, description, decimals)
}

/// Decode a Sigma-encoded Coll[Byte] (type 0e) to a UTF-8 string
fn decode_sigma_string(hex: &str) -> Option<String> {
    let bytes = hex::decode(hex).ok()?;

    if bytes.is_empty() {
        return None;
    }

    if bytes[0] == 0x0e {
        let (len, offset) = decode_vlq(&bytes[1..])?;
        if offset + 1 + len > bytes.len() {
            return None;
        }
        let string_bytes = &bytes[1 + offset..1 + offset + len];
        return String::from_utf8(string_bytes.to_vec()).ok();
    }

    String::from_utf8(bytes).ok()
}

/// Decode a Sigma-encoded integer or Coll[Byte] containing decimal digits
fn decode_sigma_int(hex: &str) -> Option<i32> {
    let bytes = hex::decode(hex).ok()?;

    if bytes.is_empty() {
        return None;
    }

    match bytes[0] {
        0x0e => {
            let (len, offset) = decode_vlq(&bytes[1..])?;
            if offset + 1 + len > bytes.len() {
                return None;
            }
            let string_bytes = &bytes[1 + offset..1 + offset + len];
            let s = String::from_utf8(string_bytes.to_vec()).ok()?;
            s.trim().parse().ok()
        }
        0x04 | 0x05 => {
            let (value, _) = decode_vlq(&bytes[1..])?;
            Some(((value >> 1) as i32) ^ -((value & 1) as i32))
        }
        _ => {
            let s = String::from_utf8(bytes).ok()?;
            s.trim().parse().ok()
        }
    }
}

/// Decode a VLQ encoded integer
fn decode_vlq(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut value: usize = 0;
    let mut shift = 0;

    for (i, &byte) in bytes.iter().enumerate() {
        value |= ((byte & 0x7f) as usize) << shift;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
        shift += 7;
        if shift > 35 {
            return None;
        }
    }
    None
}

use anyhow::{Context, Result};
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

    pub async fn process_block(&mut self, block: &Value) -> Result<()> {
        let header = block.get("header").context("Missing header")?;
        let block_txs = block.get("blockTransactions").context("Missing blockTransactions")?;
        let transactions = block_txs
            .get("transactions")
            .and_then(|t| t.as_array())
            .context("Missing transactions array")?;

        // Extract header data
        let block_id = header.get("id").and_then(|v| v.as_str()).context("Missing block id")?;
        let parent_id = header.get("parentId").and_then(|v| v.as_str()).unwrap_or("");
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
            .flat_map(|tx| {
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

        // Insert block
        self.db.execute(
            "INSERT INTO blocks (
                block_id, parent_id, height, timestamp, difficulty, block_size,
                block_coins, tx_count, miner_address, miner_reward, main_chain, global_index
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, TRUE, ?)
            ON CONFLICT (block_id) DO UPDATE SET main_chain = TRUE",
            &[
                &block_id,
                &parent_id,
                &height,
                &timestamp,
                &difficulty,
                &block_size,
                &block_coins,
                &tx_count,
                &miner_address.as_deref(),
                &miner_reward,
                &self.global_block_index,
            ],
        )?;

        // Process transactions
        for (tx_idx, tx) in transactions.iter().enumerate() {
            self.process_transaction(tx, block_id, height, timestamp, tx_idx as i32)
                .await?;
        }

        // Update network stats periodically (every 100 blocks)
        if height % 100 == 0 {
            self.update_network_stats(height, timestamp, difficulty)?;
        }

        Ok(())
    }

    async fn process_transaction(
        &mut self,
        tx: &Value,
        block_id: &str,
        height: i64,
        timestamp: i64,
        tx_idx: i32,
    ) -> Result<()> {
        let tx_id = tx.get("id").and_then(|v| v.as_str()).context("Missing tx id")?;
        let inputs = tx.get("inputs").and_then(|v| v.as_array());
        let outputs = tx.get("outputs").and_then(|v| v.as_array()).context("Missing outputs")?;
        let data_inputs = tx.get("dataInputs").and_then(|v| v.as_array());
        let size = tx.get("size").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        let input_count = inputs.map(|i| i.len()).unwrap_or(0) as i32;
        let output_count = outputs.len() as i32;
        let coinbase = input_count == 0 || tx_idx == 0;

        self.global_tx_index += 1;

        // Insert transaction
        self.db.execute(
            "INSERT INTO transactions (
                tx_id, block_id, inclusion_height, timestamp, index_in_block,
                global_index, coinbase, size, input_count, output_count
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (tx_id) DO NOTHING",
            &[
                &tx_id,
                &block_id,
                &height,
                &timestamp,
                &tx_idx,
                &self.global_tx_index,
                &coinbase,
                &size,
                &input_count,
                &output_count,
            ],
        )?;

        // Process inputs (mark boxes as spent)
        if let Some(inputs) = inputs {
            for (input_idx, input) in inputs.iter().enumerate() {
                self.process_input(input, tx_id, height, input_idx as i32)?;
            }
        }

        // Process data inputs
        if let Some(data_inputs) = data_inputs {
            for (di_idx, data_input) in data_inputs.iter().enumerate() {
                self.process_data_input(data_input, tx_id, di_idx as i32)?;
            }
        }

        // Process outputs (create boxes)
        for (output_idx, output) in outputs.iter().enumerate() {
            self.process_output(output, tx_id, height, output_idx as i32)?;
        }

        Ok(())
    }

    fn process_input(&mut self, input: &Value, tx_id: &str, height: i64, input_idx: i32) -> Result<()> {
        let box_id = input.get("boxId").and_then(|v| v.as_str()).context("Missing boxId")?;
        let proof_bytes = input
            .get("spendingProof")
            .and_then(|sp| sp.get("proofBytes"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Mark box as spent
        self.db.execute(
            "UPDATE boxes SET spent_tx_id = ?, spent_index = ?, spent_height = ? WHERE box_id = ?",
            &[&tx_id, &input_idx, &height, &box_id],
        )?;

        // Record input
        self.input_id += 1;
        self.db.execute(
            "INSERT INTO inputs (id, tx_id, box_id, input_index, proof_bytes)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT DO NOTHING",
            &[&self.input_id, &tx_id, &box_id, &input_idx, &proof_bytes],
        )?;

        Ok(())
    }

    fn process_data_input(&mut self, data_input: &Value, tx_id: &str, input_idx: i32) -> Result<()> {
        let box_id = data_input.get("boxId").and_then(|v| v.as_str()).context("Missing boxId")?;

        self.data_input_id += 1;
        self.db.execute(
            "INSERT INTO data_inputs (id, tx_id, box_id, input_index)
             VALUES (?, ?, ?, ?)
             ON CONFLICT DO NOTHING",
            &[&self.data_input_id, &tx_id, &box_id, &input_idx],
        )?;

        Ok(())
    }

    fn process_output(&mut self, output: &Value, tx_id: &str, height: i64, output_idx: i32) -> Result<()> {
        let box_id = output.get("boxId").and_then(|v| v.as_str()).context("Missing boxId")?;
        let value = output.get("value").and_then(|v| v.as_i64()).unwrap_or(0);
        let ergo_tree = output.get("ergoTree").and_then(|v| v.as_str()).unwrap_or("");
        let creation_height = output.get("creationHeight").and_then(|v| v.as_i64()).unwrap_or(height);
        let additional_registers = output.get("additionalRegisters");
        let assets = output.get("assets").and_then(|v| v.as_array());

        // Derive address from ergo_tree
        let address = ergo_tree::ergo_tree_to_address(ergo_tree).unwrap_or_else(|| ergo_tree.to_string());
        let template_hash = ergo_tree::ergo_tree_template_hash(ergo_tree);

        self.global_box_index += 1;

        // Insert box
        self.db.execute(
            "INSERT INTO boxes (
                box_id, tx_id, output_index, ergo_tree, ergo_tree_template_hash,
                address, value, creation_height, settlement_height, global_index,
                additional_registers
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (box_id) DO NOTHING",
            &[
                &box_id,
                &tx_id,
                &output_idx,
                &ergo_tree,
                &template_hash,
                &address,
                &value,
                &creation_height,
                &height,
                &self.global_box_index,
                &additional_registers.map(|r| r.to_string()),
            ],
        )?;

        // Process assets (tokens)
        if let Some(assets) = assets {
            for (asset_idx, asset) in assets.iter().enumerate() {
                self.process_asset(asset, box_id, height, asset_idx as i32)?;
            }
        }

        // Update address stats
        self.update_address_stats(&address, height)?;

        Ok(())
    }

    fn process_asset(&mut self, asset: &Value, box_id: &str, height: i64, asset_idx: i32) -> Result<()> {
        let token_id = asset.get("tokenId").and_then(|v| v.as_str()).context("Missing tokenId")?;
        let amount = asset.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);

        self.box_asset_id += 1;

        // Insert box asset
        self.db.execute(
            "INSERT INTO box_assets (id, box_id, token_id, amount, asset_index)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT DO NOTHING",
            &[&self.box_asset_id, &box_id, &token_id, &amount, &asset_idx],
        )?;

        // Check if this is a new token (first emission)
        // Token ID equals the box ID of the first input of the minting transaction
        if asset_idx == 0 {
            self.try_register_token(token_id, box_id, amount, height)?;
        }

        Ok(())
    }

    fn try_register_token(&self, token_id: &str, box_id: &str, amount: i64, height: i64) -> Result<()> {
        // Check if token already exists
        let exists: Option<i32> = self.db.query_one(
            "SELECT 1 FROM tokens WHERE token_id = ?",
            &[&token_id],
            |row| row.get(0),
        )?;

        if exists.is_none() {
            // This might be the minting box - try to extract name/description from registers
            // For now, insert basic info
            self.db.execute(
                "INSERT INTO tokens (token_id, box_id, emission_amount, creation_height)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT (token_id) DO NOTHING",
                &[&token_id, &box_id, &amount, &height],
            )?;
        }

        Ok(())
    }

    fn update_address_stats(&self, address: &str, height: i64) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        self.db.execute(
            "INSERT INTO address_stats (address, tx_count, first_seen_height, last_seen_height, updated_at)
             VALUES (?, 1, ?, ?, ?)
             ON CONFLICT (address) DO UPDATE SET
                tx_count = address_stats.tx_count + 1,
                last_seen_height = ?,
                updated_at = ?",
            &[&address, &height, &height, &now, &height, &now],
        )?;

        Ok(())
    }

    fn update_network_stats(&self, height: i64, timestamp: i64, difficulty: i64) -> Result<()> {
        // Calculate basic network stats
        let total_coins: i64 = self.db.query_one(
            "SELECT COALESCE(SUM(value), 0) FROM boxes WHERE spent_tx_id IS NULL",
            &[],
            |row| row.get(0),
        )?.unwrap_or(0);

        let block_size: i32 = self.db.query_one(
            "SELECT COALESCE(block_size, 0) FROM blocks WHERE height = ?",
            &[&height],
            |row| row.get(0),
        )?.unwrap_or(0);

        let block_coins: i64 = self.db.query_one(
            "SELECT COALESCE(block_coins, 0) FROM blocks WHERE height = ?",
            &[&height],
            |row| row.get(0),
        )?.unwrap_or(0);

        // Simple hashrate estimate (difficulty / 120 seconds target)
        let hashrate = difficulty as f64 / 120.0;

        self.db.execute(
            "INSERT INTO network_stats (
                timestamp, height, difficulty, block_size, block_coins, total_coins, hashrate
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (timestamp) DO UPDATE SET
                height = ?, difficulty = ?, hashrate = ?",
            &[
                &timestamp,
                &height,
                &difficulty,
                &block_size,
                &block_coins,
                &total_coins,
                &hashrate,
                &height,
                &difficulty,
                &hashrate,
            ],
        )?;

        Ok(())
    }
}

use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct NodeClient {
    pub url: String,
    client: Client,
    api_key: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub app_version: Option<String>,
    pub full_height: Option<i64>,
    #[serde(alias = "headersHeight")]
    pub headers_height: Option<i64>,
    #[serde(alias = "maxPeerHeight")]
    pub max_peer_height: Option<i64>,
    #[serde(alias = "bestFullHeaderId")]
    pub best_full_header_id: Option<String>,
    #[serde(alias = "bestHeaderId")]
    pub best_header_id: Option<String>,
    #[serde(alias = "stateRoot")]
    pub state_root: Option<String>,
    #[serde(alias = "stateType")]
    pub state_type: Option<String>,
    #[serde(alias = "stateVersion")]
    pub state_version: Option<String>,
    #[serde(alias = "isMining")]
    pub is_mining: Option<bool>,
    #[serde(alias = "peersCount")]
    pub peers_count: Option<i32>,
    #[serde(alias = "unconfirmedCount")]
    pub unconfirmed_count: Option<i32>,
    // Use serde_json::Number to handle large integers that overflow i64
    pub difficulty: Option<serde_json::Number>,
    #[serde(alias = "currentTime")]
    pub current_time: Option<i64>,
    #[serde(alias = "launchTime")]
    pub launch_time: Option<i64>,
    // These can be very large numbers, use serde_json::Number
    #[serde(alias = "headersScore")]
    pub headers_score: Option<serde_json::Number>,
    #[serde(alias = "fullBlocksScore")]
    pub full_blocks_score: Option<serde_json::Number>,
    #[serde(alias = "genesisBlockId")]
    pub genesis_block_id: Option<String>,
    pub parameters: Option<serde_json::Value>,
    #[serde(alias = "eip27Supported")]
    pub eip27_supported: Option<bool>,
    #[serde(alias = "restApiUrl")]
    pub rest_api_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockHeader {
    pub id: String,
    pub parent_id: String,
    pub version: i32,
    pub height: i64,
    pub n_bits: i64,
    pub difficulty: String,
    pub timestamp: i64,
    pub state_root: String,
    pub ad_proofs_root: String,
    pub transactions_root: String,
    pub extension_hash: String,
    pub miner_pk: String,
    pub w: String,
    pub n: String,
    pub d: String,
    pub votes: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: String,
    pub inputs: Vec<Input>,
    pub data_inputs: Option<Vec<DataInput>>,
    pub outputs: Vec<Output>,
    pub size: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub box_id: String,
    pub spending_proof: SpendingProof,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpendingProof {
    pub proof_bytes: String,
    pub extension: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataInput {
    pub box_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub box_id: String,
    pub value: i64,
    pub ergo_tree: String,
    pub creation_height: i64,
    pub assets: Option<Vec<Asset>>,
    pub additional_registers: Option<serde_json::Value>,
    pub transaction_id: Option<String>,
    pub index: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub token_id: String,
    pub amount: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MempoolTransaction {
    pub id: String,
    pub inputs: Vec<Input>,
    pub data_inputs: Option<Vec<DataInput>>,
    pub outputs: Vec<Output>,
    pub size: i32,
}

impl NodeClient {
    pub fn new(url: String, api_key: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            url: url.trim_end_matches('/').to_string(),
            client,
            api_key,
        }
    }

    fn build_request(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.url, path);
        let mut req = self.client.get(&url);

        if let Some(ref key) = self.api_key {
            req = req.header("api_key", key);
        }

        req
    }

    pub async fn get_info(&self) -> Result<NodeInfo> {
        let resp = self
            .build_request("/info")
            .send()
            .await
            .context("Failed to connect to node")?;

        if resp.status() != StatusCode::OK {
            anyhow::bail!("Node returned status {}", resp.status());
        }

        let text = resp.text().await.context("Failed to read response")?;

        serde_json::from_str(&text).with_context(|| {
            format!("Failed to parse node info. Response: {}", &text[..text.len().min(500)])
        })
    }

    pub async fn get_block_ids_at_height(&self, height: i64) -> Result<Vec<String>> {
        let resp = self
            .build_request(&format!("/blocks/at/{}", height))
            .send()
            .await?;

        if resp.status() != StatusCode::OK {
            anyhow::bail!("Failed to get block IDs at height {}: {}", height, resp.status());
        }

        resp.json().await.context("Failed to parse block IDs")
    }

    pub async fn get_block(&self, header_id: &str) -> Result<serde_json::Value> {
        let resp = self
            .build_request(&format!("/blocks/{}", header_id))
            .send()
            .await?;

        if resp.status() != StatusCode::OK {
            anyhow::bail!("Failed to get block {}: {}", header_id, resp.status());
        }

        resp.json().await.context("Failed to parse block")
    }

    pub async fn get_block_header(&self, header_id: &str) -> Result<BlockHeader> {
        let resp = self
            .build_request(&format!("/blocks/{}/header", header_id))
            .send()
            .await?;

        if resp.status() != StatusCode::OK {
            anyhow::bail!("Failed to get block header: {}", resp.status());
        }

        resp.json().await.context("Failed to parse block header")
    }

    pub async fn get_last_headers(&self, count: i32) -> Result<Vec<BlockHeader>> {
        let resp = self
            .build_request(&format!("/blocks/lastHeaders/{}", count))
            .send()
            .await?;

        if resp.status() != StatusCode::OK {
            anyhow::bail!("Failed to get last headers: {}", resp.status());
        }

        resp.json().await.context("Failed to parse headers")
    }

    pub async fn get_mempool_transactions(&self, limit: i32, offset: i32) -> Result<Vec<MempoolTransaction>> {
        let resp = self
            .build_request(&format!(
                "/transactions/unconfirmed?limit={}&offset={}",
                limit, offset
            ))
            .send()
            .await?;

        if resp.status() != StatusCode::OK {
            anyhow::bail!("Failed to get mempool: {}", resp.status());
        }

        resp.json().await.context("Failed to parse mempool")
    }

    pub async fn get_mempool_size(&self) -> Result<i32> {
        let info = self.get_info().await?;
        Ok(info.unconfirmed_count.unwrap_or(0))
    }

    pub async fn submit_transaction(&self, tx: &serde_json::Value) -> Result<String> {
        let resp = self
            .client
            .post(format!("{}/transactions", self.url))
            .header("Content-Type", "application/json")
            .header("api_key", self.api_key.as_deref().unwrap_or(""))
            .json(tx)
            .send()
            .await?;

        if resp.status() != StatusCode::OK {
            let error_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Failed to submit transaction: {}", error_text);
        }

        resp.json().await.context("Failed to parse response")
    }

    pub async fn check_transaction(&self, tx: &serde_json::Value) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(format!("{}/transactions/check", self.url))
            .header("Content-Type", "application/json")
            .json(tx)
            .send()
            .await?;

        resp.json().await.context("Failed to parse response")
    }

    // Wallet API endpoints (require API key)
    pub async fn wallet_addresses(&self) -> Result<Vec<String>> {
        let resp = self
            .client
            .get(format!("{}/wallet/addresses", self.url))
            .header("api_key", self.api_key.as_deref().unwrap_or(""))
            .send()
            .await?;

        if resp.status() != StatusCode::OK {
            anyhow::bail!("Failed to get wallet addresses: {}", resp.status());
        }

        resp.json().await.context("Failed to parse addresses")
    }

    pub async fn wallet_balances(&self) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(format!("{}/wallet/balances", self.url))
            .header("api_key", self.api_key.as_deref().unwrap_or(""))
            .send()
            .await?;

        if resp.status() != StatusCode::OK {
            anyhow::bail!("Failed to get wallet balances: {}", resp.status());
        }

        resp.json().await.context("Failed to parse balances")
    }

    pub async fn wallet_status(&self) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(format!("{}/wallet/status", self.url))
            .header("api_key", self.api_key.as_deref().unwrap_or(""))
            .send()
            .await?;

        resp.json().await.context("Failed to parse status")
    }

    pub async fn wallet_unlock(&self, password: &str) -> Result<()> {
        let resp = self
            .client
            .post(format!("{}/wallet/unlock", self.url))
            .header("api_key", self.api_key.as_deref().unwrap_or(""))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "pass": password }))
            .send()
            .await?;

        if resp.status() != StatusCode::OK {
            anyhow::bail!("Failed to unlock wallet: {}", resp.status());
        }

        Ok(())
    }

    pub async fn wallet_lock(&self) -> Result<()> {
        let resp = self
            .client
            .get(format!("{}/wallet/lock", self.url))
            .header("api_key", self.api_key.as_deref().unwrap_or(""))
            .send()
            .await?;

        if resp.status() != StatusCode::OK {
            anyhow::bail!("Failed to lock wallet: {}", resp.status());
        }

        Ok(())
    }

    pub async fn wallet_transaction_generate(
        &self,
        requests: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(format!("{}/wallet/transaction/generate", self.url))
            .header("api_key", self.api_key.as_deref().unwrap_or(""))
            .header("Content-Type", "application/json")
            .json(requests)
            .send()
            .await?;

        resp.json().await.context("Failed to generate transaction")
    }

    pub async fn wallet_transaction_send(
        &self,
        requests: &serde_json::Value,
    ) -> Result<String> {
        let resp = self
            .client
            .post(format!("{}/wallet/transaction/send", self.url))
            .header("api_key", self.api_key.as_deref().unwrap_or(""))
            .header("Content-Type", "application/json")
            .json(requests)
            .send()
            .await?;

        if resp.status() != StatusCode::OK {
            let error = resp.text().await.unwrap_or_default();
            anyhow::bail!("Failed to send transaction: {}", error);
        }

        resp.json().await.context("Failed to parse transaction ID")
    }
}

/// Database migrations - each tuple is (name, SQL)
pub const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_initial_schema",
        r#"
        -- Blocks table
        CREATE TABLE IF NOT EXISTS blocks (
            block_id VARCHAR(64) PRIMARY KEY,
            parent_id VARCHAR(64) NOT NULL,
            height INTEGER NOT NULL,
            timestamp BIGINT NOT NULL,
            difficulty BIGINT NOT NULL,
            block_size INTEGER NOT NULL,
            block_coins BIGINT NOT NULL,
            block_mining_time BIGINT,
            tx_count INTEGER NOT NULL,
            miner_address VARCHAR(64),
            miner_reward BIGINT NOT NULL,
            miner_name VARCHAR(128),
            main_chain BOOLEAN NOT NULL DEFAULT TRUE,
            global_index BIGINT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_blocks_height ON blocks(height) WHERE main_chain = TRUE;
        CREATE INDEX IF NOT EXISTS idx_blocks_miner ON blocks(miner_address);
        CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON blocks(timestamp);
        CREATE INDEX IF NOT EXISTS idx_blocks_global_index ON blocks(global_index);

        -- Transactions table
        CREATE TABLE IF NOT EXISTS transactions (
            tx_id VARCHAR(64) PRIMARY KEY,
            block_id VARCHAR(64) NOT NULL,
            inclusion_height INTEGER NOT NULL,
            timestamp BIGINT NOT NULL,
            index_in_block INTEGER NOT NULL,
            global_index BIGINT NOT NULL,
            coinbase BOOLEAN NOT NULL DEFAULT FALSE,
            size INTEGER NOT NULL,
            input_count INTEGER NOT NULL,
            output_count INTEGER NOT NULL,
            FOREIGN KEY (block_id) REFERENCES blocks(block_id)
        );

        CREATE INDEX IF NOT EXISTS idx_tx_block ON transactions(block_id);
        CREATE INDEX IF NOT EXISTS idx_tx_height ON transactions(inclusion_height);
        CREATE INDEX IF NOT EXISTS idx_tx_timestamp ON transactions(timestamp);
        CREATE INDEX IF NOT EXISTS idx_tx_global_index ON transactions(global_index);

        -- Boxes (UTXOs) table
        CREATE TABLE IF NOT EXISTS boxes (
            box_id VARCHAR(64) PRIMARY KEY,
            tx_id VARCHAR(64) NOT NULL,
            output_index INTEGER NOT NULL,
            ergo_tree TEXT NOT NULL,
            ergo_tree_template_hash VARCHAR(64) NOT NULL,
            address VARCHAR(64) NOT NULL,
            value BIGINT NOT NULL,
            creation_height INTEGER NOT NULL,
            settlement_height INTEGER NOT NULL,
            global_index BIGINT NOT NULL,
            additional_registers JSON,
            spent_tx_id VARCHAR(64),
            spent_index INTEGER,
            spent_height INTEGER,
            FOREIGN KEY (tx_id) REFERENCES transactions(tx_id)
        );

        CREATE INDEX IF NOT EXISTS idx_boxes_address ON boxes(address);
        CREATE INDEX IF NOT EXISTS idx_boxes_ergo_tree_hash ON boxes(ergo_tree_template_hash);
        CREATE INDEX IF NOT EXISTS idx_boxes_unspent ON boxes(address) WHERE spent_tx_id IS NULL;
        CREATE INDEX IF NOT EXISTS idx_boxes_creation_height ON boxes(creation_height);
        CREATE INDEX IF NOT EXISTS idx_boxes_global_index ON boxes(global_index);
        CREATE INDEX IF NOT EXISTS idx_boxes_tx ON boxes(tx_id);
        CREATE INDEX IF NOT EXISTS idx_boxes_spent_tx ON boxes(spent_tx_id) WHERE spent_tx_id IS NOT NULL;

        -- Box assets (tokens in boxes)
        CREATE TABLE IF NOT EXISTS box_assets (
            id BIGINT PRIMARY KEY,
            box_id VARCHAR(64) NOT NULL,
            token_id VARCHAR(64) NOT NULL,
            amount BIGINT NOT NULL,
            asset_index INTEGER NOT NULL,
            FOREIGN KEY (box_id) REFERENCES boxes(box_id)
        );

        CREATE INDEX IF NOT EXISTS idx_box_assets_box ON box_assets(box_id);
        CREATE INDEX IF NOT EXISTS idx_box_assets_token ON box_assets(token_id);

        -- Tokens registry
        CREATE TABLE IF NOT EXISTS tokens (
            token_id VARCHAR(64) PRIMARY KEY,
            box_id VARCHAR(64) NOT NULL,
            emission_amount BIGINT NOT NULL,
            name VARCHAR(512),
            description TEXT,
            token_type VARCHAR(64),
            decimals INTEGER,
            creation_height INTEGER NOT NULL,
            FOREIGN KEY (box_id) REFERENCES boxes(box_id)
        );

        CREATE INDEX IF NOT EXISTS idx_tokens_name ON tokens(name);
        CREATE INDEX IF NOT EXISTS idx_tokens_height ON tokens(creation_height);

        -- Inputs table (for tracking spending)
        CREATE TABLE IF NOT EXISTS inputs (
            id BIGINT PRIMARY KEY,
            tx_id VARCHAR(64) NOT NULL,
            box_id VARCHAR(64) NOT NULL,
            input_index INTEGER NOT NULL,
            proof_bytes TEXT,
            FOREIGN KEY (tx_id) REFERENCES transactions(tx_id),
            FOREIGN KEY (box_id) REFERENCES boxes(box_id)
        );

        CREATE INDEX IF NOT EXISTS idx_inputs_tx ON inputs(tx_id);
        CREATE INDEX IF NOT EXISTS idx_inputs_box ON inputs(box_id);

        -- Data inputs table
        CREATE TABLE IF NOT EXISTS data_inputs (
            id BIGINT PRIMARY KEY,
            tx_id VARCHAR(64) NOT NULL,
            box_id VARCHAR(64) NOT NULL,
            input_index INTEGER NOT NULL,
            FOREIGN KEY (tx_id) REFERENCES transactions(tx_id)
        );

        CREATE INDEX IF NOT EXISTS idx_data_inputs_tx ON data_inputs(tx_id);
        CREATE INDEX IF NOT EXISTS idx_data_inputs_box ON data_inputs(box_id);

        -- Address statistics (materialized)
        CREATE TABLE IF NOT EXISTS address_stats (
            address VARCHAR(64) PRIMARY KEY,
            tx_count INTEGER NOT NULL DEFAULT 0,
            balance BIGINT NOT NULL DEFAULT 0,
            first_seen_height INTEGER,
            last_seen_height INTEGER,
            updated_at BIGINT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_address_stats_balance ON address_stats(balance DESC);

        -- Network statistics (time-series)
        CREATE TABLE IF NOT EXISTS network_stats (
            timestamp BIGINT PRIMARY KEY,
            height INTEGER NOT NULL,
            difficulty BIGINT NOT NULL,
            block_size INTEGER NOT NULL,
            block_coins BIGINT NOT NULL,
            total_coins BIGINT NOT NULL,
            hashrate DOUBLE,
            block_time_avg DOUBLE
        );

        CREATE INDEX IF NOT EXISTS idx_network_stats_height ON network_stats(height);

        -- Sync status table
        CREATE TABLE IF NOT EXISTS sync_status (
            id INTEGER PRIMARY KEY DEFAULT 1,
            last_synced_height INTEGER NOT NULL DEFAULT -1,
            last_synced_block_id VARCHAR(64),
            last_sync_time BIGINT,
            sync_started_at BIGINT,
            is_syncing BOOLEAN NOT NULL DEFAULT FALSE,
            error_message TEXT
        );

        INSERT INTO sync_status (id, last_synced_height) VALUES (1, -1)
            ON CONFLICT DO NOTHING;

        -- Mempool transactions
        CREATE TABLE IF NOT EXISTS mempool_transactions (
            tx_id VARCHAR(64) PRIMARY KEY,
            tx_data JSON NOT NULL,
            first_seen BIGINT NOT NULL,
            size INTEGER NOT NULL
        );
        "#,
    ),
    (
        "002_add_token_holders",
        r#"
        -- Token holders view (materialized as table for performance)
        CREATE TABLE IF NOT EXISTS token_holders (
            token_id VARCHAR(64) NOT NULL,
            address VARCHAR(64) NOT NULL,
            amount BIGINT NOT NULL,
            PRIMARY KEY (token_id, address)
        );

        CREATE INDEX IF NOT EXISTS idx_token_holders_token ON token_holders(token_id);
        CREATE INDEX IF NOT EXISTS idx_token_holders_address ON token_holders(address);
        CREATE INDEX IF NOT EXISTS idx_token_holders_amount ON token_holders(token_id, amount DESC);
        "#,
    ),
    (
        "003_add_epochs",
        r#"
        -- Epochs table for epoch-based queries
        CREATE TABLE IF NOT EXISTS epochs (
            epoch_index INTEGER PRIMARY KEY,
            height_start INTEGER NOT NULL,
            height_end INTEGER NOT NULL,
            timestamp_start BIGINT NOT NULL,
            timestamp_end BIGINT,
            block_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_epochs_height ON epochs(height_start, height_end);
        "#,
    ),
    (
        "004_full_text_search",
        r#"
        -- Search index table
        CREATE TABLE IF NOT EXISTS search_index (
            entity_type VARCHAR(32) NOT NULL,  -- block, tx, address, token
            entity_id VARCHAR(64) NOT NULL,
            search_text TEXT NOT NULL,
            PRIMARY KEY (entity_type, entity_id)
        );
        "#,
    ),
];

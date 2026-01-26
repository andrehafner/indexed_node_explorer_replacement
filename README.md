# Ergo Index

A lightweight Ergo blockchain indexer and explorer that runs alongside your node. Provides all Explorer API endpoints without the complexity of the full Explorer stack.

## Features

- **Full Explorer API Compatibility** - All 49 endpoints from the official Explorer API
- **Lightweight** - Single binary + DuckDB (2-5 GB vs 50+ GB for full Explorer)
- **Node Agnostic** - Works with any Ergo node, easy to upgrade nodes independently
- **Parallel Sync** - Connect to multiple nodes for faster indexing
- **Real-time Status** - Monitor sync progress, connected nodes, and database stats
- **Wallet Integration** - Built-in wallet UI connected to your node
- **Docker Ready** - Multiple deployment options out of the box

## Quick Start

### Option 1: Docker with External Node (Recommended)

If you already have an Ergo node running:

```bash
# Clone the repo
git clone https://github.com/andrehafner/indexed_node_explorer_replacement.git
cd indexed_node_explorer_replacement

# Start (assumes node is at localhost:9053)
docker-compose up -d

# Or specify a different node
ERGO_NODES=http://your-node:9053 docker-compose up -d
```

### Option 2: Docker with Embedded Node

Spin up both node and indexer together:

```bash
docker-compose -f docker-compose.yml -f docker-compose.embedded.yml up -d
```

### Option 3: From Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build
cargo build --release

# Run
./target/release/ergo-index --nodes http://localhost:9053
```

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `ERGO_NODES` | `http://localhost:9053` | Comma-separated list of node URLs |
| `NODE_API_KEY` | - | Node API key (for wallet operations) |
| `DATABASE_PATH` | `./data/ergo-index.duckdb` | Path to DuckDB database file |
| `PORT` | `8080` | HTTP server port |
| `NETWORK` | `mainnet` | Network type (mainnet/testnet) |
| `SYNC_BATCH_SIZE` | `100` | Blocks per sync batch |
| `SYNC_INTERVAL` | `10` | Seconds between sync checks |

### Using Multiple Nodes for Faster Sync

```bash
# Connect to multiple nodes for parallel fetching
docker-compose -f docker-compose.yml -f docker-compose.multi-node.yml up -d

# Or via environment variable
ERGO_NODES=http://node1:9053,http://node2:9053,http://node3:9053 docker-compose up -d
```

## Endpoints

### Web UI
- `http://localhost:8080/` - Explorer UI
- `http://localhost:8080/docs` - Swagger API Documentation
- `http://localhost:8080/status` - System status and sync progress

### API v1 (Explorer-compatible)

#### Blocks
- `GET /api/v1/blocks` - List blocks
- `GET /api/v1/blocks/{id}` - Get block by ID or height
- `GET /api/v1/blocks/headers` - Get recent headers
- `GET /api/v1/blocks/at/{height}` - Get block at height
- `GET /api/v1/blocks/byMiner/{address}` - Get blocks by miner

#### Transactions
- `GET /api/v1/transactions` - List transactions
- `GET /api/v1/transactions/{id}` - Get transaction
- `GET /api/v1/transactions/byBlock/{blockId}` - Get transactions in block
- `GET /api/v1/transactions/byAddress/{address}` - Get transactions for address
- `POST /api/v1/transactions/submit` - Submit transaction

#### Addresses
- `GET /api/v1/addresses/{address}` - Get address info
- `GET /api/v1/addresses/{address}/balance/total` - Get total balance
- `GET /api/v1/addresses/{address}/balance/confirmed` - Get confirmed balance
- `GET /api/v1/addresses/{address}/transactions` - Get address transactions

#### Boxes (UTXOs)
- `GET /api/v1/boxes/{boxId}` - Get box by ID
- `GET /api/v1/boxes/byAddress/{address}` - Get boxes by address
- `GET /api/v1/boxes/unspent/byAddress/{address}` - Get unspent boxes
- `GET /api/v1/boxes/byTokenId/{tokenId}` - Get boxes containing token
- `GET /api/v1/boxes/unspent/byTokenId/{tokenId}` - Get unspent boxes with token
- `POST /api/v1/boxes/search` - Search boxes
- `POST /api/v1/boxes/unspent/search` - Search unspent boxes

#### Tokens
- `GET /api/v1/tokens` - List tokens
- `GET /api/v1/tokens/{tokenId}` - Get token info
- `GET /api/v1/tokens/search` - Search tokens by name
- `GET /api/v1/tokens/{tokenId}/holders` - Get token holders
- `GET /api/v1/tokens/byAddress/{address}` - Get tokens held by address

#### Mempool
- `GET /api/v1/mempool/transactions` - Get mempool transactions
- `GET /api/v1/mempool/transactions/{txId}` - Get mempool transaction
- `GET /api/v1/mempool/transactions/byAddress/{address}` - Get mempool txs for address
- `GET /api/v1/mempool/size` - Get mempool size

#### Stats & Info
- `GET /api/v1/info` - Get API info
- `GET /api/v1/stats` - Get explorer statistics
- `GET /api/v1/stats/network` - Get network statistics
- `GET /api/v1/epochs` - Get epochs
- `GET /api/v1/epochs/{index}` - Get specific epoch

#### Search
- `GET /api/v1/search?query={query}` - Universal search

#### Wallet (proxied to node)
- `GET /api/v1/wallet/status` - Get wallet status
- `GET /api/v1/wallet/addresses` - Get wallet addresses
- `GET /api/v1/wallet/balances` - Get wallet balances
- `POST /api/v1/wallet/unlock` - Unlock wallet
- `POST /api/v1/wallet/lock` - Lock wallet
- `POST /api/v1/wallet/transaction/generate` - Generate transaction
- `POST /api/v1/wallet/transaction/send` - Send transaction

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     ergo-index                                  │
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │    Axum API     │  │    DuckDB       │  │    Web UI      │  │
│  │   (49 endpoints)│  │  (2-5 GB)       │  │  (Svelte-like) │  │
│  └─────────────────┘  └─────────────────┘  └────────────────┘  │
│                              ▲                                  │
│                              │                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   Sync Service                           │   │
│  │         (parallel fetching from multiple nodes)          │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ HTTP
                              ▼
               ┌──────────────────────────────┐
               │     Ergo Node(s)             │
               │     (unchanged, any version) │
               │                              │
               │     RocksDB                  │
               └──────────────────────────────┘
```

## Database Schema

The indexer maintains the following tables in DuckDB:

- `blocks` - Block headers and metadata
- `transactions` - Transaction records
- `boxes` - UTXO boxes with ErgoTree and registers
- `box_assets` - Token amounts in boxes
- `tokens` - Token registry with metadata
- `inputs` - Input references
- `data_inputs` - Data input references
- `address_stats` - Pre-computed address statistics
- `network_stats` - Time-series network statistics

## Performance

| Metric | ergo-index | Full Explorer |
|--------|------------|---------------|
| Database size | 2-5 GB | 50+ GB |
| Memory usage | 512MB - 1GB | 8GB+ |
| Sync speed | ~100-500 blocks/sec | ~50-100 blocks/sec |
| Components | 1 binary | 5+ services |
| Node dependency | HTTP only | Internal |

## Development

```bash
# Run in development mode
cargo run -- --nodes http://localhost:9053

# Run tests
cargo test

# Build release
cargo build --release
```

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Credits

- [Ergo Platform](https://ergoplatform.org/)
- [explorer-backend](https://github.com/ergoplatform/explorer-backend) - API design inspiration
- [explorer_perl](https://github.com/andrehafner/explorer_perl) - Original explorer implementation

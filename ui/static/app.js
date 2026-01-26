// Ergo Index Explorer - Frontend Application

const API_BASE = '/api/v1';

// Utility functions
function formatNumber(num) {
    if (num === null || num === undefined) return '-';
    return new Intl.NumberFormat().format(num);
}

function formatBytes(bytes) {
    if (!bytes) return '-';
    const units = ['B', 'KB', 'MB', 'GB'];
    let i = 0;
    while (bytes >= 1024 && i < units.length - 1) {
        bytes /= 1024;
        i++;
    }
    return `${bytes.toFixed(2)} ${units[i]}`;
}

function formatDuration(seconds) {
    if (!seconds) return '-';
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
    const hours = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${mins}m`;
}

function formatTimestamp(ts) {
    if (!ts) return '-';
    return new Date(ts).toLocaleString();
}

function truncateId(id, len = 16) {
    if (!id || id.length <= len) return id;
    return id.substring(0, len / 2) + '...' + id.substring(id.length - len / 2);
}

function nanoErgToErg(nanoErg) {
    return (nanoErg / 1e9).toFixed(9);
}

// API calls
async function fetchApi(endpoint) {
    try {
        const res = await fetch(`${API_BASE}${endpoint}`);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return await res.json();
    } catch (e) {
        console.error(`API error: ${endpoint}`, e);
        return null;
    }
}

async function postApi(endpoint, data) {
    try {
        const res = await fetch(`${API_BASE}${endpoint}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(data)
        });
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return await res.json();
    } catch (e) {
        console.error(`API error: ${endpoint}`, e);
        return null;
    }
}

// Page navigation
function navigateTo(page) {
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    document.querySelectorAll('.nav-link').forEach(l => l.classList.remove('active'));

    document.getElementById(`${page}-page`).classList.add('active');
    document.querySelector(`[data-page="${page}"]`).classList.add('active');

    // Load page data
    if (page === 'explorer') loadExplorerData();
    if (page === 'status') loadStatusData();
    if (page === 'wallet') loadWalletData();
}

// Explorer page
async function loadExplorerData() {
    // Load stats
    const stats = await fetchApi('/stats');
    if (stats) {
        document.getElementById('stat-blocks').textContent = formatNumber(stats.blockCount);
        document.getElementById('stat-txs').textContent = formatNumber(stats.transactionCount);
        document.getElementById('stat-addresses').textContent = formatNumber(stats.addressCount);
        document.getElementById('stat-tokens').textContent = formatNumber(stats.tokenCount);
    }

    // Load latest blocks
    const blocks = await fetchApi('/blocks?limit=10');
    if (blocks && blocks.items) {
        const container = document.getElementById('latest-blocks');
        container.innerHTML = blocks.items.map(block => `
            <div class="list-item" onclick="showBlockDetail('${block.id}')">
                <div class="item-header">
                    <span class="item-id">${truncateId(block.id)}</span>
                    <span class="item-height">#${formatNumber(block.height)}</span>
                </div>
                <div class="item-details">
                    <span>${block.txCount} txs</span>
                    <span>${formatTimestamp(block.timestamp)}</span>
                </div>
            </div>
        `).join('');
    }

    // Load latest transactions
    const txs = await fetchApi('/transactions?limit=10');
    if (txs && txs.items) {
        const container = document.getElementById('latest-txs');
        container.innerHTML = txs.items.map(tx => `
            <div class="list-item" onclick="showTxDetail('${tx.id}')">
                <div class="item-header">
                    <span class="item-id">${truncateId(tx.id)}</span>
                    <span class="item-height">#${formatNumber(tx.inclusionHeight)}</span>
                </div>
                <div class="item-details">
                    <span>${tx.inputCount} in / ${tx.outputCount} out</span>
                    <span>${tx.size} bytes</span>
                </div>
            </div>
        `).join('');
    }
}

// Status page
async function loadStatusData() {
    const status = await fetch('/status').then(r => r.json()).catch(() => null);
    if (!status) return;

    // Sync status
    const progress = (status.sync.syncProgress * 100).toFixed(2);
    document.getElementById('sync-progress').style.width = `${progress}%`;
    document.getElementById('sync-text').textContent = status.sync.isSyncing
        ? `Syncing... ${progress}%`
        : `Synced (${progress}%)`;
    document.getElementById('local-height').textContent = formatNumber(status.sync.localHeight);
    document.getElementById('node-height').textContent = formatNumber(status.sync.nodeHeight);
    document.getElementById('blocks-per-sec').textContent = status.sync.blocksPerSecond?.toFixed(2) || '-';
    document.getElementById('sync-eta').textContent = status.sync.etaSeconds
        ? formatDuration(status.sync.etaSeconds)
        : '-';

    // Nodes
    const nodeList = document.getElementById('node-list');
    nodeList.innerHTML = status.sync.connectedNodes.map(node => `
        <div class="node-item">
            <span class="node-url">${node.url}</span>
            <div class="node-status">
                <span class="node-latency">${node.latencyMs ? node.latencyMs + 'ms' : '-'}</span>
                <span class="status-dot ${node.connected ? 'connected' : 'disconnected'}"></span>
            </div>
        </div>
    `).join('');

    // Database
    document.getElementById('db-blocks').textContent = formatNumber(status.database.blockCount);
    document.getElementById('db-txs').textContent = formatNumber(status.database.txCount);
    document.getElementById('db-boxes').textContent = formatNumber(status.database.boxCount);
    document.getElementById('db-tokens').textContent = formatNumber(status.database.tokenCount);
    document.getElementById('db-size').textContent = formatBytes(status.database.sizeBytes);

    // System
    document.getElementById('sys-version').textContent = status.system.version;
    document.getElementById('sys-network').textContent = status.system.network;
    document.getElementById('sys-uptime').textContent = formatDuration(status.system.uptimeSeconds);
    document.getElementById('sys-memory').textContent = status.system.memoryUsageMb
        ? `${status.system.memoryUsageMb} MB`
        : '-';
}

// Wallet page
async function loadWalletData() {
    const status = await fetchApi('/wallet/status');

    const statusDot = document.querySelector('.wallet-status .status-dot');
    const statusText = document.querySelector('.wallet-status .status-text');

    if (!status || status.error) {
        statusDot.classList.remove('connected');
        statusDot.classList.add('disconnected');
        statusText.textContent = status?.error || 'Node unavailable';
        document.getElementById('wallet-locked').classList.remove('hidden');
        document.getElementById('wallet-unlocked').classList.add('hidden');
        return;
    }

    if (status.unlocked) {
        statusDot.classList.add('connected');
        statusDot.classList.remove('disconnected');
        statusText.textContent = 'Unlocked';
        document.getElementById('wallet-locked').classList.add('hidden');
        document.getElementById('wallet-unlocked').classList.remove('hidden');

        // Load balances
        const balances = await fetchApi('/wallet/balances');
        if (balances && balances.balance !== undefined) {
            document.getElementById('wallet-balance').textContent =
                `${nanoErgToErg(balances.balance)} ERG`;
        }

        // Load addresses
        const addresses = await fetchApi('/wallet/addresses');
        if (addresses) {
            const list = document.getElementById('wallet-address-list');
            list.innerHTML = addresses.map(addr =>
                `<div class="address-item">${addr}</div>`
            ).join('');
        }
    } else {
        statusDot.classList.remove('connected', 'disconnected');
        statusText.textContent = status.initialized ? 'Locked' : 'Not initialized';
        document.getElementById('wallet-locked').classList.remove('hidden');
        document.getElementById('wallet-unlocked').classList.add('hidden');
    }
}

// Wallet actions
async function unlockWallet() {
    const password = document.getElementById('wallet-password').value;
    const result = await postApi('/wallet/unlock', { pass: password });
    if (result && result.success) {
        loadWalletData();
    } else {
        alert('Failed to unlock wallet');
    }
}

async function lockWallet() {
    await postApi('/wallet/lock', {});
    loadWalletData();
}

async function sendTransaction() {
    const to = document.getElementById('send-to').value;
    const amount = parseFloat(document.getElementById('send-amount').value);

    if (!to || !amount) {
        alert('Please enter recipient and amount');
        return;
    }

    const nanoErgs = Math.floor(amount * 1e9);
    const result = await postApi('/wallet/transaction/send', {
        requests: [{
            address: to,
            value: nanoErgs,
            assets: []
        }]
    });

    if (result && result.id) {
        alert(`Transaction sent: ${result.id}`);
        document.getElementById('send-to').value = '';
        document.getElementById('send-amount').value = '';
        loadWalletData();
    } else {
        alert('Failed to send transaction');
    }
}

// Search
async function performSearch() {
    const query = document.getElementById('searchInput').value.trim();
    if (!query) return;

    const results = await fetchApi(`/search?query=${encodeURIComponent(query)}`);
    if (!results || results.length === 0) {
        alert('No results found');
        return;
    }

    const modal = document.getElementById('search-modal');
    const container = document.getElementById('search-results');

    container.innerHTML = results.map(r => {
        let display = '';
        if (r.block) {
            display = `Block #${r.block.height}`;
        } else if (r.transaction) {
            display = `Height: ${r.transaction.inclusionHeight}`;
        } else if (r.address) {
            display = `Balance: ${nanoErgToErg(r.address.balance.nanoErgs)} ERG`;
        } else if (r.token) {
            display = r.token.name || 'Unknown token';
        }

        return `
            <div class="search-result" onclick="handleSearchResult('${r.entityType}', '${r.entityId}')">
                <div class="result-type">${r.entityType}</div>
                <div class="result-id">${r.entityId}</div>
                <div class="result-info">${display}</div>
            </div>
        `;
    }).join('');

    modal.classList.remove('hidden');
}

function handleSearchResult(type, id) {
    closeModal('search-modal');

    switch (type) {
        case 'block':
            showBlockDetail(id);
            break;
        case 'transaction':
        case 'box':
            showTxDetail(id);
            break;
        case 'address':
            showAddressDetail(id);
            break;
        case 'token':
            showTokenDetail(id);
            break;
    }
}

// Detail views
async function showBlockDetail(blockId) {
    const block = await fetchApi(`/blocks/${blockId}`);
    if (!block) return;

    const txs = await fetchApi(`/transactions/byBlock/${blockId}?limit=10`);

    const modal = document.getElementById('detail-modal');
    document.getElementById('detail-title').textContent = `Block #${block.height}`;

    document.getElementById('detail-content').innerHTML = `
        <div class="detail-section">
            <h4>Block Info</h4>
            <div class="detail-row">
                <span class="detail-label">Block ID</span>
                <span class="detail-value">${block.id}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Parent</span>
                <span class="detail-value">
                    <a href="#" onclick="showBlockDetail('${block.parentId}')">${truncateId(block.parentId)}</a>
                </span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Height</span>
                <span class="detail-value">${formatNumber(block.height)}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Timestamp</span>
                <span class="detail-value">${formatTimestamp(block.timestamp)}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Transactions</span>
                <span class="detail-value">${block.txCount}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Size</span>
                <span class="detail-value">${formatBytes(block.blockSize)}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Difficulty</span>
                <span class="detail-value">${formatNumber(block.difficulty)}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Miner</span>
                <span class="detail-value">
                    ${block.minerAddress ? `<a href="#" onclick="showAddressDetail('${block.minerAddress}')">${truncateId(block.minerAddress)}</a>` : '-'}
                </span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Reward</span>
                <span class="detail-value">${nanoErgToErg(block.minerReward)} ERG</span>
            </div>
        </div>
        <div class="detail-section">
            <h4>Transactions</h4>
            ${txs?.items?.map(tx => `
                <div class="detail-row" style="cursor:pointer" onclick="showTxDetail('${tx.id}')">
                    <span class="detail-value" style="color: var(--accent-primary)">${truncateId(tx.id)}</span>
                    <span class="detail-label">${tx.inputCount}→${tx.outputCount}</span>
                </div>
            `).join('') || 'No transactions'}
        </div>
    `;

    modal.classList.remove('hidden');
}

async function showTxDetail(txId) {
    const tx = await fetchApi(`/transactions/${txId}`);
    if (!tx) return;

    const modal = document.getElementById('detail-modal');
    document.getElementById('detail-title').textContent = 'Transaction';

    document.getElementById('detail-content').innerHTML = `
        <div class="detail-section">
            <h4>Transaction Info</h4>
            <div class="detail-row">
                <span class="detail-label">TX ID</span>
                <span class="detail-value">${tx.id}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Block</span>
                <span class="detail-value">
                    <a href="#" onclick="showBlockDetail('${tx.blockId}')">${truncateId(tx.blockId)}</a>
                </span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Height</span>
                <span class="detail-value">${formatNumber(tx.inclusionHeight)}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Timestamp</span>
                <span class="detail-value">${formatTimestamp(tx.timestamp)}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Size</span>
                <span class="detail-value">${tx.size} bytes</span>
            </div>
        </div>
        <div class="detail-section">
            <h4>Inputs (${tx.inputs?.length || 0})</h4>
            ${tx.inputs?.map(inp => `
                <div class="detail-row">
                    <span class="detail-value" style="font-size:0.75rem">${truncateId(inp.boxId)}</span>
                    <span class="detail-label">${inp.value ? nanoErgToErg(inp.value) + ' ERG' : '-'}</span>
                </div>
            `).join('') || 'Coinbase'}
        </div>
        <div class="detail-section">
            <h4>Outputs (${tx.outputs?.length || 0})</h4>
            ${tx.outputs?.map(out => `
                <div class="detail-row">
                    <span class="detail-value" style="font-size:0.75rem">
                        <a href="#" onclick="showAddressDetail('${out.address}')">${truncateId(out.address)}</a>
                    </span>
                    <span class="detail-label">${nanoErgToErg(out.value)} ERG</span>
                </div>
            `).join('') || '-'}
        </div>
    `;

    modal.classList.remove('hidden');
}

async function showAddressDetail(address) {
    const info = await fetchApi(`/addresses/${address}`);
    if (!info) return;

    const txs = await fetchApi(`/addresses/${address}/transactions?limit=10`);

    const modal = document.getElementById('detail-modal');
    document.getElementById('detail-title').textContent = 'Address';

    document.getElementById('detail-content').innerHTML = `
        <div class="detail-section">
            <h4>Address Info</h4>
            <div class="detail-row">
                <span class="detail-label">Address</span>
                <span class="detail-value">${info.address}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Balance</span>
                <span class="detail-value">${nanoErgToErg(info.balance.nanoErgs)} ERG</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Transactions</span>
                <span class="detail-value">${formatNumber(info.txCount)}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">First Seen</span>
                <span class="detail-value">Block #${info.firstSeenHeight || '-'}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Last Seen</span>
                <span class="detail-value">Block #${info.lastSeenHeight || '-'}</span>
            </div>
        </div>
        ${info.balance.tokens?.length > 0 ? `
        <div class="detail-section">
            <h4>Tokens (${info.balance.tokens.length})</h4>
            ${info.balance.tokens.slice(0, 10).map(token => `
                <div class="detail-row">
                    <span class="detail-value" style="font-size:0.75rem">
                        <a href="#" onclick="showTokenDetail('${token.tokenId}')">${token.name || truncateId(token.tokenId)}</a>
                    </span>
                    <span class="detail-label">${formatNumber(token.amount)}</span>
                </div>
            `).join('')}
        </div>
        ` : ''}
        <div class="detail-section">
            <h4>Recent Transactions</h4>
            ${txs?.items?.map(tx => `
                <div class="detail-row" style="cursor:pointer" onclick="showTxDetail('${tx.id}')">
                    <span class="detail-value" style="color: var(--accent-primary)">${truncateId(tx.id)}</span>
                    <span class="detail-label">${formatTimestamp(tx.timestamp)}</span>
                </div>
            `).join('') || 'No transactions'}
        </div>
    `;

    modal.classList.remove('hidden');
}

async function showTokenDetail(tokenId) {
    const token = await fetchApi(`/tokens/${tokenId}`);
    if (!token) return;

    const holders = await fetchApi(`/tokens/${tokenId}/holders?limit=10`);

    const modal = document.getElementById('detail-modal');
    document.getElementById('detail-title').textContent = token.name || 'Token';

    document.getElementById('detail-content').innerHTML = `
        <div class="detail-section">
            <h4>Token Info</h4>
            <div class="detail-row">
                <span class="detail-label">Token ID</span>
                <span class="detail-value">${token.id}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Name</span>
                <span class="detail-value">${token.name || '-'}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Description</span>
                <span class="detail-value">${token.description || '-'}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Decimals</span>
                <span class="detail-value">${token.decimals ?? '-'}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Total Supply</span>
                <span class="detail-value">${formatNumber(token.emissionAmount)}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Created at Height</span>
                <span class="detail-value">${formatNumber(token.creationHeight)}</span>
            </div>
        </div>
        <div class="detail-section">
            <h4>Top Holders</h4>
            ${holders?.items?.map(h => `
                <div class="detail-row">
                    <span class="detail-value" style="font-size:0.75rem">
                        <a href="#" onclick="showAddressDetail('${h.address}')">${truncateId(h.address)}</a>
                    </span>
                    <span class="detail-label">${formatNumber(h.balance)}</span>
                </div>
            `).join('') || 'No holders'}
        </div>
    `;

    modal.classList.remove('hidden');
}

// Modal handling
function closeModal(modalId) {
    document.getElementById(modalId).classList.add('hidden');
}

// Event listeners
document.addEventListener('DOMContentLoaded', () => {
    // Navigation
    document.querySelectorAll('.nav-link[data-page]').forEach(link => {
        link.addEventListener('click', (e) => {
            e.preventDefault();
            navigateTo(link.dataset.page);
        });
    });

    // Search
    document.getElementById('searchBtn').addEventListener('click', performSearch);
    document.getElementById('searchInput').addEventListener('keypress', (e) => {
        if (e.key === 'Enter') performSearch();
    });

    // Wallet actions
    document.getElementById('unlock-btn').addEventListener('click', unlockWallet);
    document.getElementById('lock-btn').addEventListener('click', lockWallet);
    document.getElementById('send-btn').addEventListener('click', sendTransaction);

    // Modal close buttons
    document.querySelectorAll('.modal-close').forEach(btn => {
        btn.addEventListener('click', () => {
            btn.closest('.modal').classList.add('hidden');
        });
    });

    // Close modal on background click
    document.querySelectorAll('.modal').forEach(modal => {
        modal.addEventListener('click', (e) => {
            if (e.target === modal) {
                modal.classList.add('hidden');
            }
        });
    });

    // Initial load
    loadExplorerData();

    // Auto-refresh
    setInterval(() => {
        const activePage = document.querySelector('.page.active').id.replace('-page', '');
        if (activePage === 'explorer') loadExplorerData();
        if (activePage === 'status') loadStatusData();
    }, 10000);
});

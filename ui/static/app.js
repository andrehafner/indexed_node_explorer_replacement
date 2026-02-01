// Ergo Index Explorer - Frontend Application

const API_BASE = '/api/v1';

// NFT Image CDN URLs
const NFT_CDN = {
    auctionHouse: 'https://f003.backblazeb2.com/file/auctionhouse-mainnet/original/',
    ipfs: 'https://ipfs.io/ipfs/',
    nautilusIcons: 'https://raw.githubusercontent.com/nautls/nautilus-wallet/master/public/icons/assets/'
};

// Cache for token images
const tokenImageCache = new Map();

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

function formatTimeAgo(ts) {
    if (!ts) return '-';
    const seconds = Math.floor((Date.now() - ts) / 1000);
    if (seconds < 60) return 'just now';
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
    return `${Math.floor(seconds / 86400)}d ago`;
}

function truncateId(id, len = 16) {
    if (!id || id.length <= len) return id;
    return id.substring(0, len / 2) + '...' + id.substring(id.length - len / 2);
}

function nanoErgToErg(nanoErg) {
    if (!nanoErg) return '0';
    return (nanoErg / 1e9).toFixed(4);
}

function formatTokenAmount(amount, decimals = 0) {
    if (!amount) return '0';
    if (decimals > 0) {
        return (amount / Math.pow(10, decimals)).toFixed(decimals);
    }
    return formatNumber(amount);
}

function escapeHtml(text) {
    if (!text) return '';
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

function copyToClipboard(text) {
    navigator.clipboard.writeText(text).then(() => {
        // Show brief feedback
        const btn = event.target;
        const originalText = btn.textContent;
        btn.textContent = '✓';
        setTimeout(() => btn.textContent = originalText, 1000);
    });
}

// NFT Image URL generation
function isNFT(token) {
    // NFT: emission amount = 1, decimals = 0
    return token.emissionAmount === 1 && (!token.decimals || token.decimals === 0);
}

function getNFTImageUrl(tokenId) {
    // Primary: Ergo Auction House CDN
    return `${NFT_CDN.auctionHouse}${tokenId}`;
}

function getTokenIconUrl(tokenName) {
    // Try Nautilus wallet icons
    if (!tokenName) return null;
    const name = tokenName.toLowerCase().replace(/\s+/g, '');
    return `${NFT_CDN.nautilusIcons}${name}.png`;
}

function getIPFSUrl(hash) {
    if (!hash) return null;
    // Handle ipfs:// protocol
    if (hash.startsWith('ipfs://')) {
        hash = hash.replace('ipfs://', '');
    }
    return `${NFT_CDN.ipfs}${hash}`;
}

// Extract image URL from token description or registers
function extractImageUrl(token) {
    if (!token) return null;

    // Check description for URLs
    if (token.description) {
        const urlMatch = token.description.match(/https?:\/\/[^\s"'<>]+\.(png|jpg|jpeg|gif|webp|svg)/i);
        if (urlMatch) return urlMatch[0];

        // Check for IPFS hash
        const ipfsMatch = token.description.match(/ipfs:\/\/([a-zA-Z0-9]+)/i);
        if (ipfsMatch) return getIPFSUrl(ipfsMatch[1]);

        // Check for raw IPFS hash (Qm... or bafy...)
        const rawIpfsMatch = token.description.match(/(Qm[a-zA-Z0-9]{44,}|bafy[a-zA-Z0-9]+)/);
        if (rawIpfsMatch) return getIPFSUrl(rawIpfsMatch[1]);
    }

    return null;
}

// Load token image with fallbacks
async function loadTokenImage(token, imgElement, size = 'small') {
    const tokenId = token.id || token.tokenId;

    // Check cache first
    if (tokenImageCache.has(tokenId)) {
        const cachedUrl = tokenImageCache.get(tokenId);
        if (cachedUrl) {
            imgElement.src = cachedUrl;
            return true;
        }
        return false;
    }

    const urls = [];

    if (isNFT(token)) {
        // For NFTs, try auction house first
        urls.push(getNFTImageUrl(tokenId));
    } else {
        // For fungible tokens, try Nautilus icons first
        const iconUrl = getTokenIconUrl(token.name);
        if (iconUrl) urls.push(iconUrl);
    }

    // Try extracted URL from description
    const extractedUrl = extractImageUrl(token);
    if (extractedUrl) urls.push(extractedUrl);

    // Try each URL
    for (const url of urls) {
        try {
            const loaded = await testImageLoad(url);
            if (loaded) {
                tokenImageCache.set(tokenId, url);
                imgElement.src = url;
                return true;
            }
        } catch (e) {
            // Continue to next URL
        }
    }

    // No image found
    tokenImageCache.set(tokenId, null);
    return false;
}

function testImageLoad(url, timeout = 5000) {
    return new Promise((resolve) => {
        const img = new Image();
        const timer = setTimeout(() => {
            img.src = '';
            resolve(false);
        }, timeout);
        img.onload = () => {
            clearTimeout(timer);
            resolve(true);
        };
        img.onerror = () => {
            clearTimeout(timer);
            resolve(false);
        };
        img.src = url;
    });
}

// API calls with timeout
async function fetchApi(endpoint, timeoutMs = 10000) {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

    try {
        const res = await fetch(`${API_BASE}${endpoint}`, {
            signal: controller.signal
        });
        clearTimeout(timeoutId);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return await res.json();
    } catch (e) {
        clearTimeout(timeoutId);
        if (e.name === 'AbortError') {
            console.warn(`API timeout: ${endpoint} (sync may be running)`);
        } else {
            console.error(`API error: ${endpoint}`, e);
        }
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

// Page navigation with URL hash persistence
function navigateTo(page, updateHash = true) {
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    document.querySelectorAll('.nav-link').forEach(l => l.classList.remove('active'));

    const pageEl = document.getElementById(`${page}-page`);
    const linkEl = document.querySelector(`[data-page="${page}"]`);

    if (pageEl) pageEl.classList.add('active');
    if (linkEl) linkEl.classList.add('active');

    // Update URL hash for persistence
    if (updateHash) {
        window.location.hash = page;
    }

    // Load page data
    if (page === 'explorer') loadExplorerData();
    if (page === 'status') loadStatusData();
    if (page === 'wallet') loadWalletData();
    if (page === 'tokens') loadTokensData();
}

// Get page from URL hash
function getPageFromHash() {
    const hash = window.location.hash.slice(1); // Remove #
    const validPages = ['explorer', 'tokens', 'wallet', 'status', 'api'];
    return validPages.includes(hash) ? hash : 'explorer';
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
                    <span>${formatTimeAgo(block.timestamp)}</span>
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
                    <span>${formatTimeAgo(tx.timestamp)}</span>
                </div>
            </div>
        `).join('');
    }
}

// Tokens page
let currentTokenFilter = 'all';
let tokensData = [];

async function loadTokensData() {
    const container = document.getElementById('tokens-list');
    if (!container) return;

    container.innerHTML = '<div class="loading">Loading tokens...</div>';

    const result = await fetchApi('/tokens?limit=50', 15000);
    if (result && result.items) {
        tokensData = result.items;
        renderTokensList();
    } else {
        container.innerHTML = `
            <div class="loading">
                <p>Unable to load tokens</p>
                <p style="font-size:0.8rem;margin-top:0.5rem;color:var(--text-secondary)">
                    If sync is running, database queries may be slow. Try again in a moment.
                </p>
                <button class="btn btn-secondary btn-small" style="margin-top:1rem" onclick="loadTokensData()">Retry</button>
            </div>
        `;
    }
}

function filterTokens(filter) {
    currentTokenFilter = filter;

    // Update button states
    document.querySelectorAll('.filter-btn').forEach(btn => {
        btn.classList.remove('active');
        if (btn.dataset.filter === filter) btn.classList.add('active');
    });

    renderTokensList();
}

function renderTokensList() {
    const container = document.getElementById('tokens-list');
    if (!container) return;

    let filtered = tokensData;
    if (currentTokenFilter === 'nft') {
        filtered = tokensData.filter(t => isNFT(t));
    } else if (currentTokenFilter === 'fungible') {
        filtered = tokensData.filter(t => !isNFT(t));
    }

    if (filtered.length === 0) {
        container.innerHTML = '<div class="loading">No tokens found</div>';
        return;
    }

    container.innerHTML = `
        <div class="token-cards-grid">
            ${filtered.map(token => renderTokenCard(token)).join('')}
        </div>
    `;

    // Load images after render
    filtered.forEach(token => {
        const img = document.getElementById(`token-img-${token.id}`);
        if (img) {
            loadTokenImage(token, img);
        }
    });
}

function renderTokenCard(token) {
    const isNft = isNFT(token);
    const displayName = token.name || truncateId(token.id);
    const firstLetter = (token.name || token.id || '?')[0].toUpperCase();

    return `
        <div class="token-card ${isNft ? 'nft' : ''}" onclick="showTokenDetail('${token.id}')">
            <div class="token-card-icon">
                <img id="token-img-${token.id}"
                     src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7"
                     alt=""
                     onerror="this.style.display='none'; this.nextElementSibling.style.display='flex';"
                     style="display:none;">
                <span>${firstLetter}</span>
            </div>
            <div class="token-card-info">
                <div class="token-card-name">${escapeHtml(displayName)}</div>
                <div class="token-card-amount">
                    ${isNft ? 'NFT' : formatTokenAmount(token.emissionAmount, token.decimals)}
                </div>
                <div class="token-card-id">${truncateId(token.id, 12)}</div>
            </div>
        </div>
    `;
}

// Status page
async function loadStatusData() {
    const status = await fetch('/status').then(r => r.json()).catch(() => null);
    if (!status) return;

    // Sync status
    const progress = (status.sync.syncProgress * 100).toFixed(2);
    document.getElementById('sync-progress').style.width = `${progress}%`;
    document.getElementById('sync-percentage').textContent = `${progress}%`;

    const syncBadge = document.getElementById('sync-status-badge');
    if (status.sync.isSyncing) {
        syncBadge.textContent = 'Syncing';
        syncBadge.classList.remove('synced');
    } else {
        syncBadge.textContent = 'Synced';
        syncBadge.classList.add('synced');
    }

    document.getElementById('local-height').textContent = formatNumber(status.sync.localHeight);
    document.getElementById('node-height').textContent = formatNumber(status.sync.nodeHeight);
    document.getElementById('blocks-per-sec').textContent = status.sync.blocksPerSecond?.toFixed(2) || '-';
    document.getElementById('sync-eta').textContent = status.sync.etaSeconds
        ? formatDuration(status.sync.etaSeconds)
        : '-';

    // Primary Node Info (first connected node)
    const primaryNode = status.sync.connectedNodes.find(n => n.connected) || status.sync.connectedNodes[0];
    const nodeBadge = document.getElementById('node-status-badge');

    if (primaryNode && primaryNode.connected) {
        nodeBadge.textContent = 'Connected';
        nodeBadge.classList.remove('offline');

        document.getElementById('node-version').textContent = primaryNode.appVersion || '-';
        document.getElementById('node-state-type').textContent = primaryNode.stateType || '-';
        document.getElementById('node-headers-height').textContent = primaryNode.headersHeight
            ? formatNumber(primaryNode.headersHeight)
            : '-';
        document.getElementById('node-max-peer-height').textContent = primaryNode.maxPeerHeight
            ? formatNumber(primaryNode.maxPeerHeight)
            : '-';
        document.getElementById('node-peers').textContent = primaryNode.peersCount ?? '-';
        document.getElementById('node-mempool').textContent = primaryNode.unconfirmedCount ?? '-';
        document.getElementById('node-mining').textContent = primaryNode.isMining !== null
            ? (primaryNode.isMining ? 'Yes' : 'No')
            : '-';
        document.getElementById('node-difficulty').textContent = primaryNode.difficulty
            ? formatLargeDifficulty(primaryNode.difficulty)
            : '-';
    } else {
        nodeBadge.textContent = 'Offline';
        nodeBadge.classList.add('offline');
    }

    // Connected nodes count
    const connectedCount = status.sync.connectedNodes.filter(n => n.connected).length;
    document.getElementById('connected-node-count').textContent = `${connectedCount}/${status.sync.connectedNodes.length}`;

    // Nodes list - detailed card for each node
    const nodeList = document.getElementById('node-list');
    nodeList.innerHTML = status.sync.connectedNodes.map((node, idx) => `
        <div class="node-card ${node.connected ? 'connected' : 'disconnected'}">
            <div class="node-card-header">
                <div class="node-card-title">
                    <span class="status-dot ${node.connected ? 'connected' : 'disconnected'}"></span>
                    <span class="node-url">${node.url}</span>
                    ${idx === 0 ? '<span class="primary-badge">Primary</span>' : ''}
                </div>
                <span class="node-status-tag ${node.connected ? 'online' : 'offline'}">${node.connected ? 'Online' : 'Offline'}</span>
            </div>
            <div class="node-card-grid">
                <div class="node-info-item">
                    <span class="node-info-label">Version</span>
                    <span class="node-info-value">${node.appVersion || '-'}</span>
                </div>
                <div class="node-info-item">
                    <span class="node-info-label">State</span>
                    <span class="node-info-value">${node.stateType || '-'}</span>
                </div>
                <div class="node-info-item">
                    <span class="node-info-label">Height</span>
                    <span class="node-info-value">${node.height ? formatNumber(node.height) : '-'}</span>
                </div>
                <div class="node-info-item">
                    <span class="node-info-label">Headers</span>
                    <span class="node-info-value">${node.headersHeight ? formatNumber(node.headersHeight) : '-'}</span>
                </div>
                <div class="node-info-item">
                    <span class="node-info-label">Peers</span>
                    <span class="node-info-value">${node.peersCount ?? '-'}</span>
                </div>
                <div class="node-info-item">
                    <span class="node-info-label">Mempool</span>
                    <span class="node-info-value">${node.unconfirmedCount ?? '-'}</span>
                </div>
                <div class="node-info-item">
                    <span class="node-info-label">Latency</span>
                    <span class="node-info-value">${node.latencyMs ? node.latencyMs + 'ms' : '-'}</span>
                </div>
                <div class="node-info-item">
                    <span class="node-info-label">Mining</span>
                    <span class="node-info-value">${node.isMining !== null && node.isMining !== undefined ? (node.isMining ? 'Yes' : 'No') : '-'}</span>
                </div>
            </div>
            ${node.difficulty ? `
            <div class="node-card-footer">
                <span class="node-info-label">Difficulty:</span>
                <span class="node-info-value">${formatLargeDifficulty(node.difficulty)}</span>
            </div>
            ` : ''}
        </div>
    `).join('');

    // Database
    document.getElementById('db-blocks').textContent = formatNumber(status.database.blockCount);
    document.getElementById('db-txs').textContent = formatNumber(status.database.txCount);
    document.getElementById('db-boxes').textContent = formatNumber(status.database.boxCount);
    document.getElementById('db-tokens').textContent = formatNumber(status.database.tokenCount);
    document.getElementById('db-size').textContent = formatBytes(status.database.sizeBytes);

    // Load table sizes
    loadTableSizes();

    // System
    document.getElementById('sys-version').textContent = status.system.version;
    document.getElementById('sys-network').textContent = status.system.network;
    document.getElementById('sys-uptime').textContent = formatDuration(status.system.uptimeSeconds);
    document.getElementById('sys-memory').textContent = status.system.memoryUsageMb
        ? `${status.system.memoryUsageMb} MB`
        : '-';
}

// Load table sizes from API
async function loadTableSizes() {
    const sizes = await fetchApi('/stats/tables');
    const container = document.getElementById('indexes-list');

    if (!sizes || !Array.isArray(sizes) || sizes.length === 0) {
        container.innerHTML = '<div class="no-data">Table info unavailable</div>';
        return;
    }

    container.innerHTML = sizes.map(table => `
        <div class="index-item">
            <span class="index-name">${table.name}</span>
            <div class="index-stats">
                <span class="index-rows">${formatNumber(table.rowCount)} rows</span>
                <span class="index-size">${formatBytes(table.sizeBytes)}</span>
            </div>
        </div>
    `).join('');
}

function formatLargeDifficulty(diffStr) {
    if (!diffStr) return '-';
    const num = parseFloat(diffStr);
    if (isNaN(num)) return diffStr;
    if (num >= 1e15) return (num / 1e15).toFixed(2) + ' P';
    if (num >= 1e12) return (num / 1e12).toFixed(2) + ' T';
    if (num >= 1e9) return (num / 1e9).toFixed(2) + ' G';
    if (num >= 1e6) return (num / 1e6).toFixed(2) + ' M';
    if (num >= 1e3) return (num / 1e3).toFixed(2) + ' K';
    return formatNumber(num);
}

// Wallet page
async function loadWalletData() {
    const status = await fetchApi('/wallet/status');

    const statusIndicator = document.querySelector('.wallet-status .status-indicator');
    const statusText = document.querySelector('.wallet-status .status-text');
    const lockedSection = document.getElementById('wallet-locked');
    const helpCard = document.getElementById('wallet-help');

    if (!status || status.error) {
        statusIndicator.classList.remove('connected');
        statusIndicator.classList.add('disconnected');

        // Check for common errors
        let errorMsg = status?.error || 'Node unavailable';
        let helpTitle = 'Connection Error';

        if (errorMsg.includes('401') || errorMsg.includes('Unauthorized') || errorMsg.includes('api_key')) {
            errorMsg = 'API Key Required';
            helpTitle = 'Authentication Required';
        } else if (errorMsg.includes('timeout') || errorMsg.includes('connect')) {
            errorMsg = 'Node Unavailable';
        }

        statusText.textContent = errorMsg;

        // Show the locked section with error styling
        lockedSection.innerHTML = `
            <div class="wallet-locked-content">
                <div class="lock-icon" style="background: rgba(239, 68, 68, 0.15);">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="color: var(--accent-error);">
                        <circle cx="12" cy="12" r="10"></circle>
                        <line x1="15" y1="9" x2="9" y2="15"></line>
                        <line x1="9" y1="9" x2="15" y2="15"></line>
                    </svg>
                </div>
                <h3>${helpTitle}</h3>
                <p class="unlock-description">${errorMsg}</p>
            </div>
        `;
        lockedSection.classList.remove('hidden');
        document.getElementById('wallet-unlocked').classList.add('hidden');
        helpCard.classList.remove('hidden');
        return;
    }

    if (status.unlocked) {
        statusIndicator.classList.add('connected');
        statusIndicator.classList.remove('disconnected');
        statusText.textContent = 'Unlocked';
        lockedSection.classList.add('hidden');
        document.getElementById('wallet-unlocked').classList.remove('hidden');
        helpCard.classList.add('hidden');

        // Load balances
        const balances = await fetchApi('/wallet/balances');
        if (balances) {
            // Handle different response formats - balance could be at top level or nested
            const balance = balances.balance ?? balances.confirmed?.nanoErgs ?? 0;
            document.getElementById('wallet-balance').textContent =
                `${nanoErgToErg(balance)} ERG`;

            // Display wallet tokens - could be 'assets' or 'tokens'
            const assets = balances.assets || balances.tokens || [];
            renderWalletTokens(assets);
            document.getElementById('token-count').textContent = assets.length;
        }

        // Load addresses
        let addresses = await fetchApi('/wallet/addresses');
        const list = document.getElementById('wallet-address-list');

        // Normalize addresses - could be array of strings or array of objects with 'address' field
        if (addresses && Array.isArray(addresses)) {
            addresses = addresses.map(addr => typeof addr === 'object' ? addr.address : addr);
        } else {
            addresses = [];
        }

        // Use changeAddress from status as fallback if no addresses returned
        if (addresses.length === 0 && status.changeAddress) {
            addresses = [status.changeAddress];
        }

        document.getElementById('address-count').textContent = addresses.length;
        if (addresses.length > 0) {
            list.innerHTML = addresses.map(addr =>
                `<div class="address-item" onclick="showAddressDetail('${addr}')">${addr}</div>`
            ).join('');
        } else {
            list.innerHTML = '<div class="no-data">No addresses found</div>';
        }
    } else {
        statusIndicator.classList.remove('connected', 'disconnected');
        statusText.textContent = status.initialized ? 'Locked' : 'Not initialized';

        // Restore the unlock form HTML
        lockedSection.innerHTML = `
            <div class="wallet-locked-content">
                <div class="lock-icon">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                        <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
                    </svg>
                </div>
                <h3>Unlock Your Wallet</h3>
                <p class="unlock-description">Enter your wallet password to access your funds and manage transactions.</p>
                <div class="unlock-form">
                    <input type="password" id="wallet-password" placeholder="Wallet password">
                    <button id="unlock-btn" class="btn btn-primary btn-large">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                            <path d="M7 11V7a5 5 0 0 1 9.9-1"></path>
                        </svg>
                        Unlock Wallet
                    </button>
                </div>
            </div>
        `;
        // Re-attach event listener
        document.getElementById('unlock-btn')?.addEventListener('click', unlockWallet);

        lockedSection.classList.remove('hidden');
        document.getElementById('wallet-unlocked').classList.add('hidden');
        helpCard.classList.remove('hidden');
    }
}

// Render wallet token holdings
function renderWalletTokens(assets) {
    const container = document.getElementById('wallet-tokens');
    if (!container) return;

    if (!assets || assets.length === 0) {
        container.innerHTML = `
            <div style="text-align:center;color:var(--text-secondary);padding:2rem">
                <p>No tokens in wallet</p>
            </div>
        `;
        return;
    }

    container.innerHTML = assets.map(asset => {
        const tokenId = asset.tokenId;
        const name = asset.name || truncateId(tokenId, 12);
        const firstLetter = (asset.name || tokenId || '?')[0].toUpperCase();
        const amount = formatTokenAmount(asset.amount, asset.decimals || 0);
        const isNft = asset.amount === 1 && (!asset.decimals || asset.decimals === 0);

        return `
            <div class="wallet-token-item ${isNft ? 'nft' : ''}" onclick="showTokenDetail('${tokenId}')">
                <div class="wallet-token-icon">
                    <img id="wallet-token-img-${tokenId}"
                         src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7"
                         alt=""
                         style="display:none;"
                         onerror="this.style.display='none';">
                    <span>${firstLetter}</span>
                </div>
                <div class="wallet-token-info">
                    <div class="wallet-token-name">${escapeHtml(name)}</div>
                    <div class="wallet-token-amount">${isNft ? 'NFT' : amount}</div>
                </div>
            </div>
        `;
    }).join('');

    // Try to load token images
    assets.forEach(asset => {
        const img = document.getElementById(`wallet-token-img-${asset.tokenId}`);
        if (img) {
            const token = { id: asset.tokenId, name: asset.name, emissionAmount: asset.amount, decimals: asset.decimals };
            loadTokenImage(token, img).then(loaded => {
                if (loaded) {
                    img.style.display = 'block';
                    img.nextElementSibling.style.display = 'none';
                }
            });
        }
    });
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

    // If single exact match, go directly to detail
    if (results.length === 1) {
        handleSearchResult(results[0].entityType, results[0].entityId);
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
                <span class="detail-value">
                    ${block.id}
                    <button class="copy-btn" onclick="copyToClipboard('${block.id}')">📋</button>
                </span>
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
        <div class="external-links">
            <a href="https://explorer.ergoplatform.com/en/blocks/${block.id}" target="_blank" class="external-link">Ergo Explorer ↗</a>
            <a href="https://ergexplorer.com/block/${block.id}" target="_blank" class="external-link">ErgExplorer ↗</a>
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
                <span class="detail-value">
                    ${tx.id}
                    <button class="copy-btn" onclick="copyToClipboard('${tx.id}')">📋</button>
                </span>
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
        <div class="external-links">
            <a href="https://explorer.ergoplatform.com/en/transactions/${tx.id}" target="_blank" class="external-link">Ergo Explorer ↗</a>
            <a href="https://ergexplorer.com/tx/${tx.id}" target="_blank" class="external-link">ErgExplorer ↗</a>
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

    const tokens = info.balance?.tokens || [];
    const nfts = tokens.filter(t => t.amount === 1);
    const fungibles = tokens.filter(t => t.amount !== 1);

    document.getElementById('detail-content').innerHTML = `
        <div class="address-balance-hero">
            <div>
                <div class="address-balance-erg">${nanoErgToErg(info.balance?.nanoErgs || 0)} ERG</div>
            </div>
            <div class="address-stats-mini">
                <div class="address-stat">
                    <div class="address-stat-value">${formatNumber(info.txCount || 0)}</div>
                    <div class="address-stat-label">Transactions</div>
                </div>
                <div class="address-stat">
                    <div class="address-stat-value">${tokens.length}</div>
                    <div class="address-stat-label">Tokens</div>
                </div>
            </div>
        </div>

        <div class="detail-section">
            <h4>Address</h4>
            <div class="detail-row">
                <span class="detail-value" style="max-width:100%; font-size:0.8rem">
                    ${info.address}
                    <button class="copy-btn" onclick="copyToClipboard('${info.address}')">📋</button>
                </span>
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

        ${nfts.length > 0 ? `
        <div class="detail-section">
            <div class="section-header">
                <span class="section-title">NFTs</span>
                <span class="section-count">${nfts.length}</span>
            </div>
            <div class="nft-gallery" id="address-nfts">
                ${nfts.slice(0, 8).map(nft => `
                    <div class="nft-thumb" onclick="showTokenDetail('${nft.tokenId}')">
                        <div class="nft-thumb-image">
                            <img id="nft-thumb-${nft.tokenId}"
                                 src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7"
                                 alt=""
                                 onerror="this.style.display='none'; this.nextElementSibling.style.display='flex';">
                            <span class="placeholder" style="display:none">🖼️</span>
                        </div>
                        <div class="nft-thumb-info">
                            <div class="nft-thumb-name">${escapeHtml(nft.name) || truncateId(nft.tokenId, 8)}</div>
                        </div>
                    </div>
                `).join('')}
            </div>
        </div>
        ` : ''}

        ${fungibles.length > 0 ? `
        <div class="detail-section">
            <div class="section-header">
                <span class="section-title">Tokens</span>
                <span class="section-count">${fungibles.length}</span>
            </div>
            <div class="token-cards-grid">
                ${fungibles.slice(0, 6).map(token => `
                    <div class="token-card" onclick="showTokenDetail('${token.tokenId}')">
                        <div class="token-card-icon">
                            <img id="token-card-${token.tokenId}"
                                 src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7"
                                 alt=""
                                 onerror="this.style.display='none';">
                            <span>${(token.name || token.tokenId || '?')[0].toUpperCase()}</span>
                        </div>
                        <div class="token-card-info">
                            <div class="token-card-name">${escapeHtml(token.name) || truncateId(token.tokenId, 8)}</div>
                            <div class="token-card-amount">${formatTokenAmount(token.amount, token.decimals)}</div>
                        </div>
                    </div>
                `).join('')}
            </div>
        </div>
        ` : ''}

        <div class="detail-section">
            <h4>Recent Transactions</h4>
            ${txs?.items?.map(tx => `
                <div class="tx-item" onclick="showTxDetail('${tx.id}')">
                    <div class="tx-item-left">
                        <span class="tx-item-id">${truncateId(tx.id)}</span>
                        <span class="tx-item-time">${formatTimeAgo(tx.timestamp)}</span>
                    </div>
                </div>
            `).join('') || 'No transactions'}
        </div>

        <div class="external-links">
            <a href="https://explorer.ergoplatform.com/en/addresses/${address}" target="_blank" class="external-link">Ergo Explorer ↗</a>
            <a href="https://ergexplorer.com/address/${address}" target="_blank" class="external-link">ErgExplorer ↗</a>
        </div>
    `;

    modal.classList.remove('hidden');

    // Load NFT images
    nfts.slice(0, 8).forEach(nft => {
        const img = document.getElementById(`nft-thumb-${nft.tokenId}`);
        if (img) {
            img.src = getNFTImageUrl(nft.tokenId);
            img.onload = () => { img.style.display = 'block'; };
            img.onerror = () => {
                img.style.display = 'none';
                img.nextElementSibling.style.display = 'flex';
            };
        }
    });
}

async function showTokenDetail(tokenId) {
    const token = await fetchApi(`/tokens/${tokenId}`);
    if (!token) return;

    const holders = await fetchApi(`/tokens/${tokenId}/holders?limit=10`);
    const isNft = isNFT(token);

    const modal = document.getElementById('detail-modal');
    document.getElementById('detail-title').textContent = token.name || 'Token';

    const displayName = token.name || 'Unknown Token';
    const firstLetter = displayName[0].toUpperCase();

    document.getElementById('detail-content').innerHTML = `
        <div class="token-hero">
            <div class="token-icon-box" id="token-detail-icon">
                <span>${firstLetter}</span>
            </div>
            <div class="token-info">
                <div class="token-name-row">
                    <span class="token-name">${escapeHtml(displayName)}</span>
                    <span class="badge ${isNft ? 'badge-nft' : 'badge-token'}">${isNft ? 'NFT' : 'Token'}</span>
                </div>
                <div class="token-id-row">
                    <span class="token-id">${token.id}</span>
                    <button class="copy-btn" onclick="copyToClipboard('${token.id}')">📋</button>
                </div>
                ${token.description ? `<div class="token-description">${escapeHtml(token.description)}</div>` : ''}
            </div>
        </div>

        ${isNft ? `
        <div class="nft-preview">
            <div class="nft-image-container loading" id="nft-image-container">
                <img id="nft-detail-image"
                     src="${getNFTImageUrl(tokenId)}"
                     alt="${escapeHtml(displayName)}"
                     onload="this.parentElement.classList.remove('loading'); document.getElementById('nft-loading-text')?.remove();"
                     onerror="tryNextImageSource(this, '${tokenId}');">
                <div class="nft-loading-text" id="nft-loading-text">Loading image...</div>
            </div>
            <div class="nft-footer">
                <span class="nft-source" id="nft-source">Ergo Auction House</span>
                <div class="nft-actions">
                    <button class="btn btn-secondary btn-small" onclick="window.open('${getNFTImageUrl(tokenId)}', '_blank')">Open Original</button>
                    <a href="https://ergoauctions.org/artwork/${tokenId}" target="_blank" class="btn btn-secondary btn-small">Ergo Auctions ↗</a>
                </div>
            </div>
        </div>
        ` : ''}

        <div class="detail-section">
            <h4>Token Details</h4>
            <div class="detail-row">
                <span class="detail-label">Total Supply</span>
                <span class="detail-value">${formatTokenAmount(token.emissionAmount, token.decimals)}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Decimals</span>
                <span class="detail-value">${token.decimals ?? 0}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Created at Height</span>
                <span class="detail-value">${formatNumber(token.creationHeight)}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Minting Box</span>
                <span class="detail-value">${truncateId(token.boxId)}</span>
            </div>
        </div>

        <div class="detail-section">
            <h4>Top Holders</h4>
            ${holders?.items?.map(h => `
                <div class="detail-row">
                    <span class="detail-value" style="font-size:0.75rem">
                        <a href="#" onclick="showAddressDetail('${h.address}')">${truncateId(h.address)}</a>
                    </span>
                    <span class="detail-label">${formatTokenAmount(h.balance, token.decimals)}</span>
                </div>
            `).join('') || 'No holders found'}
        </div>

        <div class="external-links">
            <a href="https://explorer.ergoplatform.com/en/tokens/${tokenId}" target="_blank" class="external-link">Ergo Explorer ↗</a>
            <a href="https://ergexplorer.com/token/${tokenId}" target="_blank" class="external-link">ErgExplorer ↗</a>
            ${isNft ? `<a href="https://ergoauctions.org/artwork/${tokenId}" target="_blank" class="external-link">Ergo Auctions ↗</a>` : ''}
            <a href="https://sigmaspace.io/token/${tokenId}" target="_blank" class="external-link">SigmaSpace ↗</a>
        </div>
    `;

    modal.classList.remove('hidden');

    // Load token icon for fungible tokens
    if (!isNft && token.name) {
        const iconBox = document.getElementById('token-detail-icon');
        const iconUrl = getTokenIconUrl(token.name);
        if (iconUrl) {
            const img = new Image();
            img.onload = () => {
                iconBox.innerHTML = `<img src="${iconUrl}" alt="">`;
            };
            img.src = iconUrl;
        }
    }
}

// NFT image fallback chain
let imageSourceIndex = {};

function tryNextImageSource(imgElement, tokenId) {
    const sources = [
        { url: getNFTImageUrl(tokenId), name: 'Ergo Auction House' },
        { url: `https://ipfs.io/ipfs/${tokenId}`, name: 'IPFS' },
    ];

    const currentIndex = imageSourceIndex[tokenId] || 0;
    const nextIndex = currentIndex + 1;

    if (nextIndex < sources.length) {
        imageSourceIndex[tokenId] = nextIndex;
        imgElement.src = sources[nextIndex].url;
        const sourceLabel = document.getElementById('nft-source');
        if (sourceLabel) sourceLabel.textContent = sources[nextIndex].name;
    } else {
        // All sources failed
        imgElement.parentElement.classList.remove('loading');
        imgElement.parentElement.innerHTML = '<div class="placeholder" style="font-size:4rem;color:var(--accent-nft)">🖼️</div>';
        const loadingText = document.getElementById('nft-loading-text');
        if (loadingText) loadingText.textContent = 'Image not available';
    }
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

    // Handle browser back/forward with hash changes
    window.addEventListener('hashchange', () => {
        navigateTo(getPageFromHash(), false);
    });

    // Search
    document.getElementById('searchBtn').addEventListener('click', performSearch);
    document.getElementById('searchInput').addEventListener('keypress', (e) => {
        if (e.key === 'Enter') performSearch();
    });

    // Wallet actions
    document.getElementById('unlock-btn')?.addEventListener('click', unlockWallet);
    document.getElementById('lock-btn')?.addEventListener('click', lockWallet);
    document.getElementById('send-btn')?.addEventListener('click', sendTransaction);

    // Token filters
    document.querySelectorAll('.filter-btn[data-filter]').forEach(btn => {
        btn.addEventListener('click', () => filterTokens(btn.dataset.filter));
    });

    // Modal close buttons
    document.querySelectorAll('.modal-close').forEach(btn => {
        btn.addEventListener('click', () => {
            btn.closest('.modal').classList.add('hidden');
        });
    });

    // Close modal on backdrop click
    document.querySelectorAll('.modal-backdrop').forEach(backdrop => {
        backdrop.addEventListener('click', () => {
            backdrop.closest('.modal').classList.add('hidden');
        });
    });

    // Initial load - navigate to page from URL hash or default to explorer
    const initialPage = getPageFromHash();
    navigateTo(initialPage, false);

    // Auto-refresh
    setInterval(() => {
        const activePage = document.querySelector('.page.active')?.id?.replace('-page', '');
        if (activePage === 'explorer') loadExplorerData();
        if (activePage === 'status') loadStatusData();
    }, 10000);
});

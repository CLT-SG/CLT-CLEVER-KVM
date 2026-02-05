/**
 * CLEVER KVM Relay - Dashboard JavaScript
 */

// Auto-refresh interval (5 seconds)
const REFRESH_INTERVAL = 5000;
let refreshTimer = null;

/**
 * Initialize dashboard
 */
document.addEventListener('DOMContentLoaded', () => {
    // Start auto-refresh
    startAutoRefresh();
    
    // Initial refresh
    refreshDevices();
});

/**
 * Start auto-refresh timer
 */
function startAutoRefresh() {
    if (refreshTimer) {
        clearInterval(refreshTimer);
    }
    refreshTimer = setInterval(refreshDevices, REFRESH_INTERVAL);
}

/**
 * Refresh devices list
 */
async function refreshDevices() {
    try {
        // Fetch devices
        const devicesResponse = await fetch('/api/devices');
        const devices = await devicesResponse.json();
        
        // Fetch stats
        const statsResponse = await fetch('/api/stats');
        const stats = await statsResponse.json();
        
        // Update stats
        updateStats(stats);
        
        // Update devices grid
        updateDevicesGrid(devices);
    } catch (error) {
        console.error('Failed to refresh devices:', error);
    }
}

/**
 * Update statistics display
 */
function updateStats(stats) {
    document.getElementById('total-devices').textContent = stats.total_devices;
    document.getElementById('online-devices').textContent = stats.online_devices;
    document.getElementById('streaming-devices').textContent = stats.streaming_devices;
    document.getElementById('total-viewers').textContent = stats.total_viewers;
}

/**
 * Update devices grid
 */
function updateDevicesGrid(devices) {
    const grid = document.getElementById('devices-grid');
    const noDevices = document.getElementById('no-devices');
    
    if (devices.length === 0) {
        grid.innerHTML = '';
        noDevices.style.display = 'block';
        return;
    }
    
    noDevices.style.display = 'none';
    
    // Build device cards HTML
    const html = devices.map(device => renderDeviceCard(device)).join('\n');
    grid.innerHTML = html;
}

/**
 * Render a single device card
 */
function renderDeviceCard(device) {
    const stateClass = getStateClass(device.state);
    const onlineClass = device.is_online ? 'online' : 'offline';
    const onlineText = device.is_online ? 'Online' : 'Offline';
    const stateText = getStateText(device.state);
    const disabled = device.is_online ? '' : 'disabled';
    
    return `
        <div class="device-card ${stateClass} ${onlineClass}">
            <div class="device-header">
                <div class="device-icon">🖥️</div>
                <div class="device-info">
                    <h3 class="device-name">${escapeHtml(device.display_name)}</h3>
                    <p class="device-hostname">${escapeHtml(device.hostname)}.local</p>
                </div>
                <div class="device-status">
                    <span class="status-dot ${onlineClass}"></span>
                    <span class="status-text">${onlineText}</span>
                </div>
            </div>
            <div class="device-details">
                <div class="detail-row">
                    <span class="detail-label">OS:</span>
                    <span class="detail-value">${escapeHtml(device.os)}</span>
                </div>
                <div class="detail-row">
                    <span class="detail-label">Resolution:</span>
                    <span class="detail-value">${device.stream_config.width}x${device.stream_config.height}</span>
                </div>
                <div class="detail-row">
                    <span class="detail-label">State:</span>
                    <span class="detail-value state-${stateClass}">${stateText}</span>
                </div>
                <div class="detail-row">
                    <span class="detail-label">Viewers:</span>
                    <span class="detail-value">${device.viewer_count}</span>
                </div>
            </div>
            <div class="device-actions">
                <a href="/kvm?hostname=${encodeURIComponent(device.hostname)}" 
                   class="action-btn primary" ${disabled}>
                    <span>📺</span> View
                </a>
            </div>
        </div>
    `;
}

/**
 * Get CSS class for device state
 */
function getStateClass(state) {
    if (typeof state === 'object') {
        if ('Error' in state) return 'error';
    }
    switch (state) {
        case 'Streaming': return 'streaming';
        case 'Paused': return 'paused';
        case 'Idle': return 'idle';
        default: return 'idle';
    }
}

/**
 * Get display text for device state
 */
function getStateText(state) {
    if (typeof state === 'object') {
        if ('Error' in state) return state.Error;
    }
    return state;
}

/**
 * Escape HTML special characters
 */
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

/**
 * Format bytes to human-readable string
 */
function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

/**
 * Format time ago
 */
function formatTimeAgo(date) {
    const seconds = Math.floor((new Date() - new Date(date)) / 1000);
    
    if (seconds < 60) return 'Just now';
    if (seconds < 3600) return Math.floor(seconds / 60) + ' min ago';
    if (seconds < 86400) return Math.floor(seconds / 3600) + ' hours ago';
    return Math.floor(seconds / 86400) + ' days ago';
}

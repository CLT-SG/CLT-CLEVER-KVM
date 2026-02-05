/**
 * CLEVER KVM - Relay Client
 * 
 * WebSocket-based KVM client that connects to devices through the relay server.
 */

class RelayKVMClient {
    constructor(config) {
        this.config = config;
        this.ws = null;
        this.connected = false;
        this.h264Decoder = null;
        
        // Display elements
        this.videoScreen = document.getElementById('video-screen');
        this.canvasLayer = document.getElementById('canvas-layer');
        this.statusDisplay = document.getElementById('status-display');
        this.osdOverlay = document.querySelector('.osd-overlay');
        this.networkStats = document.getElementById('network-stats');
        
        // Stats
        this.frameCount = 0;
        this.lastFpsUpdate = Date.now();
        this.latency = 0;
        this.bitrate = 0;
        this.lastPingTime = 0;
        
        // Stream dimensions
        this.streamWidth = config.width || 1920;
        this.streamHeight = config.height || 1080;
        
        // OSD state
        this.osdVisible = true;
        this.mouseIdleTimer = null;
        
        // Canvas for H.264 rendering
        this.renderCanvas = null;
        this.renderCtx = null;
        
        this.initialize();
    }
    
    /**
     * Initialize the client
     */
    initialize() {
        this.setupCanvas();
        this.initializeH264Decoder();
        this.setupEventListeners();
        this.connect();
    }
    
    /**
     * Setup canvas for rendering
     */
    setupCanvas() {
        // Use canvas for H.264 frame rendering
        this.renderCanvas = document.createElement('canvas');
        this.renderCanvas.width = this.streamWidth;
        this.renderCanvas.height = this.streamHeight;
        this.renderCtx = this.renderCanvas.getContext('2d', {
            alpha: false,
            desynchronized: true
        });
        
        // Setup visible canvas
        if (this.canvasLayer) {
            this.canvasLayer.width = this.streamWidth;
            this.canvasLayer.height = this.streamHeight;
            this.canvasCtx = this.canvasLayer.getContext('2d', {
                alpha: false,
                desynchronized: true
            });
        }
    }
    
    /**
     * Initialize H.264 decoder
     */
    initializeH264Decoder() {
        if (typeof H264Decoder !== 'undefined') {
            this.h264Decoder = new H264Decoder({
                width: this.streamWidth,
                height: this.streamHeight,
                onFrame: (frame, metadata) => this.handleDecodedFrame(frame, metadata),
                onError: (error) => console.error('H.264 decode error:', error),
                onReady: () => {
                    console.log('✅ H.264 decoder ready');
                    this.hideStatus();
                }
            });
        } else {
            console.warn('⚠️ H264Decoder not available');
        }
    }
    
    /**
     * Handle decoded H.264 frame
     */
    handleDecodedFrame(frame, metadata) {
        if (frame instanceof VideoFrame) {
            // WebCodecs VideoFrame - render directly
            if (this.canvasCtx) {
                this.canvasCtx.drawImage(frame, 0, 0);
            }
            frame.close();
        } else if (frame instanceof ImageBitmap) {
            if (this.canvasCtx) {
                this.canvasCtx.drawImage(frame, 0, 0);
            }
            frame.close();
        } else if (frame instanceof ImageData) {
            if (this.canvasCtx) {
                this.canvasCtx.putImageData(frame, 0, 0);
            }
        }
        
        this.frameCount++;
        this.updateStats();
    }
    
    /**
     * Setup event listeners
     */
    setupEventListeners() {
        // Mouse movement for OSD
        document.addEventListener('mousemove', () => this.handleMouseActivity());
        document.addEventListener('click', () => this.handleMouseActivity());
        
        // OSD controls
        const fullscreenBtn = document.getElementById('fullscreen-btn');
        if (fullscreenBtn) {
            fullscreenBtn.addEventListener('click', () => this.toggleFullscreen());
        }
        
        const disconnectBtn = document.getElementById('disconnect-btn');
        if (disconnectBtn) {
            disconnectBtn.addEventListener('click', () => this.disconnect());
        }
        
        // Input handlers
        this.setupInputHandlers();
        
        // Keyboard shortcuts
        document.addEventListener('keydown', (e) => {
            if (e.key === 'F11') {
                e.preventDefault();
                this.toggleFullscreen();
            } else if (e.key === 'Escape' && document.fullscreenElement) {
                document.exitFullscreen();
            }
        });
    }
    
    /**
     * Setup input handlers for remote control
     */
    setupInputHandlers() {
        const screen = document.getElementById('screen');
        if (!screen) return;
        
        // Mouse events
        screen.addEventListener('mousedown', (e) => this.handleMouseEvent(e, 'mousedown'));
        screen.addEventListener('mouseup', (e) => this.handleMouseEvent(e, 'mouseup'));
        screen.addEventListener('mousemove', (e) => this.handleMouseEvent(e, 'mousemove'));
        screen.addEventListener('wheel', (e) => this.handleWheelEvent(e), { passive: false });
        
        // Prevent context menu
        screen.addEventListener('contextmenu', (e) => e.preventDefault());
        
        // Keyboard events
        document.addEventListener('keydown', (e) => this.handleKeyEvent(e, 'keydown'));
        document.addEventListener('keyup', (e) => this.handleKeyEvent(e, 'keyup'));
    }
    
    /**
     * Handle mouse events
     */
    handleMouseEvent(e, type) {
        if (!this.connected || !this.ws) return;
        
        const rect = this.canvasLayer?.getBoundingClientRect() || { left: 0, top: 0, width: this.streamWidth, height: this.streamHeight };
        const scaleX = this.streamWidth / rect.width;
        const scaleY = this.streamHeight / rect.height;
        
        const x = Math.round((e.clientX - rect.left) * scaleX);
        const y = Math.round((e.clientY - rect.top) * scaleY);
        
        let msg;
        switch (type) {
            case 'mousedown':
            case 'mouseup':
                msg = { type, x, y, button: this.getMouseButton(e.button) };
                break;
            case 'mousemove':
                msg = { type, x, y };
                break;
        }
        
        if (msg) {
            this.sendMessage(msg);
        }
    }
    
    /**
     * Handle wheel events
     */
    handleWheelEvent(e) {
        if (!this.connected || !this.ws) return;
        e.preventDefault();
        
        const rect = this.canvasLayer?.getBoundingClientRect() || { left: 0, top: 0 };
        const x = Math.round(e.clientX - rect.left);
        const y = Math.round(e.clientY - rect.top);
        
        this.sendMessage({
            type: 'wheel',
            x,
            y,
            delta_x: e.deltaX,
            delta_y: e.deltaY
        });
    }
    
    /**
     * Handle keyboard events
     */
    handleKeyEvent(e, type) {
        if (!this.connected || !this.ws) return;
        
        // Allow some keys to pass through
        if (e.key === 'F11' || (e.key === 'Escape' && document.fullscreenElement)) {
            return;
        }
        
        // Prevent default for most keys when connected
        e.preventDefault();
        
        this.sendMessage({
            type,
            key: e.key,
            key_code: e.keyCode
        });
    }
    
    /**
     * Get mouse button name
     */
    getMouseButton(button) {
        switch (button) {
            case 0: return 'left';
            case 1: return 'middle';
            case 2: return 'right';
            default: return 'left';
        }
    }
    
    /**
     * Connect to relay server
     */
    connect() {
        this.showStatus('Connecting', `Establishing connection to ${this.config.deviceHostname}...`);
        
        const wsUrl = this.config.wsUrl;
        console.log(`🔌 Connecting to relay server: ${wsUrl}`);
        
        this.ws = new WebSocket(wsUrl);
        this.ws.binaryType = 'arraybuffer';
        
        this.ws.onopen = () => {
            console.log('✅ Connected to relay server');
            this.connected = true;
            this.showStatus('Connected', 'Waiting for video stream...');
            this.startPing();
        };
        
        this.ws.onmessage = (event) => {
            if (event.data instanceof ArrayBuffer) {
                this.handleBinaryMessage(event.data);
            } else {
                this.handleTextMessage(event.data);
            }
        };
        
        this.ws.onclose = (event) => {
            console.log('🔌 Disconnected from relay server');
            this.connected = false;
            this.showStatus('Disconnected', 'Connection to relay server closed.');
            this.stopPing();
        };
        
        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
            this.showStatus('Error', 'Failed to connect to relay server.');
        };
    }
    
    /**
     * Disconnect from relay server
     */
    disconnect() {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
        this.connected = false;
        window.location.href = '/';
    }
    
    /**
     * Handle binary message (video frame)
     */
    handleBinaryMessage(data) {
        const bytes = new Uint8Array(data);
        
        // Check for H.264 magic header
        if (bytes.length > 4) {
            const magic = String.fromCharCode(bytes[0], bytes[1], bytes[2], bytes[3]);
            if (magic === 'H264' && this.h264Decoder) {
                this.h264Decoder.decode(bytes);
                return;
            }
        }
        
        // Fallback: try to decode as H.264 anyway
        if (this.h264Decoder) {
            this.h264Decoder.decode(bytes);
        }
    }
    
    /**
     * Handle text message (control/status)
     */
    handleTextMessage(data) {
        try {
            const msg = JSON.parse(data);
            
            switch (msg.type) {
                case 'connected':
                    console.log(`✅ Connected to device: ${msg.device_name}`);
                    this.streamWidth = msg.width;
                    this.streamHeight = msg.height;
                    this.setupCanvas();
                    this.hideStatus();
                    break;
                    
                case 'stream_init':
                    console.log(`📺 Stream initialized: ${msg.width}x${msg.height} @ ${msg.framerate}fps`);
                    this.streamWidth = msg.width;
                    this.streamHeight = msg.height;
                    this.setupCanvas();
                    if (this.h264Decoder) {
                        this.h264Decoder.updateDimensions(msg.width, msg.height);
                    }
                    this.hideStatus();
                    break;
                    
                case 'pong':
                    this.latency = Date.now() - msg.timestamp;
                    break;
                    
                case 'error':
                    console.error('Server error:', msg.message);
                    this.showStatus('Error', msg.message);
                    break;
                    
                case 'device_offline':
                    this.showStatus('Device Offline', 'The device has disconnected.');
                    break;
            }
        } catch (e) {
            console.error('Failed to parse message:', e);
        }
    }
    
    /**
     * Send message to server
     */
    sendMessage(msg) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(msg));
        }
    }
    
    /**
     * Start ping interval
     */
    startPing() {
        this.pingInterval = setInterval(() => {
            if (this.connected) {
                this.lastPingTime = Date.now();
                this.sendMessage({ type: 'ping', timestamp: this.lastPingTime });
            }
        }, 1000);
    }
    
    /**
     * Stop ping interval
     */
    stopPing() {
        if (this.pingInterval) {
            clearInterval(this.pingInterval);
            this.pingInterval = null;
        }
    }
    
    /**
     * Update statistics display
     */
    updateStats() {
        const now = Date.now();
        const elapsed = now - this.lastFpsUpdate;
        
        if (elapsed >= 1000) {
            const fps = Math.round(this.frameCount * 1000 / elapsed);
            this.frameCount = 0;
            this.lastFpsUpdate = now;
            
            // Update display
            const fpsElement = document.getElementById('fps');
            const latencyElement = document.getElementById('latency');
            const statFps = document.getElementById('stat-fps');
            const statLatency = document.getElementById('stat-latency');
            
            if (fpsElement) fpsElement.textContent = fps;
            if (latencyElement) latencyElement.textContent = this.latency;
            if (statFps) statFps.textContent = fps;
            if (statLatency) statLatency.textContent = this.latency;
        }
    }
    
    /**
     * Show status display
     */
    showStatus(title, message) {
        if (this.statusDisplay) {
            this.statusDisplay.querySelector('h2').textContent = title;
            this.statusDisplay.querySelector('p').textContent = message;
            this.statusDisplay.classList.remove('hidden');
        }
    }
    
    /**
     * Hide status display
     */
    hideStatus() {
        if (this.statusDisplay) {
            this.statusDisplay.classList.add('hidden');
        }
    }
    
    /**
     * Handle mouse activity for OSD
     */
    handleMouseActivity() {
        this.showOSD();
        
        if (this.mouseIdleTimer) {
            clearTimeout(this.mouseIdleTimer);
        }
        
        this.mouseIdleTimer = setTimeout(() => {
            this.hideOSD();
        }, 3000);
    }
    
    /**
     * Show OSD
     */
    showOSD() {
        if (this.osdOverlay) {
            this.osdOverlay.classList.add('visible');
        }
        document.getElementById('screen')?.classList.add('show-cursor');
    }
    
    /**
     * Hide OSD
     */
    hideOSD() {
        if (this.osdOverlay) {
            this.osdOverlay.classList.remove('visible');
        }
        document.getElementById('screen')?.classList.remove('show-cursor');
    }
    
    /**
     * Toggle fullscreen
     */
    toggleFullscreen() {
        const screen = document.getElementById('screen');
        if (!screen) return;
        
        if (document.fullscreenElement) {
            document.exitFullscreen();
        } else {
            screen.requestFullscreen();
        }
    }
}

// Initialize client when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    if (window.KVM_CONFIG) {
        window.kvmClient = new RelayKVMClient(window.KVM_CONFIG);
    } else {
        console.error('KVM_CONFIG not found');
    }
});

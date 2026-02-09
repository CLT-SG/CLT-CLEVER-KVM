class KVMClient {
    constructor(config) {
        this.config = config;
        this.ws = null;
        this.connected = false;
        this.lastFrame = 0;
        this.frameCount = 0;
        this.lastFpsUpdate = Date.now();
        // Initialize with sensible defaults - will be updated from server_info or first frame
        this.screenWidth = 1920;
        this.screenHeight = 1080;
        this.latency = 0;
        this.lastPingTime = 0;
        this.pingInterval = null;
        this.qualityLevel = 85;
        this.availableMonitors = [];
        this.currentMonitor = config.monitor;
        this.currentCodec = "h264"; // Use H.264 for low latency hardware-accelerated streaming
        this.videoQueue = [];
        this.showStats = false;
        
        // H.264 decoder for low-latency streaming
        this.h264Decoder = null;
        this.h264SPS = null;
        this.h264PPS = null;

        // VP8/VP9 decoder (RustDesk-inspired, primary codec)
        this.vpxDecoder = null;
        this.serverCodec = 'vp9'; // Will be updated from server_info
        this.protocolVersion = 1; // v2 = rdengine binary protocol
        
        // Canvas for frame rendering
        this.decoderCanvas = null;
        this.decoderCtx = null;
        
        // Video properties
        this.needsKeyframe = true;
        this.supportsHardwareDecoding = false;
        
        // OSD state
        this.osdVisible = true;
        this.osdTimer = null;
        this.mouseIdleTimer = null;
        this.lastMouseMove = Date.now();
        
        // Connection health monitoring
        this.lastFrameTime = Date.now();
        this.connectionHealthInterval = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 5;
        
        // Multi-touch and gesture support
        this.touchIdentifiers = new Map();
        this.gestureInProgress = false;
        this.initialTouchDistance = 0;
        this.initialTouchAngle = 0;

        // WebRTC for audio
        this.peerConnection = null;
        this.audioStream = null;

        // ── Host cursor synchronization ──────────────────────────────────
        // Tracks the remote host cursor position and shape so the client
        // can render it. When the host is actively moving the cursor,
        // the client's own cursor interactions are visually suppressed.
        this.hostCursor = {
            x: 0,
            y: 0,
            shape: 'default',      // CSS cursor name
            visible: true,
            lastUpdate: 0,         // Timestamp of last host cursor update
            isHostControlling: false, // True when host is actively moving
        };
        // How long (ms) after the last host cursor move before client regains control
        this.hostControlTimeoutMs = 500;
        // Timer ID for host-control expiry
        this.hostControlTimer = null;

        this.initializeElements();
        this.initializeH264Decoder();
        this.initializeVpxDecoder();
        this.initializeFrameTracking();
        this.setupEventListeners();
        this.connect();
    }
    
    // Initialize VP8/VP9 decoder (RustDesk-inspired primary codec)
    initializeVpxDecoder() {
        console.log('Initializing VP8/VP9 decoder...');
        
        if (typeof VpxDecoder !== 'undefined') {
            this.vpxDecoder = new VpxDecoder({
                width: this.screenWidth,
                height: this.screenHeight,
                codec: this.serverCodec || 'vp9',
                onFrame: (frame) => this.handleVpxFrame(frame),
                onError: (error) => {
                    if (error && error.message === 'delta_decode_failed') {
                        // Request keyframe from server
                        this.requestKeyframe();
                    } else {
                        // Any decoder error (including hardware decode failure)
                        // — request keyframe to recover
                        console.warn('VPX decode error, requesting keyframe:', error?.message || error);
                        this.requestKeyframe();
                    }
                },
                onReady: () => {
                    console.log('VP8/VP9 decoder ready (WebCodecs:', this.vpxDecoder?.useWebCodecs, ')');
                    this.supportsHardwareDecoding = this.vpxDecoder?.useWebCodecs || this.supportsHardwareDecoding;
                }
            });
        } else {
            console.warn('VpxDecoder not loaded — VP8/VP9 decoding unavailable');
        }
    }

    // Handle decoded VP8/VP9 frame
    handleVpxFrame(frame) {
        if (!this.realCanvas || !this.realCtx) {
            this.initializeOptimizedCanvas(this.screenWidth, this.screenHeight);
        }
        
        if (frame instanceof VideoFrame) {
            this.realCtx.drawImage(frame, 0, 0);
            frame.close();
        }
        
        this.updateFrameStats();
    }

    // Initialize H.264 decoder
    initializeH264Decoder() {
        console.log('Initializing H.264 decoder...');
        
        // Check if H264Decoder class is available
        if (typeof H264Decoder !== 'undefined') {
            this.h264Decoder = new H264Decoder({
                width: this.screenWidth,
                height: this.screenHeight,
                onFrame: (frame, metadata) => this.handleH264Frame(frame, metadata),
                onError: (error) => {
                    // Only log actual decode errors, not initialization warnings
                    if (error && error.message) {
                        console.warn('H.264 decode warning:', error.message);
                    }
                },
                onReady: () => {
                    console.log('✅ H.264 decoder ready');
                    this.supportsHardwareDecoding = this.h264Decoder?.useWebCodecs || false;
                }
            });
        } else {
            console.warn('⚠️ H264Decoder not loaded, will use fallback');
        }
    }
    
    // Handle decoded H.264 frame
    handleH264Frame(frame, metadata) {
        if (!this.realCanvas || !this.realCtx) {
            this.initializeOptimizedCanvas(this.screenWidth, this.screenHeight);
        }
        
        if (frame instanceof VideoFrame) {
            // WebCodecs VideoFrame - render directly
            this.realCtx.drawImage(frame, 0, 0);
            frame.close(); // Important: close to free resources
        } else if (frame instanceof ImageBitmap) {
            // Software-decoded ImageBitmap
            this.realCtx.drawImage(frame, 0, 0);
            frame.close();
        } else if (frame instanceof ImageData) {
            // Raw ImageData
            this.realCtx.putImageData(frame, 0, 0);
        }
        
        this.updateFrameStats();
    }

    // Initialize frame tracking variables
    initializeFrameTracking() {
        this.frameLogCounter = 0;
        this.previousFrameData = null;
        this.realCanvas = null;
        this.realCtx = null;
        
        // High-performance frame pipeline
        this.frameQueue = [];
        this.maxQueueSize = 3; // Aggressive frame dropping for low latency
        this.isDecompressing = false;
        this.lastFrameTime = Date.now();
        this.targetFrameTime = 16.67; // 60 FPS = 16.67ms per frame
        
        // Performance monitoring
        this.perfStats = {
            decompressTime: 0,
            renderTime: 0,
            totalFrames: 0,
            droppedFrames: 0,
            lastStatsUpdate: 0
        };
        
        // Adaptive quality system
        this.adaptiveQuality = {
            enabled: true,
            currentLevel: 'high',  // high, medium, low
            performanceHistory: [],
            lastAdjustment: 0,
            adjustmentInterval: 2000  // Adjust every 2 seconds max
        };
        
        // Use OffscreenCanvas if available for background processing
        this.useOffscreenCanvas = typeof OffscreenCanvas !== 'undefined';
        if (this.useOffscreenCanvas) {
            console.log('🚀 Using OffscreenCanvas for background rendering');
        }
    }

    initializeElements() {
        // Main elements - H.264 uses canvas for display with WebCodecs decoder
        this.videoScreen = document.getElementById('video-screen');
        this.canvasLayer = document.getElementById('canvas-layer'); // Used only for input handling
        this.audioElement = document.getElementById('remote-audio');
        
        // OSD elements
        this.osdOverlay = document.querySelector('.osd-overlay');
        this.statusDisplay = document.querySelector('.status-display');
        this.osdTitle = document.querySelector('.osd-title');
        this.networkStats = document.querySelector('.network-stats');
        this.gestureIndicator = document.querySelector('.gesture-indicator');
        this.notificationArea = document.querySelector('.notification-area');
        
        // Controls
        this.monitorDropdown = document.getElementById('monitor-dropdown');
        this.codecDropdown = document.getElementById('codec-dropdown');
        this.qualityDropdown = document.getElementById('quality-dropdown');
        this.settingsPanel = document.querySelector('.settings-panel');
        
        // WebRTC quality tracking
        this.currentQuality = 'medium';
        this.adaptiveQuality = true;
        this.networkStats = {
            bandwidth: 0,
            latency: 0,
            packetLoss: 0
        };
        this.frameStats = {
            framesReceived: 0,
            keyframesReceived: 0,
            totalBytes: 0,
            currentFps: 0,
            lastFrameCount: 0,
            lastFpsUpdate: Date.now()
        };
        
        // Settings controls - check if they exist before using
        this.settingStretch = document.getElementById('setting-stretch');
        this.settingAudio = document.getElementById('setting-audio');
        this.settingMute = document.getElementById('setting-mute');
        this.settingStats = document.getElementById('setting-stats');
        
        // Fix: Use bitrate-slider instead of quality-slider
        this.bitrateSlider = document.getElementById('bitrate-slider');
        this.bitrateValue = document.getElementById('bitrate-value');

        // Initialize settings from config - only if elements exist
        if (this.settingStretch) this.settingStretch.checked = this.config.stretch;
        if (this.settingAudio) this.settingAudio.checked = this.config.audio;
        if (this.settingMute) this.settingMute.checked = this.config.mute;
        if (this.codecDropdown) this.codecDropdown.value = this.config.codec;
        
        if (this.audioElement) this.audioElement.muted = this.config.mute;
        
        // Ensure video element is visible
        if (this.videoScreen) {
            this.videoScreen.style.display = 'block';
        }
    }

    setupEventListeners() {
        // OSD auto-hide functionality
        document.addEventListener('mousemove', (e) => {
            this.handleMouseActivity();
        });

        document.addEventListener('click', () => {
            this.handleMouseActivity();
        });

        document.addEventListener('keydown', (e) => {
            this.handleMouseActivity();
        });

        // Screen interactions
        this.setupInputHandlers();
        
        // Control buttons - check if they exist
        const fullscreenBtn = document.getElementById('fullscreen-btn');
        if (fullscreenBtn) {
            fullscreenBtn.addEventListener('click', () => {
                this.toggleFullscreen();
            });
        }

        const settingsBtn = document.getElementById('settings-btn');
        if (settingsBtn) {
            settingsBtn.addEventListener('click', () => {
                this.toggleSettings();
            });
        }

        const disconnectBtn = document.getElementById('disconnect-btn');
        if (disconnectBtn) {
            disconnectBtn.addEventListener('click', () => {
                this.disconnect();
            });
        }

        // Settings panel
        const settingsSave = document.getElementById('settings-save');
        if (settingsSave) {
            settingsSave.addEventListener('click', () => {
                this.saveSettings();
            });
        }

        const settingsCancel = document.getElementById('settings-cancel');
        if (settingsCancel) {
            settingsCancel.addEventListener('click', () => {
                this.hideSettings();
            });
        }

        const closeButton = document.querySelector('.close-button');
        if (closeButton) {
            closeButton.addEventListener('click', () => {
                this.hideSettings();
            });
        }

        // Settings controls - only add listeners if elements exist
        if (this.bitrateSlider && this.bitrateValue) {
            this.bitrateSlider.addEventListener('input', (e) => {
                this.bitrateValue.textContent = e.target.value;
            });
        }

        if (this.settingStats) {
            this.settingStats.addEventListener('change', (e) => {
                this.showStats = e.target.checked;
                this.updateStatsVisibility();
            });
        }

        // Monitor and codec selection
        if (this.monitorDropdown) {
            this.monitorDropdown.addEventListener('change', (e) => {
                const newMonitor = parseInt(e.target.value);
                if (newMonitor !== this.currentMonitor) {
                    this.switchMonitor(newMonitor);
                }
            });
        }

        // Codec dropdown is disabled - using H.264 only

        if (this.qualityDropdown) {
            this.qualityDropdown.addEventListener('change', (e) => {
                const selectedQuality = e.target.value;
                if (selectedQuality === 'auto') {
                    this.adaptiveQuality = true;
                    this.showNotification('Auto quality enabled', 2000);
                } else {
                    this.adaptiveQuality = false;
                    this.switchQuality(selectedQuality);
                }
            });
        }

        // Click outside settings to close
        document.addEventListener('click', (e) => {
            if (this.settingsPanel && this.settingsPanel.classList.contains('visible') && 
                !this.settingsPanel.contains(e.target) && 
                !document.getElementById('settings-btn')?.contains(e.target)) {
                this.hideSettings();
            }
        });

        // Keyboard shortcuts
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape') {
                if (this.settingsPanel && this.settingsPanel.classList.contains('visible')) {
                    this.hideSettings();
                } else if (document.fullscreenElement) {
                    document.exitFullscreen();
                }
            } else if (e.key === 'F11') {
                e.preventDefault();
                this.toggleFullscreen();
            } else if (e.key === 's' && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                this.toggleSettings();
            }
        });
    }

    handleMouseActivity() {
        this.lastMouseMove = Date.now();
        this.showOSD();
        
        // Clear existing timer
        if (this.mouseIdleTimer) {
            clearTimeout(this.mouseIdleTimer);
        }
        
        // Set timer to hide OSD after 3 seconds of inactivity
        this.mouseIdleTimer = setTimeout(() => {
            this.hideOSD();
        }, 3000);
    }

    showOSD() {
        if (this.osdOverlay) {
            this.osdVisible = true;
            this.osdOverlay.classList.add('visible');
        }
        const screenElement = document.getElementById('screen');
        if (screenElement) {
            screenElement.classList.add('show-cursor');
        }
    }

    hideOSD() {
        if (this.settingsPanel && this.settingsPanel.classList.contains('visible')) {
            return; // Don't hide OSD while settings are open
        }
        
        if (this.osdOverlay) {
            this.osdVisible = false;
            this.osdOverlay.classList.remove('visible');
        }
        const screenElement = document.getElementById('screen');
        if (screenElement) {
            screenElement.classList.remove('show-cursor');
        }
    }

    // Input handling methods
    setupInputHandlers() {
        const screenContainer = document.getElementById('screen');
        
        // Mouse events - attach to screen container as a fallback.
        // Note: initializeOptimizedCanvas() adds its own listeners to realCanvas.
        // We use stopPropagation in the handler to prevent double-firing.
        ['mousedown', 'mouseup', 'mousemove', 'wheel'].forEach(event => {
            if (screenContainer) {
                screenContainer.addEventListener(event, (e) => this.handleMouseEvent(e));
            }
        });
        
        // Touch events
        ['touchstart', 'touchmove', 'touchend', 'touchcancel'].forEach(event => {
            if (screenContainer) {
                screenContainer.addEventListener(event, (e) => this.handleTouchEvent(e), { passive: false });
            }
        });
        
        // Keyboard events
        document.addEventListener('keydown', (e) => this.handleKeyEvent(e, 'keydown'));
        document.addEventListener('keyup', (e) => this.handleKeyEvent(e, 'keyup'));
        
        // Prevent context menu on video screen and screen container
        if (this.videoScreen) {
            this.videoScreen.addEventListener('contextmenu', (e) => e.preventDefault());
        }
        if (screenContainer) {
            screenContainer.addEventListener('contextmenu', (e) => e.preventDefault());
        }
    }

    /**
     * Calculate the actual rendered content rectangle within an element
     * that uses object-fit: contain. The element's bounding rect includes
     * letterbox/pillarbox black bars, but we need only the content area.
     */
    getContentRect(element) {
        const rect = element.getBoundingClientRect();
        const elementAspect = rect.width / rect.height;
        const contentAspect = this.screenWidth / this.screenHeight;

        let contentWidth, contentHeight, contentLeft, contentTop;

        if (elementAspect > contentAspect) {
            // Element is wider than content — pillarboxing (black bars on sides)
            contentHeight = rect.height;
            contentWidth = rect.height * contentAspect;
            contentLeft = rect.left + (rect.width - contentWidth) / 2;
            contentTop = rect.top;
        } else {
            // Element is taller than content — letterboxing (black bars top/bottom)
            contentWidth = rect.width;
            contentHeight = rect.width / contentAspect;
            contentLeft = rect.left;
            contentTop = rect.top + (rect.height - contentHeight) / 2;
        }

        return { left: contentLeft, top: contentTop, width: contentWidth, height: contentHeight };
    }

    handleMouseEvent(e) {
        if (!this.connected) return;

        // ── Host control priority ──────────────────────────────────────
        // When the host is actively controlling the cursor, suppress
        // client mouse input to avoid conflicting cursor movements.
        if (this.hostCursor.isHostControlling) {
            // Allow scroll events through (they don't move the cursor)
            if (e.type !== 'wheel') {
                return;
            }
        }
        
        // Use the appropriate element - prefer realCanvas (dynamically created), then fallbackCanvas, then videoScreen
        let targetElement = null;
        if (this.realCanvas && this.realCanvas.parentElement) {
            targetElement = this.realCanvas;
        } else if (this.fallbackCanvas && this.fallbackCanvas.style.display !== 'none') {
            targetElement = this.fallbackCanvas;
        } else {
            targetElement = this.videoScreen;
        }
            
        if (!targetElement) return;
        
        // Ensure valid dimensions to prevent NaN/Infinity
        if (this.screenWidth <= 0 || this.screenHeight <= 0) {
            console.warn('Invalid screen dimensions for coordinate calculation');
            return;
        }
        
        // Get the actual content area (excluding object-fit: contain black bars)
        const content = this.getContentRect(targetElement);
        
        if (content.width <= 0 || content.height <= 0) {
            console.warn('Invalid content dimensions for coordinate calculation');
            return;
        }
        
        // Calculate position relative to the content area (not the element)
        const relX = e.clientX - content.left;
        const relY = e.clientY - content.top;
        
        // Ignore clicks in the letterbox/pillarbox black bars
        if (relX < 0 || relX >= content.width || relY < 0 || relY >= content.height) {
            return;
        }
        
        const x = Math.floor(Math.min(relX / content.width * this.screenWidth, this.screenWidth - 1));
        const y = Math.floor(Math.min(relY / content.height * this.screenHeight, this.screenHeight - 1));
        
        let eventData = {
            x,
            y,
            monitor_id: this.getActiveMonitorId()
        };
        
        switch(e.type) {
            case 'mousedown':
                eventData.type = 'mousedown';
                eventData.button = e.button; // 0=left, 1=middle, 2=right (numeric)
                this.sendInputEvent(eventData);
                break;
            case 'mouseup':
                eventData.type = 'mouseup';
                eventData.button = e.button; // 0=left, 1=middle, 2=right (numeric)
                this.sendInputEvent(eventData);
                break;
            case 'mousemove':
                eventData.type = 'mousemove';
                this.sendInputEvent(eventData);
                break;
            case 'wheel':
                e.preventDefault();
                eventData.type = 'wheel';
                eventData.delta_y = e.deltaY;
                eventData.delta_x = e.deltaX;
                this.sendInputEvent(eventData);
                break;
        }
    }

    handleKeyEvent(e, type) {
        if (!this.connected) return;
        
        // Don't capture certain keys if settings panel is open
        if (this.settingsPanel && this.settingsPanel.classList.contains('visible')) {
            return;
        }
        
        // Let some special keys pass through
        if (['F11', 'F12'].includes(e.key) || 
            (e.key === 's' && (e.ctrlKey || e.metaKey))) {
            return;
        }
        
        e.preventDefault();
        
        this.sendInputEvent({
            type: type,
            key: e.key,
            code: e.code,   // e.g. "KeyA", "Digit1", "ArrowUp" — required by server
            keyCode: e.keyCode,
            ctrlKey: e.ctrlKey,
            altKey: e.altKey,
            shiftKey: e.shiftKey,
            metaKey: e.metaKey,
            monitor_id: this.getActiveMonitorId()
        });
    }

    handleTouchEvent(e) {
        e.preventDefault();
        
        if (!this.connected) return;
        
        // Use the appropriate target element - prefer realCanvas
        let targetElement = null;
        if (this.realCanvas && this.realCanvas.parentElement) {
            targetElement = this.realCanvas;
        } else if (this.fallbackCanvas && this.fallbackCanvas.style.display !== 'none') {
            targetElement = this.fallbackCanvas;
        } else {
            targetElement = this.videoScreen;
        }
        
        if (!targetElement) return;
        
        // Handle touch events for mobile devices
        for (let i = 0; i < e.changedTouches.length; i++) {
            const touch = e.changedTouches[i];
            
            // Ensure valid screen dimensions
            if (this.screenWidth <= 0 || this.screenHeight <= 0) {
                continue;
            }
            
            // Get the actual content area (excluding object-fit: contain black bars)
            const content = this.getContentRect(targetElement);
            
            if (content.width <= 0 || content.height <= 0) {
                continue;
            }
            
            // Calculate position relative to the content area
            const relX = touch.clientX - content.left;
            const relY = touch.clientY - content.top;
            
            // Ignore touches in the letterbox/pillarbox black bars
            if (relX < 0 || relX >= content.width || relY < 0 || relY >= content.height) {
                continue;
            }
            
            const x = Math.floor(Math.min(relX / content.width * this.screenWidth, this.screenWidth - 1));
            const y = Math.floor(Math.min(relY / content.height * this.screenHeight, this.screenHeight - 1));
            
            let eventData = {
                type: e.type,
                x,
                y,
                identifier: touch.identifier,
                monitor_id: this.getActiveMonitorId()
            };
            
            this.sendInputEvent(eventData);
        }
    }

    sendInputEvent(event) {
        if (this.connected && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(event));
        }
    }

    sendMessage(message) {
        if (this.connected && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(message));
        }
    }

    getActiveMonitorId() {
        return this.availableMonitors[this.currentMonitor]?.id || 'primary';
    }

    // Utility methods
    sendPing() {
        if (this.connected && this.ws.readyState === WebSocket.OPEN) {
            this.lastPingTime = Date.now();
            this.ws.send(JSON.stringify({
                type: 'ping',
                timestamp: this.lastPingTime
            }));
        }
    }

    handlePingResponse() {
        if (this.lastPingTime > 0) {
            this.latency = Date.now() - this.lastPingTime;
            const latencyElement = document.getElementById('latency');
            if (latencyElement) {
                latencyElement.textContent = this.latency;
            }
        }
    }

    sendNetworkStats() {
        const stats = {
            latency: this.latency,
            bandwidth: this.estimateBandwidth(),
            packet_loss: this.estimatePacketLoss()
        };
        
        if (this.connected && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify({
                type: 'network_stats',
                stats: stats
            }));
        }
    }

    estimateBandwidth() {
        // Simple bandwidth estimation based on frame rate and quality
        const bytesPerFrame = (this.screenWidth * this.screenHeight * this.qualityLevel) / 1000;
        const fps = this.frameCount;
        return (bytesPerFrame * fps * 8) / 1024; // kbps
    }

    estimatePacketLoss() {
        // Simplified packet loss estimation
        return Math.max(0, (this.latency - 50) / 500);
    }

    showNotification(message, duration = 3000) {
        if (!this.notificationArea) return;
        
        const notification = document.createElement('div');
        notification.className = 'notification';
        notification.textContent = message;
        
        this.notificationArea.appendChild(notification);
        
        setTimeout(() => {
            notification.classList.add('show');
        }, 10);
        
        setTimeout(() => {
            notification.classList.remove('show');
            setTimeout(() => {
                if (notification.parentNode) {
                    notification.parentNode.removeChild(notification);
                }
            }, 300);
        }, duration);
    }

    toggleSettings() {
        if (this.settingsPanel) {
            if (this.settingsPanel.classList.contains('visible')) {
                this.hideSettings();
            } else {
                this.showSettings();
            }
        }
    }

    showSettings() {
        if (this.settingsPanel) {
            this.settingsPanel.classList.add('visible');
        }
    }

    hideSettings() {
        if (this.settingsPanel) {
            this.settingsPanel.classList.remove('visible');
        }
    }

    updateStatsVisibility() {
        if (this.networkStats) {
            this.networkStats.style.display = this.showStats ? 'block' : 'none';
        }
    }

    updateFrameStats() {
        this.frameCount++;
        const now = Date.now();
        
        if (now - this.lastFpsUpdate >= 1000) {
            const fps = this.frameCount;
            
            // Store current FPS for canvas display
            if (!this.frameStats) this.frameStats = {};
            this.frameStats.currentFps = fps;
            
            this.frameCount = 0;
            this.lastFpsUpdate = now;
            
            const fpsElement = document.getElementById('fps');
            if (fpsElement) {
                fpsElement.textContent = fps;
            }
        }
    }

    saveSettings() {
        // Get values from settings panel
        if (this.settingStretch) {
            this.config.stretch = this.settingStretch.checked;
        }
        if (this.settingAudio) {
            this.config.audio = this.settingAudio.checked;
        }
        if (this.settingMute) {
            this.config.mute = this.settingMute.checked;
            if (this.audioElement) {
                this.audioElement.muted = this.config.mute;
            }
        }
        
        // Apply stretch setting to video
        if (this.videoScreen) {
            if (this.config.stretch) {
                this.videoScreen.style.width = '100%';
                this.videoScreen.style.height = '100%';
                this.videoScreen.style.objectFit = 'fill';
            } else {
                this.videoScreen.style.width = 'auto';
                this.videoScreen.style.height = 'auto';
                this.videoScreen.style.objectFit = 'contain';
            }
        }
        
        this.hideSettings();
        this.showNotification('Settings saved');
    }

    toggleFullscreen() {
        if (!document.fullscreenElement) {
            document.documentElement.requestFullscreen().catch(err => {
                console.error(`Error attempting to enable fullscreen: ${err.message}`);
                this.showNotification(`Fullscreen error: ${err.message}`);
            });
        } else {
            document.exitFullscreen();
        }
    }

    disconnect() {
        if (this.connected) {
            this.ws.close();
        }
        
        // Stop all monitoring intervals
        this.stopNetworkMonitoring();
        this.stopConnectionHealthMonitoring();
        
        if (this.pingInterval) {
            clearInterval(this.pingInterval);
            this.pingInterval = null;
        }
        
        window.location.href = '/';
    }

    // Monitor and codec switching
    switchMonitor(monitorIndex) {
        console.log('Switching to monitor:', monitorIndex);
        this.currentMonitor = monitorIndex;
        // Reconnect with new monitor
        if (this.ws) {
            this.ws.close();
        }
    }

    sendQualitySetting(quality) {
        if (this.connected && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify({
                type: 'quality_update',
                quality: parseInt(quality)
            }));
        }
    }

    // WebRTC setup for audio
    setupWebRTC(encryption) {
        if (!this.config.audio) return;
        
        console.log('Setting up WebRTC for audio streaming');
        
        // This would be implemented for actual WebRTC audio support
        // For now, just log that it's being set up
        if (encryption) {
            console.log('WebRTC will use encryption');
        }
    }

    handleWebRTCOffer(data) {
        console.log('Received WebRTC offer:', data);
        
        // In a real implementation, this would:
        // 1. Create RTCPeerConnection
        // 2. Set remote description with the offer
        // 3. Create and send answer back to server
        
        // For now, just acknowledge
        if (this.connected && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify({
                type: 'webrtc_answer',
                sdp: 'mock_answer_sdp'
            }));
        }
    }

    handleQualityUpdate(data) {
        console.log('Quality update received:', data);
        this.qualityLevel = data.quality || data.value || 85;
        
        // Update UI elements
        const qualityElements = document.querySelectorAll('#quality');
        qualityElements.forEach(el => el.textContent = this.qualityLevel);
    }

    handleMonitorList(data) {
        console.log('Monitor list received:', data);
        this.availableMonitors = data.monitors || [];
        
        // Update monitor dropdown
        if (this.monitorDropdown) {
            this.monitorDropdown.innerHTML = '';
            
            if (this.availableMonitors.length > 0) {
                this.availableMonitors.forEach((monitor, index) => {
                    const option = document.createElement('option');
                    option.value = index;
                    option.textContent = `${monitor.name} ${monitor.is_primary ? '(Primary)' : ''} - ${monitor.width}x${monitor.height}`;
                    this.monitorDropdown.appendChild(option);
                });
            } else {
                // Fallback if no monitors are detected
                const option = document.createElement('option');
                option.value = 0;
                option.textContent = 'Primary Monitor';
                this.monitorDropdown.appendChild(option);
            }
            
            // Set current selection
            this.monitorDropdown.value = this.currentMonitor;
        }
        
        // If this is the first monitor list and status is still showing, hide it
        if (this.statusDisplay && this.statusDisplay.style.display !== 'none') {
            setTimeout(() => {
                if (this.statusDisplay) {
                    this.statusDisplay.style.display = 'none';
                }
            }, 500);
        }
    }

    // Connect method
    connect() {
        this.updateStatus('Connecting', 'Establishing connection to server...', true);
        
        // Send a test HTTP request to verify connectivity
        fetch('/static/kvm-client.css')
            .then(response => console.log('Test connectivity check successful:', response.status))
            .catch(error => console.error('Test connectivity check failed:', error));
        
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        
        // Get the hostname from the current URL - this should preserve IP addresses and hostnames
        let hostname = window.location.hostname;
        
        // Debug logging
        console.log('Current location:', window.location.href);
        console.log('Hostname extracted:', hostname);
        console.log('Port from location:', window.location.port);
        
        // Check for manual server override in URL parameters
        const urlParams = new URLSearchParams(window.location.search);
        const serverOverride = urlParams.get('server');
        
        // Determine the WebSocket host
        let wsHost;
        if (serverOverride) {
            // Manual server override via URL parameter: ?server=192.168.1.100:9921
            wsHost = serverOverride;
            console.log('Using server override from URL:', wsHost);
        } else if (window.location.port && window.location.port !== '80' && window.location.port !== '443') {
            // If we're on a custom port (like the Vite dev server), use the hostname with port 9921
            wsHost = `${hostname}:9921`;
        } else {
            // If we're on standard HTTP/HTTPS ports, assume KVM is also on the same host with port 9921
            wsHost = `${hostname}:9921`;
        }
        
        const wsUrl = `${protocol}//${wsHost}/ws?monitor=${this.currentMonitor}&codec=${this.currentCodec}${this.config.audio ? '&audio=true' : ''}`;
        
        console.log('Connecting to WebSocket:', wsUrl);
        console.log('WebSocket host resolved to:', wsHost);
        
        this.ws = new WebSocket(wsUrl);
        this.ws.binaryType = 'arraybuffer'; // Receive binary data as ArrayBuffer (avoid Blob async conversion)
        
        this.ws.onopen = () => {
            this.connected = true;
            this.reconnectAttempts = 0;
            this.lastFrameTime = Date.now(); // Reset frame timer on each new connection
            this.updateStatus('Connected', 'Connection established successfully');
            console.log('WebSocket connection established');
            
            // H.264 streaming uses WebCodecs decoder - no MediaSource needed
            console.log('🎬 Using H.264 hardware-accelerated streaming');
            
            // Start sending ping messages to measure latency
            this.pingInterval = setInterval(() => {
                this.sendPing();
            }, 2000);

            // Start network monitoring and adaptive quality
            this.startNetworkMonitoring();
            
            // Start connection health monitoring to detect freezes
            this.startConnectionHealthMonitoring();
            
            // Request monitor list if not received within 2 seconds
            setTimeout(() => {
                if (this.availableMonitors.length === 0) {
                    console.log('No monitors received, using fallback...');
                    // Create a fallback monitor entry
                    this.availableMonitors = [{
                        id: "primary",
                        name: "Primary Monitor", 
                        width: this.screenWidth || 1920,
                        height: this.screenHeight || 1080,
                        is_primary: true
                    }];
                    this.handleMonitorList({ monitors: this.availableMonitors });
                }
            }, 2000);
        };
        
        this.ws.onmessage = async (event) => {
            try {
                // Check if the message is binary data (video frame) or text data (control message)
                if (event.data instanceof ArrayBuffer) {
                    // ArrayBuffer - handle as video frame directly
                    this.handleBinaryVideoFrame(event.data);
                } else if (event.data instanceof Blob) {
                    // Blob - convert to ArrayBuffer first
                    const arrayBuffer = await event.data.arrayBuffer();
                    this.handleBinaryVideoFrame(arrayBuffer);
                } else {
                    // Text data - handle as JSON control message
                    const data = JSON.parse(event.data);
                    this.handleMessage(data);
                }
            } catch (e) {
                console.error('Error handling WebSocket message:', e);
            }
        };
        
        this.ws.onclose = (event) => {
            this.connected = false;
            
            // Stop all monitoring intervals
            if (this.pingInterval) {
                clearInterval(this.pingInterval);
                this.pingInterval = null;
            }
            this.stopNetworkMonitoring();
            this.stopConnectionHealthMonitoring();
            
            console.log('WebSocket closed. Code:', event.code, 'Reason:', event.reason);
            
            if (event.code === 1006) {
                this.updateStatus('Connection Failed', `Could not connect to KVM server at ${wsHost}. Please check that the server is running and accessible.`);
            } else {
                this.updateStatus('Disconnected', 'Connection closed');
            }
            
            // Attempt to reconnect with backoff
            if (this.reconnectAttempts < this.maxReconnectAttempts) {
                const delay = Math.min(3000 * (this.reconnectAttempts + 1), 15000);
                setTimeout(() => {
                    if (!this.connected) {
                        console.log(`Attempting to reconnect... (attempt ${this.reconnectAttempts + 1}/${this.maxReconnectAttempts})`);
                        this.connect();
                    }
                }, delay);
            }
        };
        
        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
            console.error('Failed to connect to:', wsUrl);
            this.updateStatus('Connection Error', `Failed to connect to KVM server. Check that port 9921 is accessible on ${hostname}.`);
        };
    }

    handleMessage(data) {
        switch(data.type) {
            case 'server_info':
            case 'info':  // Fallback for older message type
                this.handleServerInfo(data);
                break;
            case 'stream_info':
                this.handleStreamInfo(data);
                break;
            case 'video_frame':
                this.handleVideoFrame(data);
                break;
            case 'pong':
                this.handlePingResponse();
                break;
            case 'quality_update':
                this.handleQualityUpdate(data);
                break;
            case 'monitors':
                this.handleMonitorList(data);
                break;
            case 'webrtc_offer':
                this.handleWebRTCOffer(data);
                break;
            case 'streaming_stats':
                this.handleStreamingStats(data);
                break;
            case 'webrtc_frame':
                this.handleWebRTCFrame(data);
                break;
            case 'ping':
                // Server sends JSON ping — respond with pong
                if (this.ws && this.ws.readyState === WebSocket.OPEN) {
                    this.ws.send(JSON.stringify({ type: 'pong', timestamp: data.timestamp }));
                }
                break;
            default:
                console.log('Unknown message type:', data.type);
        }
    }

    handleServerInfo(data) {
        console.log('Server info received:', data);
        
        // Update screen dimensions with proper fallbacks
        if (data.width && data.height) {
            this.screenWidth = data.width;
            this.screenHeight = data.height;
        } else {
            console.warn('Server info missing dimensions, using defaults:', this.screenWidth, this.screenHeight);
        }
        
        // Detect rdengine protocol version
        if (data.protocol_version) {
            this.protocolVersion = data.protocol_version;
            console.log('Protocol version:', this.protocolVersion);
        }
        
        // Detect server codec
        const serverCodec = (data.codec || 'h264').toLowerCase();
        this.serverCodec = serverCodec;
        this.currentCodec = serverCodec;
        console.log('Server codec:', serverCodec);
        
        // Update canvas size if fallback is active
        if (this.fallbackCanvas) {
            this.fallbackCanvas.width = this.screenWidth;
            this.fallbackCanvas.height = this.screenHeight;
        }
        
        // Update UI
        if (this.osdTitle) {
            const hostname = data.hostname || 'KVM Server';
            const monitor = data.monitor || 0;
            const codecLabel = serverCodec.toUpperCase();
            const fps = data.framerate || 30;
            const bitrate = data.bitrate_kbps ? `${data.bitrate_kbps}kbps` : '';
            this.osdTitle.textContent = `${hostname} - Monitor ${monitor} (${this.screenWidth}x${this.screenHeight} ${codecLabel} ${fps}fps ${bitrate})`.trim();
        }
        
        // Update codec dropdown if present
        if (this.codecDropdown) {
            this.codecDropdown.value = serverCodec;
        }
        
        // Initialize canvas size
        if (this.canvasLayer) {
            this.canvasLayer.width = this.screenWidth;
            this.canvasLayer.height = this.screenHeight;
        }
        
        // Pre-initialize the optimized canvas with server dimensions
        this.initializeOptimizedCanvas(this.screenWidth, this.screenHeight);
        
        // Initialize correct decoder based on codec
        if (serverCodec === 'vp8' || serverCodec === 'vp9') {
            // rdengine VP8/VP9 path
            if (this.vpxDecoder) {
                this.vpxDecoder.setCodec(serverCodec);
                this.vpxDecoder.setDimensions(this.screenWidth, this.screenHeight);
                console.log(`VPX decoder configured for ${serverCodec} ${this.screenWidth}x${this.screenHeight}`);
            } else {
                console.warn('VPX decoder not available, initializing...');
                this.initializeVpxDecoder();
            }
        } else {
            // Legacy H.264 path
            this.initializeVideoStreaming();
        }
        
        // Audio: rdengine sends Opus directly over WebSocket, no WebRTC needed
        if (data.audio_enabled) {
            console.log('Server audio enabled (Opus over WebSocket)');
        } else if (this.config.audio && data.audio) {
            // Legacy WebRTC audio path
            this.setupWebRTC(data.encryption);
        }
        
        // Hide loading status after successful connection
        setTimeout(() => {
            if (this.statusDisplay) {
                this.statusDisplay.style.display = 'none';
            }
        }, 1000);
        
        const codecDisplay = serverCodec.toUpperCase();
        this.showNotification(`Connected to ${data.hostname} - ${data.width}x${data.height} using ${codecDisplay}`);
    }

    handleStreamInfo(data) {
        console.log('Stream info received:', data);
        
        // Extract video configuration
        const videoConfig = data.video_config;
        const serverInfo = data.server_info;
        
        this.screenWidth = videoConfig.width;
        this.screenHeight = videoConfig.height;
        
        // Update canvas size if fallback is active
        if (this.fallbackCanvas) {
            this.fallbackCanvas.width = this.screenWidth;
            this.fallbackCanvas.height = this.screenHeight;
            console.log(`Updated canvas size to: ${this.screenWidth}x${this.screenHeight}`);
        }
        
        // Update UI with server information
        if (this.osdTitle) {
            this.osdTitle.textContent = `${serverInfo.hostname} - Monitor ${serverInfo.current_monitor} (${videoConfig.width}x${videoConfig.height})`;
        }
        
        // Keep the codec that was initialized - always H.264
        console.log('Using codec:', this.currentCodec);
        if (this.codecDropdown) {
            this.codecDropdown.value = 'h264';
        }
        
        // Initialize canvas size
        if (this.canvasLayer) {
            this.canvasLayer.width = this.screenWidth;
            this.canvasLayer.height = this.screenHeight;
        }
        
        // Ensure video element is visible
        if (this.videoScreen) {
            this.videoScreen.style.display = 'block';
        }
        
        // Initialize video streaming
        this.initializeVideoStreaming();
        
        // Initialize WebRTC for audio if enabled (audio_config will be present)
        if (this.config.audio && data.audio_config) {
            // TODO: Handle audio configuration
            console.log('Audio config:', data.audio_config);
        }
        
        // Hide loading status after successful connection
        setTimeout(() => {
            if (this.statusDisplay) {
                this.statusDisplay.style.display = 'none';
            }
        }, 1000);
        
        this.showNotification(`Connected to ${serverInfo.hostname} - ${videoConfig.width}x${videoConfig.height} using ${videoConfig.codec}`);
    }

    initializeVideoStreaming() {
        if (!this.videoScreen) {
            console.error('Video screen element not found');
            return;
        }
        
        console.log('🎬 Initializing H.264 video streaming');
        
        // H.264 uses canvas-based rendering with WebCodecs decoder
        // Set video element dimensions for fallback
        this.videoScreen.width = this.screenWidth;
        this.videoScreen.height = this.screenHeight;
        
        // Apply stretch setting
        if (this.config.stretch) {
            this.videoScreen.style.width = '100%';
            this.videoScreen.style.height = '100%';
            this.videoScreen.style.objectFit = 'fill';
        } else {
            this.videoScreen.style.width = 'auto';
            this.videoScreen.style.height = 'auto';
            this.videoScreen.style.objectFit = 'contain';
        }
        
        // Initialize optimized canvas for H.264 frame rendering
        this.initializeOptimizedCanvas(this.screenWidth, this.screenHeight);
    }

    // H.264 streaming uses WebCodecs VideoDecoder - no MediaSource needed
    // The h264-decoder.js handles all H.264 decoding with hardware acceleration

    processVideoQueue() {
        // Process queued H.264 frames if any
        if (this.videoQueue.length === 0) {
            return;
        }
        
        // H.264 frames are processed directly by the decoder
        const frame = this.videoQueue.shift();
        if (frame && this.h264Decoder && this.h264Decoder.isReady) {
            this.h264Decoder.decode(frame.data, frame.metadata);
        }
    }

    handleBinaryVideoFrame(binaryData) {
        // Ultra-minimal logging for performance
        if (!this.frameLogCounter) this.frameLogCounter = 0;
        
        if (!binaryData || binaryData.byteLength === 0) return;
        
        // Log occasionally
        const shouldLog = this.frameLogCounter < 10 || this.frameLogCounter % 300 === 0;
        if (shouldLog) {
            console.log('Frame stream active:', (binaryData.byteLength / 1024).toFixed(1) + 'KB', 
                        'Frame #' + this.frameLogCounter);
        }
        this.frameLogCounter++;
        
        // Update last frame time for connection health monitoring
        this.lastFrameTime = Date.now();

        try {
            const view = new DataView(binaryData);
            const firstByte = view.getUint8(0);
            
            // Check for rdengine binary protocol (MSG_VIDEO_FRAME = 0x01)
            if (firstByte === 0x01) {
                this.handleRdEngineVideoFrame(binaryData);
                this.updateFrameStats();
                return;
            }
            
            // Check for rdengine audio frame (MSG_AUDIO_FRAME = 0x02)  
            if (firstByte === 0x02) {
                // Audio frames handled by WebCodecs AudioDecoder (future)
                return;
            }
            
            // Check for rdengine cursor update (MSG_CURSOR = 0x03)
            if (firstByte === 0x03) {
                this.handleCursorMessage(binaryData);
                return;
            }
            
            // Check for rdengine ping (MSG_PING = 0x05)
            if (firstByte === 0x05) {
                // Server should not send us pings as binary, but handle it
                return;
            }
            
            // Check for rdengine pong (MSG_PONG = 0x06)
            if (firstByte === 0x06) {
                this.handlePingResponse();
                return;
            }
            
            // Legacy format detection by 4-byte header
            const header = String.fromCharCode(
                view.getUint8(0), view.getUint8(1), 
                view.getUint8(2), view.getUint8(3)
            );
            
            if (header === 'H264') {
                // Legacy H.264 frame from old pipeline
                this.handleH264VideoFrame(binaryData);
            } else {
                // Fall back to RGBA/RLE frame parsing
                this.parseAndRenderFrame(binaryData);
            }
            
            this.updateFrameStats();
            
        } catch (e) {
            if (this.frameLogCounter % 100 === 0) {
                console.error('Frame processing error:', e.message);
            }
        }
    }
    
    // ── Host Cursor Synchronization ─────────────────────────────────────

    /**
     * Map cursor shape byte to CSS cursor name.
     * Must match CursorShape enum in cursor_service.rs.
     */
    static CURSOR_SHAPE_MAP = [
        'default',      // 0
        'pointer',      // 1
        'text',         // 2
        'wait',         // 3
        'crosshair',    // 4
        'move',         // 5
        'not-allowed',  // 6
        'help',         // 7
        'n-resize',     // 8
        's-resize',     // 9
        'e-resize',     // 10
        'w-resize',     // 11
        'ne-resize',    // 12
        'nw-resize',    // 13
        'se-resize',    // 14
        'sw-resize',    // 15
        'ew-resize',    // 16
        'ns-resize',    // 17
        'nesw-resize',  // 18
        'nwse-resize',  // 19
        'grab',         // 20
        'grabbing',     // 21
        'progress',     // 22
    ];

    /**
     * Handle a MSG_CURSOR (0x03) binary message from the server.
     * Format: [1B type=0x03] [4B payload_len] [4B x_le] [4B y_le] [1B shape] [1B visible]
     */
    handleCursorMessage(binaryData) {
        const view = new DataView(binaryData);
        if (binaryData.byteLength < 15) return; // 1 + 4 + 4 + 4 + 1 + 1 = 15

        // Skip type (1B) + payload_len (4B)
        const x = view.getInt32(5, true);
        const y = view.getInt32(9, true);
        const shapeId = view.getUint8(13);
        const visible = view.getUint8(14) !== 0;

        const shapeName = (shapeId === 255)
            ? 'none'
            : (KVMClient.CURSOR_SHAPE_MAP[shapeId] || 'default');

        // Check if the host cursor actually moved (not just first message)
        const moved = (x !== this.hostCursor.x || y !== this.hostCursor.y);

        this.hostCursor.x = x;
        this.hostCursor.y = y;
        this.hostCursor.shape = shapeName;
        this.hostCursor.visible = visible;
        this.hostCursor.lastUpdate = Date.now();

        // If the host cursor moved, enter host-control mode
        if (moved) {
            this.setHostControlling(true);
        }

        // Render the host cursor overlay
        this.renderHostCursor();
    }

    /**
     * Enter or leave host-control mode.
     * While the host is controlling, the client's native cursor is hidden
     * over the canvas and input events are suppressed so the two cursors
     * don't fight each other.
     */
    setHostControlling(active) {
        this.hostCursor.isHostControlling = active;

        // Clear any previous expiry timer
        if (this.hostControlTimer) {
            clearTimeout(this.hostControlTimer);
            this.hostControlTimer = null;
        }

        if (active) {
            // Hide the client's native cursor over the screen area
            this.setClientCursorStyle('none');

            // After a period of host inactivity, hand control back to the client
            this.hostControlTimer = setTimeout(() => {
                this.hostCursor.isHostControlling = false;
                // Restore normal client cursor
                this.setClientCursorStyle('default');
            }, this.hostControlTimeoutMs);
        } else {
            this.setClientCursorStyle('default');
        }
    }

    /**
     * Set the CSS cursor style on all interactive screen elements.
     */
    setClientCursorStyle(cursorStyle) {
        const targets = [
            document.getElementById('screen'),
            this.realCanvas,
            this.videoScreen,
        ];
        for (const el of targets) {
            if (el) el.style.cursor = cursorStyle;
        }
    }

    /**
     * Create (once) and update the host cursor overlay element.
     * The overlay is a small cursor icon absolutely positioned over the
     * remote screen content area, matching the host's reported coordinates.
     */
    renderHostCursor() {
        // Lazily create the overlay element
        if (!this.hostCursorOverlay) {
            this.hostCursorOverlay = document.createElement('div');
            this.hostCursorOverlay.id = 'host-cursor-overlay';
            this.hostCursorOverlay.innerHTML = `
                <svg width="20" height="20" viewBox="0 0 24 24" class="host-cursor-svg">
                    <path d="M5 3l14 8-6.5 1.5L11 19z" fill="rgba(0,0,0,0.85)" stroke="white" stroke-width="1.5"
                          stroke-linejoin="round"/>
                </svg>
                <span class="host-cursor-label">Host</span>
            `;
            const screenContainer = document.getElementById('screen');
            if (screenContainer) {
                screenContainer.appendChild(this.hostCursorOverlay);
            }
        }

        if (!this.hostCursor.visible) {
            this.hostCursorOverlay.style.display = 'none';
            return;
        }

        this.hostCursorOverlay.style.display = '';

        // Convert host screen coordinates → CSS pixel position on the rendered content area
        const targetElement = this.realCanvas || this.videoScreen;
        if (!targetElement || this.screenWidth <= 0 || this.screenHeight <= 0) return;

        const content = this.getContentRect(targetElement);
        if (content.width <= 0 || content.height <= 0) return;

        const cssX = content.left + (this.hostCursor.x / this.screenWidth) * content.width;
        const cssY = content.top + (this.hostCursor.y / this.screenHeight) * content.height;

        this.hostCursorOverlay.style.left = `${cssX}px`;
        this.hostCursorOverlay.style.top = `${cssY}px`;

        // Update cursor shape class for different visual indicators
        this.hostCursorOverlay.dataset.shape = this.hostCursor.shape;
    }

    /**
     * Handle rdengine binary protocol video frame
     * Format: [1B type=0x01] [4B payload_len] [1B codec] [1B flags] [4B width] [4B height] [8B timestamp] [data...]
     */
    handleRdEngineVideoFrame(binaryData) {
        const view = new DataView(binaryData);
        
        // Parse outer envelope
        // type (1B) + payload_len (4B) = 5 bytes header
        if (binaryData.byteLength < 5) return;
        
        const payloadLen = view.getUint32(1, true);
        if (binaryData.byteLength < 5 + payloadLen) return;
        
        // Parse inner header (18 bytes)
        let offset = 5;
        const codec = view.getUint8(offset); offset += 1;
        const flags = view.getUint8(offset); offset += 1;
        const width = view.getUint32(offset, true); offset += 4;
        const height = view.getUint32(offset, true); offset += 4;
        const timestampMs = Number(view.getBigUint64(offset, true)); offset += 8;
        
        const isKeyframe = (flags & 0x01) !== 0;
        const encodedData = new Uint8Array(binaryData, offset, payloadLen - 18);
        
        // Codec IDs: 0x01=VP8, 0x02=VP9, 0x03=H264
        const codecName = codec === 0x01 ? 'vp8' : codec === 0x02 ? 'vp9' : 'h264';
        
        // Update dimensions if changed
        if (this.screenWidth !== width || this.screenHeight !== height) {
            console.log(`Dimensions: ${width}x${height} (${codecName})`);
            this.screenWidth = width;
            this.screenHeight = height;
            
            if (this.vpxDecoder) {
                this.vpxDecoder.setDimensions(width, height);
                this.vpxDecoder.setCodec(codecName);
            }
            
            this.initializeOptimizedCanvas(width, height);
        }
        
        // Log keyframes and periodic stats
        if (isKeyframe && this.frameLogCounter < 20) {
            console.log(`Keyframe: ${codecName} ${width}x${height}, size=${encodedData.byteLength}`);
        }
        
        // Decode with VP8/VP9 decoder (preferred)
        if ((codecName === 'vp8' || codecName === 'vp9') && this.vpxDecoder && this.vpxDecoder.isReady) {
            this.vpxDecoder.decode(encodedData, {
                isKeyframe,
                timestamp: timestampMs * 1000, // Convert ms to us for WebCodecs
            });
            return;
        }
        
        // Fallback to H.264 decoder for H.264 codec
        if (codecName === 'h264' && this.h264Decoder && this.h264Decoder.isReady) {
            this.h264Decoder.decode(encodedData, {
                isKeyframe,
                timestamp: timestampMs * 1000,
            });
            return;
        }
        
        // No suitable decoder available
        if (this.frameLogCounter < 5) {
            console.warn(`No decoder for codec: ${codecName}`);
        }
    }
    
    /**
     * Handle H.264 video frame from low-latency pipeline
     * Frame format:
     * [4 bytes] Magic: "H264"
     * [4 bytes] Width (little-endian)
     * [4 bytes] Height (little-endian)
     * [8 bytes] Timestamp (little-endian, microseconds)
     * [4 bytes] Frame size (little-endian)
     * [1 byte]  Flags (bit 0: keyframe)
     * [N bytes] H.264 NAL units
     */
    handleH264VideoFrame(binaryData) {
        const view = new DataView(binaryData);
        let offset = 4; // Skip "H264" header
        
        // Parse frame header
        const width = view.getUint32(offset, true); offset += 4;
        const height = view.getUint32(offset, true); offset += 4;
        const timestamp = Number(view.getBigUint64(offset, true)); offset += 8;
        const frameSize = view.getUint32(offset, true); offset += 4;
        const flags = view.getUint8(offset); offset += 1;
        const isKeyframe = (flags & 0x01) !== 0;
        
        // Update dimensions if changed
        if (this.screenWidth !== width || this.screenHeight !== height) {
            console.log(`📐 H.264 dimensions: ${width}x${height}`);
            this.screenWidth = width;
            this.screenHeight = height;
            
            if (this.h264Decoder) {
                this.h264Decoder.setDimensions(width, height);
            }
            
            this.initializeOptimizedCanvas(width, height);
        }
        
        // Extract H.264 data
        const h264Data = new Uint8Array(binaryData, offset, frameSize);
        
        // Log keyframes
        if (isKeyframe && this.frameLogCounter < 20) {
            console.log(`🔑 H.264 keyframe: ${width}x${height}, size=${frameSize}`);
        }
        
        // Decode with H.264 decoder if available
        if (this.h264Decoder && this.h264Decoder.isReady) {
            this.h264Decoder.decode(h264Data, {
                isKeyframe,
                timestamp,
                width,
                height
            });
        } else {
            // Fallback: try to render simplified H.264 data directly
            this.renderH264Fallback(h264Data, width, height, isKeyframe);
        }
    }
    
    /**
     * Fallback H.264 rendering when WebCodecs is not available
     * Handles high-quality subsampled YUV420 data with bilinear upscaling
     */
    renderH264Fallback(h264Data, width, height, isKeyframe) {
        // Initialize canvas if needed
        if (!this.realCanvas || !this.realCtx) {
            this.initializeOptimizedCanvas(width, height);
        }
        
        // Skip NAL headers to find YUV data
        let offset = 0;
        
        // Look for slice NAL unit (start code + NAL type 5 for IDR or 1 for slice)
        while (offset < h264Data.length - 4) {
            if (h264Data[offset] === 0 && h264Data[offset + 1] === 0 && 
                h264Data[offset + 2] === 0 && h264Data[offset + 3] === 1) {
                const nalType = h264Data[offset + 4] & 0x1F;
                if (nalType === 5 || nalType === 1) { // IDR or Slice
                    offset += 9; // Skip NAL header and slice header
                    break;
                }
            }
            offset++;
        }
        
        // Read subsampled dimensions from header
        if (offset + 8 > h264Data.length) {
            console.warn('Not enough data for YUV header');
            return;
        }
        
        const yOutWidth = h264Data[offset] | (h264Data[offset + 1] << 8);
        const yOutHeight = h264Data[offset + 2] | (h264Data[offset + 3] << 8);
        const uvOutWidth = h264Data[offset + 4] | (h264Data[offset + 5] << 8);
        const uvOutHeight = h264Data[offset + 6] | (h264Data[offset + 7] << 8);
        offset += 8;
        
        const yDataSize = yOutWidth * yOutHeight;
        const uvDataSize = uvOutWidth * uvOutHeight;
        
        if (offset + yDataSize + uvDataSize * 2 > h264Data.length) {
            console.warn('Not enough YUV data:', offset + yDataSize + uvDataSize * 2, '>', h264Data.length);
            // Try legacy format
            this.renderH264FallbackLegacy(h264Data, width, height);
            return;
        }
        
        // Extract YUV planes
        const yPlane = h264Data.subarray(offset, offset + yDataSize);
        offset += yDataSize;
        const uPlane = h264Data.subarray(offset, offset + uvDataSize);
        offset += uvDataSize;
        const vPlane = h264Data.subarray(offset, offset + uvDataSize);
        
        // Create image data with bilinear upscaling
        const imageData = this.realCtx.createImageData(width, height);
        const pixels = imageData.data;
        
        // Calculate scaling factors
        const yScaleX = yOutWidth / width;
        const yScaleY = yOutHeight / height;
        const uvScaleX = uvOutWidth / width;
        const uvScaleY = uvOutHeight / height;
        
        // Bilinear interpolation for high-quality upscaling
        for (let py = 0; py < height; py++) {
            for (let px = 0; px < width; px++) {
                // Y plane interpolation
                const ySrcX = px * yScaleX;
                const ySrcY = py * yScaleY;
                const y = this.bilinearSample(yPlane, yOutWidth, yOutHeight, ySrcX, ySrcY);
                
                // UV plane interpolation
                const uvSrcX = px * uvScaleX;
                const uvSrcY = py * uvScaleY;
                const u = this.bilinearSample(uPlane, uvOutWidth, uvOutHeight, uvSrcX, uvSrcY);
                const v = this.bilinearSample(vPlane, uvOutWidth, uvOutHeight, uvSrcX, uvSrcY);
                
                // Convert YUV to RGB (BT.601 full range)
                // Y is already in 0-255 range, U/V centered at 128
                const yVal = y;
                const uVal = u - 128;
                const vVal = v - 128;
                
                // Full range BT.601 conversion (no clamping needed for Y)
                const r = Math.max(0, Math.min(255, Math.round(yVal + 1.402 * vVal)));
                const g = Math.max(0, Math.min(255, Math.round(yVal - 0.344 * uVal - 0.714 * vVal)));
                const b = Math.max(0, Math.min(255, Math.round(yVal + 1.772 * uVal)));
                
                const pixelIndex = (py * width + px) * 4;
                pixels[pixelIndex] = r;
                pixels[pixelIndex + 1] = g;
                pixels[pixelIndex + 2] = b;
                pixels[pixelIndex + 3] = 255;
            }
        }
        
        this.realCtx.putImageData(imageData, 0, 0);
    }
    
    /**
     * Bilinear sampling for smooth upscaling
     */
    bilinearSample(plane, planeWidth, planeHeight, x, y) {
        const x0 = Math.floor(x);
        const y0 = Math.floor(y);
        const x1 = Math.min(x0 + 1, planeWidth - 1);
        const y1 = Math.min(y0 + 1, planeHeight - 1);
        
        const fx = x - x0;
        const fy = y - y0;
        
        const p00 = plane[y0 * planeWidth + x0] || 128;
        const p10 = plane[y0 * planeWidth + x1] || 128;
        const p01 = plane[y1 * planeWidth + x0] || 128;
        const p11 = plane[y1 * planeWidth + x1] || 128;
        
        // Bilinear interpolation
        const top = p00 * (1 - fx) + p10 * fx;
        const bottom = p01 * (1 - fx) + p11 * fx;
        return top * (1 - fy) + bottom * fy;
    }
    
    /**
     * Legacy fallback for old macroblock format
     */
    renderH264FallbackLegacy(h264Data, width, height) {
        const mbWidth = Math.ceil(width / 16);
        const mbHeight = Math.ceil(height / 16);
        
        let offset = 0;
        while (offset < h264Data.length - 4) {
            if (h264Data[offset] === 0 && h264Data[offset + 1] === 0 && 
                h264Data[offset + 2] === 0 && h264Data[offset + 3] === 1) {
                const nalType = h264Data[offset + 4] & 0x1F;
                if (nalType === 5 || nalType === 1) {
                    offset += 9;
                    break;
                }
            }
            offset++;
        }
        
        const mbDataSize = mbWidth * mbHeight * 3;
        if (offset + mbDataSize > h264Data.length) {
            this.renderH264FallbackGrayscale(h264Data, width, height, offset, mbWidth, mbHeight);
            return;
        }
        
        const imageData = this.realCtx.createImageData(width, height);
        const pixels = imageData.data;
        
        for (let mbY = 0; mbY < mbHeight; mbY++) {
            for (let mbX = 0; mbX < mbWidth; mbX++) {
                const mbIndex = (mbY * mbWidth + mbX) * 3;
                const y = h264Data[offset + mbIndex] || 128;
                const u = h264Data[offset + mbIndex + 1] || 128;
                const v = h264Data[offset + mbIndex + 2] || 128;
                
                // BT.601 full range conversion
                const yVal = y;
                const uVal = u - 128;
                const vVal = v - 128;
                
                const r = Math.max(0, Math.min(255, Math.round(yVal + 1.402 * vVal)));
                const g = Math.max(0, Math.min(255, Math.round(yVal - 0.344 * uVal - 0.714 * vVal)));
                const b = Math.max(0, Math.min(255, Math.round(yVal + 1.772 * uVal)));
                
                for (let dy = 0; dy < 16; dy++) {
                    const py = mbY * 16 + dy;
                    if (py >= height) continue;
                    
                    for (let dx = 0; dx < 16; dx++) {
                        const px = mbX * 16 + dx;
                        if (px >= width) continue;
                        
                        const pixelIndex = (py * width + px) * 4;
                        pixels[pixelIndex] = r;
                        pixels[pixelIndex + 1] = g;
                        pixels[pixelIndex + 2] = b;
                        pixels[pixelIndex + 3] = 255;
                    }
                }
            }
        }
        
        this.realCtx.putImageData(imageData, 0, 0);
    }
    
    /**
     * Grayscale fallback for legacy Y-only format
     */
    renderH264FallbackGrayscale(h264Data, width, height, offset, mbWidth, mbHeight) {
        const imageData = this.realCtx.createImageData(width, height);
        const pixels = imageData.data;
        
        for (let mbY = 0; mbY < mbHeight; mbY++) {
            for (let mbX = 0; mbX < mbWidth; mbX++) {
                const mbIndex = mbY * mbWidth + mbX;
                const yValue = h264Data[offset + mbIndex] || 128;
                
                for (let dy = 0; dy < 16; dy++) {
                    const py = mbY * 16 + dy;
                    if (py >= height) continue;
                    
                    for (let dx = 0; dx < 16; dx++) {
                        const px = mbX * 16 + dx;
                        if (px >= width) continue;
                        
                        const pixelIndex = (py * width + px) * 4;
                        pixels[pixelIndex] = yValue;
                        pixels[pixelIndex + 1] = yValue;
                        pixels[pixelIndex + 2] = yValue;
                        pixels[pixelIndex + 3] = 255;
                    }
                }
            }
        }
        
        this.realCtx.putImageData(imageData, 0, 0);
    }

    // H.264 is the only supported codec - no WebM/VP8 fallback needed

    parseAndRenderFrame(arrayBuffer) {
        const now = performance.now();
        
        // Aggressive frame dropping for ultra-low latency
        if (this.frameQueue.length >= this.maxQueueSize) {
            this.perfStats.droppedFrames++;
            return; // Drop frame to maintain low latency
        }
        
        const dataView = new DataView(arrayBuffer);
        let offset = 0;
        
        // Check for ultra-fast RGBA format from backend (starts with "RGBA")
        if (dataView.byteLength < 24) return;
        
        const rgbaSignature = dataView.getUint32(0, false) === 0x52474241; // "RGBA" in big-endian
        
        if (rgbaSignature) {
            // New ultra-fast RGBA format from optimized backend - zero conversion overhead!
            offset = 4; // Skip "RGBA" signature
            
            const width = dataView.getUint32(offset, true); offset += 4;
            const height = dataView.getUint32(offset, true); offset += 4;
            const frameNumber = dataView.getBigUint64(offset, true); offset += 8;
            const dataLength = dataView.getUint32(offset, true); offset += 4;
            
            // Update screen dimensions from frame data if not set
            if (!this.screenWidth || !this.screenHeight || 
                this.screenWidth !== width || this.screenHeight !== height) {
                this.screenWidth = width;
                this.screenHeight = height;
                console.log(`📐 Screen dimensions updated from frame: ${width}x${height}`);
                
                // Update canvas sizes
                if (this.realCanvas) {
                    this.realCanvas.width = width;
                    this.realCanvas.height = height;
                }
                if (this.fallbackCanvas) {
                    this.fallbackCanvas.width = width;
                    this.fallbackCanvas.height = height;
                }
            }
            
            // Only log occasionally to reduce console spam
            if (frameNumber % 60 === 0n || frameNumber < 5n) {
                console.log(`📺 RGBA frame: ${width}x${height}, frame #${frameNumber}, data: ${dataLength} bytes`);
            }
            
            if (dataView.byteLength < offset + dataLength) {
                console.error(`❌ RGBA frame truncated: need ${offset + dataLength} bytes, got ${dataView.byteLength} bytes`);
                return;
            }
            
            // Direct RGBA data - MUST copy the data since ArrayBuffer may be reused
            const rgbaData = new Uint8Array(dataLength);
            rgbaData.set(new Uint8Array(arrayBuffer, offset, dataLength));
            
            // Log first frame for debugging
            if (frameNumber < 3n) {
                console.log(`🎨 Frame ${frameNumber} RGBA data: first bytes = [${rgbaData[0]}, ${rgbaData[1]}, ${rgbaData[2]}, ${rgbaData[3]}]`);
            }
            
            this.frameQueue.push({
                rgbaData,
                width,
                height,
                isKeyframe: true,
                frameNumber,
                timestamp: now,
                format: 'rgba_direct' // Ultra-fast format
            });
        } else {
            // Legacy RLE format fallback
            const header = dataView.getUint32(offset, false);
            offset += 3;
            
            if ((header >>> 8) !== 0xAABB01 && (header >>> 8) !== 0xAABB02) {
                console.error('Invalid frame header');
                return;
            }
            
            const isKeyframe = (header & 0xFF) === 0x01;
            const width = dataView.getUint32(offset, true); offset += 4;
            const height = dataView.getUint32(offset, true); offset += 4;
            const frameNumber = dataView.getBigUint64(offset, true); offset += 8;
            const compressedLength = dataView.getUint32(offset, true); offset += 4;
            
            if (dataView.byteLength < offset + compressedLength) {
                console.error('Frame truncated');
                return;
            }
            
            const compressedData = new Uint8Array(arrayBuffer, offset, compressedLength);
            
            this.frameQueue.push({
                compressedData,
                width,
                height,
                isKeyframe,
                frameNumber,
                timestamp: now,
                format: 'rle' // Legacy format
            });
        }
        
        // Process frames asynchronously
        this.processFrameQueue();
    }

    async processFrameQueue() {
        if (this.isDecompressing || this.frameQueue.length === 0) return;
        
        this.isDecompressing = true;
        
        try {
            const frame = this.frameQueue.shift();
            const decompressStart = performance.now();
            
            // High-performance decompression
            const rgbaData = await this.fastDecompressFrame(frame);
            
            this.perfStats.decompressTime = performance.now() - decompressStart;
            
            if (rgbaData) {
                // Hide status display when we start receiving frames
                if (this.statusDisplay && this.statusDisplay.style.display !== 'none') {
                    this.statusDisplay.style.display = 'none';
                }
                
                // Render on next animation frame for smooth 60fps
                requestAnimationFrame(() => {
                    this.fastRenderFrame(rgbaData, frame.width, frame.height);
                    this.previousFrameData = rgbaData; // Store for next delta
                });
            }
            
        } catch (error) {
            console.error('Frame processing error:', error);
        } finally {
            this.isDecompressing = false;
            
            // Continue processing queue
            if (this.frameQueue.length > 0) {
                this.processFrameQueue();
            }
        }
    }

    async fastDecompressFrame(frame) {
        const { rgbaData, compressedData, width, height, isKeyframe, format } = frame;
        
        if (format === 'rgba_direct') {
            // Ultra-fast RGBA format - zero decompression needed!
            return rgbaData;
        } else if (isKeyframe || !this.previousFrameData) {
            // Legacy RLE decompression
            return this.fastDecompressRLE(compressedData, width * height * 4);
        } else {
            // Fast delta application for legacy format
            return this.fastApplyDelta(compressedData, this.previousFrameData);
        }
    }

    fastDecompressRLE(compressedData, expectedSize) {
        const rgbaData = new Uint8Array(expectedSize);
        let outputIndex = 0;
        let inputIndex = 0;
        const length = compressedData.length;
        
        // Optimized RLE decompression with batch operations
        while (inputIndex < length && outputIndex < expectedSize) {
            const count = compressedData[inputIndex++];
            
            if (inputIndex + 4 > length) break;
            
            // Read RGBA values
            const r = compressedData[inputIndex++];
            const g = compressedData[inputIndex++];
            const b = compressedData[inputIndex++];
            const a = compressedData[inputIndex++];
            
            // Fast pixel replication using set() for larger chunks
            if (count > 8) {
                // Create a template pixel array for batch copying
                const pixelTemplate = new Uint8Array(count * 4);
                for (let i = 0; i < count * 4; i += 4) {
                    pixelTemplate[i] = r;
                    pixelTemplate[i + 1] = g;
                    pixelTemplate[i + 2] = b;
                    pixelTemplate[i + 3] = a;
                }
                
                // Batch copy to output
                const endIndex = outputIndex + count * 4;
                if (endIndex <= expectedSize) {
                    rgbaData.set(pixelTemplate, outputIndex);
                    outputIndex = endIndex;
                } else {
                    break;
                }
            } else {
                // Small count - direct loop is faster than array creation
                for (let i = 0; i < count && outputIndex < expectedSize; i++) {
                    rgbaData[outputIndex++] = r;
                    rgbaData[outputIndex++] = g;
                    rgbaData[outputIndex++] = b;
                    rgbaData[outputIndex++] = a;
                }
            }
        }
        
        return rgbaData;
    }

    fastApplyDelta(compressedData, previousFrame) {
        // Create copy using set() for fast cloning
        const rgbaData = new Uint8Array(previousFrame.length);
        rgbaData.set(previousFrame);
        
        const dataView = new DataView(compressedData.buffer, compressedData.byteOffset, compressedData.byteLength);
        
        if (compressedData.length < 4) return rgbaData;
        
        const changeCount = dataView.getUint32(0, true);
        let offset = 4;
        
        // Batch delta application with bounds checking
        const maxChanges = Math.min(changeCount, (compressedData.length - 4) / 8);
        
        for (let i = 0; i < maxChanges; i++) {
            const pixelIndex = dataView.getUint32(offset, true);
            offset += 4;
            
            const byteIndex = pixelIndex * 4;
            if (byteIndex + 3 < rgbaData.length) {
                // Unrolled pixel copy for speed
                rgbaData[byteIndex] = compressedData[offset];
                rgbaData[byteIndex + 1] = compressedData[offset + 1];
                rgbaData[byteIndex + 2] = compressedData[offset + 2];
                rgbaData[byteIndex + 3] = compressedData[offset + 3];
            }
            offset += 4;
        }
        
        return rgbaData;
    }

    fastRenderFrame(rgbaData, width, height) {
        const renderStart = performance.now();
        
        // Validate input data
        if (!rgbaData || rgbaData.length === 0) {
            console.error('❌ fastRenderFrame: No RGBA data provided');
            return;
        }
        
        const expectedSize = width * height * 4;
        if (rgbaData.length !== expectedSize) {
            console.warn(`⚠️ RGBA data size mismatch: got ${rgbaData.length}, expected ${expectedSize}`);
        }
        
        // Initialize canvas with optimal settings
        if (!this.realCanvas || !this.realCtx) {
            console.log('🎨 Initializing canvas for first frame render:', width, 'x', height);
            this.initializeOptimizedCanvas(width, height);
        }
        
        // Ensure canvas and context are available
        if (!this.realCanvas || !this.realCtx) {
            console.error('❌ Failed to initialize canvas for rendering');
            return;
        }
        
        // Resize canvas if needed (rare case)
        if (this.realCanvas.width !== width || this.realCanvas.height !== height) {
            console.log('📐 Resizing canvas:', this.realCanvas.width, 'x', this.realCanvas.height, '->', width, 'x', height);
            this.realCanvas.width = width;
            this.realCanvas.height = height;
        }
        
        // Fast ImageData creation and rendering
        const imageData = this.realCtx.createImageData(width, height);
        imageData.data.set(rgbaData); // Fast typed array copy
        
        // Single putImageData call for maximum performance
        this.realCtx.putImageData(imageData, 0, 0);
        
        this.perfStats.renderTime = performance.now() - renderStart;
        this.perfStats.totalFrames++;
        
        // Update performance stats every 60 frames (1 second at 60fps)
        const now = performance.now();
        if (now - this.perfStats.lastStatsUpdate > 1000) {
            this.updatePerformanceDisplay();
            this.perfStats.lastStatsUpdate = now;
        }
    }

    initializeOptimizedCanvas(width, height) {
        console.log('🚀 Initializing high-performance canvas renderer...');
        
        // Don't recreate if already exists with same dimensions
        if (this.realCanvas && this.realCanvas.width === width && this.realCanvas.height === height) {
            return;
        }
        
        // Create new canvas or reuse existing
        if (!this.realCanvas) {
            this.realCanvas = document.createElement('canvas');
            this.realCanvas.id = 'real-canvas';
        }
        
        this.realCanvas.width = width;
        this.realCanvas.height = height;
        
        // Optimized canvas styling for performance
        this.realCanvas.style.cssText = `
            width: 100%;
            height: 100%;
            max-width: 100vw;
            max-height: 100vh;
            object-fit: contain;
            background-color: #000;
            display: block;
            image-rendering: pixelated;
            image-rendering: -moz-crisp-edges;
            image-rendering: crisp-edges;
            cursor: default;
        `;
        
        // Get context with performance optimizations
        this.realCtx = this.realCanvas.getContext('2d', {
            alpha: false,           // No transparency for better performance
            desynchronized: true,   // Allow async rendering
            willReadFrequently: false  // We only write, never read
        });
        
        // Disable antialiasing for pixel-perfect rendering
        this.realCtx.imageSmoothingEnabled = false;
        
        // Find the screen container or video parent
        const screenContainer = document.getElementById('screen');
        const videoContainer = this.videoScreen ? this.videoScreen.parentElement : screenContainer;
        
        if (videoContainer && !this.realCanvas.parentElement) {
            // Remove any existing fallback canvas
            if (this.fallbackCanvas && this.fallbackCanvas.parentElement) {
                this.fallbackCanvas.parentElement.removeChild(this.fallbackCanvas);
            }
            
            // Add the canvas to the container
            videoContainer.appendChild(this.realCanvas);
            
            // Hide video element since we're using canvas
            if (this.videoScreen) {
                this.videoScreen.style.display = 'none';
            }
        }
        
        // Add mouse event listeners to the new canvas (stop propagation to prevent double-firing with container)
        ['mousedown', 'mouseup', 'mousemove', 'wheel'].forEach(event => {
            this.realCanvas.addEventListener(event, (e) => {
                e.stopPropagation();
                this.handleMouseEvent(e);
            });
        });
        this.realCanvas.addEventListener('contextmenu', (e) => e.preventDefault());
        
        console.log(`✅ Optimized canvas initialized: ${width}x${height}`);
    }

    updatePerformanceDisplay() {
        const { decompressTime, renderTime, totalFrames, droppedFrames } = this.perfStats;
        
        // Calculate FPS and frame drop rate
        const fps = totalFrames;
        const dropRate = droppedFrames / (totalFrames + droppedFrames) * 100;
        const totalProcessingTime = decompressTime + renderTime;
        
        // Adaptive quality adjustment
        this.adjustAdaptiveQuality(totalProcessingTime, dropRate, fps);
        
        // Only log performance issues (not every update)
        if (decompressTime > 10 || renderTime > 5 || dropRate > 5) {
            console.warn(`⚡ Performance: decompress=${decompressTime.toFixed(1)}ms, render=${renderTime.toFixed(1)}ms, drops=${dropRate.toFixed(1)}%`);
        }
        
        // Reset counters
        this.perfStats.totalFrames = 0;
        this.perfStats.droppedFrames = 0;
        
        // Update frame stats for display
        if (!this.frameStats) this.frameStats = {};
        this.frameStats.currentFps = fps;
        this.frameStats.dropRate = dropRate;
        this.frameStats.avgDecompressTime = decompressTime;
        this.frameStats.avgRenderTime = renderTime;
        this.frameStats.totalLatency = totalProcessingTime;
    }

    adjustAdaptiveQuality(processingTime, dropRate, fps) {
        if (!this.adaptiveQuality.enabled) return;
        
        const now = performance.now();
        if (now - this.adaptiveQuality.lastAdjustment < this.adaptiveQuality.adjustmentInterval) {
            return;
        }
        
        // Performance thresholds (in milliseconds)
        const thresholds = {
            excellent: 8,   // < 8ms total processing
            good: 12,       // < 12ms total processing  
            poor: 20        // > 20ms processing or >5% drops
        };
        
        let newLevel = this.adaptiveQuality.currentLevel;
        
        // Determine quality adjustment needed
        if (processingTime > thresholds.poor || dropRate > 5 || fps < 45) {
            // Performance is poor - reduce quality
            if (this.adaptiveQuality.currentLevel === 'high') {
                newLevel = 'medium';
            } else if (this.adaptiveQuality.currentLevel === 'medium') {
                newLevel = 'low';
            }
        } else if (processingTime < thresholds.excellent && dropRate < 1 && fps >= 58) {
            // Performance is excellent - can increase quality
            if (this.adaptiveQuality.currentLevel === 'low') {
                newLevel = 'medium';
            } else if (this.adaptiveQuality.currentLevel === 'medium') {
                newLevel = 'high';
            }
        }
        
        // Apply quality change if needed
        if (newLevel !== this.adaptiveQuality.currentLevel) {
            this.applyQualityLevel(newLevel);
            this.adaptiveQuality.currentLevel = newLevel;
            this.adaptiveQuality.lastAdjustment = now;
            
            console.log(`🎯 Adaptive quality: ${this.adaptiveQuality.currentLevel} (processing: ${processingTime.toFixed(1)}ms, drops: ${dropRate.toFixed(1)}%)`);
        }
    }

    applyQualityLevel(level) {
        switch (level) {
            case 'low':
                this.maxQueueSize = 1;  // Ultra-aggressive frame dropping
                this.adaptiveQuality.adjustmentInterval = 1000;  // More frequent adjustments
                break;
            case 'medium':
                this.maxQueueSize = 2;  // Moderate frame dropping
                this.adaptiveQuality.adjustmentInterval = 1500;
                break;
            case 'high':
                this.maxQueueSize = 3;  // Standard frame dropping
                this.adaptiveQuality.adjustmentInterval = 2000;
                break;
        }
        
        // Send quality preference to server if connection exists
        if (this.connected && this.ws.readyState === WebSocket.OPEN) {
            const qualityMap = { low: 65, medium: 80, high: 95 };
            this.ws.send(JSON.stringify({
                type: 'quality_update',
                quality: qualityMap[level],
                adaptive: true
            }));
        }
    }

    // Minimal overlay - only render when performance is stable
    addRealStreamingOverlay() {
        // Skip overlay rendering in high-performance mode to reduce latency
        if (this.perfStats.decompressTime > 8 || this.perfStats.renderTime > 3) {
            return; // Skip overlay when performance is critical
        }
        
        // Only render overlay every 30 frames to reduce overhead
        if (this.frameLogCounter % 30 !== 0) return;
        
        const ctx = this.realCtx;
        const canvas = this.realCanvas;
        
        // Minimal performance-optimized overlay
        ctx.fillStyle = 'rgba(0, 0, 0, 0.6)';
        ctx.fillRect(canvas.width - 120, 10, 110, 50);
        
        ctx.fillStyle = '#00ff88';
        ctx.font = '12px monospace';
        ctx.textAlign = 'left';
        ctx.fillText(`${(this.frameStats?.currentFps || 0).toFixed(0)} FPS`, canvas.width - 115, 25);
        
        if (this.frameStats?.dropRate > 0) {
            ctx.fillStyle = '#ff6b6b';
            ctx.fillText(`${this.frameStats.dropRate.toFixed(1)}% drop`, canvas.width - 115, 40);
        } else {
            ctx.fillStyle = '#88aaff';
            ctx.fillText('LIVE', canvas.width - 115, 40);
        }
        
        ctx.textAlign = 'center';
    }

    // Legacy method - no longer used since we decode actual frames
    renderBinaryFrame(videoData) {
        console.warn('renderBinaryFrame called - this should not happen with H.264 frame decoding');
    }

    // Legacy video frame handler - H.264 frames are handled via handleH264VideoFrame
    handleVideoFrame(data) {
        // H.264 binary frames are handled directly by handleBinaryVideoFrame
        // This method exists for JSON-based frame messages (legacy)
        if (!this.frameLogCounter) this.frameLogCounter = 0;
        if (this.frameLogCounter % 30 === 0) {
            console.log('Video frame received:', data.codec, 'size:', (data.data?.length / 1024).toFixed(1) + 'KB');
        }
        this.frameLogCounter++;
        
        if (!data.data) {
            console.error('No video data received');
            return;
        }
        
        try {
            const videoData = this.base64ToArrayBuffer(data.data);
            
            // Process as raw frame data
            this.parseAndRenderFrame(videoData);
            this.updateFrameStats();
            
        } catch (e) {
            console.error('Error handling video frame:', e);
            this.showError('Video frame processing error');
        }
    }

    // Stream overlay for debugging
    addStreamOverlay(ctx, canvas, frameNumber, dataSize) {
        // Add semi-transparent overlay with stream info (top-left)
        ctx.fillStyle = 'rgba(0, 0, 0, 0.8)';
        ctx.fillRect(10, 10, 280, 100);
        
        // Border for the info panel
        ctx.strokeStyle = '#4a90e2';
        ctx.lineWidth = 2;
        ctx.strokeRect(10, 10, 280, 100);
        
        // Add stream information text
        ctx.fillStyle = '#ffffff';
        ctx.font = 'bold 14px Arial';
        ctx.textAlign = 'left';
        const fps = this.frameStats?.currentFps || 0;
        
        ctx.fillText('🖥️ H.264 Remote Desktop', 20, 30);
        ctx.font = '12px monospace';
        ctx.fillStyle = '#00ff88';
        ctx.fillText(`Frame: #${frameNumber}`, 20, 50);
        ctx.fillText(`FPS: ${fps}`, 150, 50);
        ctx.fillStyle = '#ffaa00';
        ctx.fillText(`Data: ${(dataSize / 1024).toFixed(1)} KB`, 20, 70);
        ctx.fillText(`Resolution: ${canvas.width}x${canvas.height}`, 20, 90);
        
        // Add connection status indicator (top-right)
        ctx.fillStyle = '#4caf50';
        ctx.beginPath();
        ctx.arc(canvas.width - 30, 30, 12, 0, Math.PI * 2);
        ctx.fill();
        
        ctx.fillStyle = '#ffffff';
        ctx.font = 'bold 10px Arial';
        ctx.textAlign = 'center';
        ctx.fillText('LIVE', canvas.width - 30, 35);
        
        // Reset text alignment
        ctx.textAlign = 'left';
    }

    isValidVideoData(data) {
        // Basic validation for H.264 data
        const view = new Uint8Array(data);
        
        // Check if it looks like valid data
        if (view.length >= 10) {
            return view.length > 0;
        }
        
        return view.length > 0;
    }

    requestKeyframe() {
        const now = performance.now();
        
        // Don't spam keyframe requests
        if (this.lastKeyframeRequest && (now - this.lastKeyframeRequest) < 1000) {
            return;
        }
        
        this.lastKeyframeRequest = now;
        console.log('Requesting keyframe from server');
        
        this.sendMessage({
            type: 'request_keyframe'
        });
    }

    handleAutoplayFailed() {
        // Show a play button overlay
        this.showPlayButton();
    }

    showPlayButton() {
        if (this.playButton) return; // Already showing
        
        this.playButton = document.createElement('button');
        this.playButton.textContent = '▶ Click to Play';
        this.playButton.className = 'play-button-overlay';
        this.playButton.style.cssText = `
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            background: rgba(0,0,0,0.8);
            color: white;
            border: none;
            padding: 15px 25px;
            border-radius: 5px;
            font-size: 16px;
            cursor: pointer;
            z-index: 1000;
        `;
        
        this.playButton.onclick = () => {
            this.videoScreen.play().then(() => {
                this.playButton.remove();
                this.playButton = null;
            }).catch(console.error);
        };
        
        const screenElement = document.getElementById('screen');
        if (screenElement) {
            screenElement.appendChild(this.playButton);
        }
    }

    handleWebRTCFrame(data) {
        console.log('Received WebRTC frame:', {
            size: data.data ? data.data.length : 0,
            isKeyframe: data.is_keyframe,
            timestamp: data.timestamp,
            sequence: data.sequence_number
        });
        
        try {
            if (!data.data) {
                console.error('No WebRTC frame data received');
                return;
            }

            // H.264 is the only supported codec
            if (!data.codec) {
                data.codec = 'h264';
            }

            // Skip non-keyframes if we haven't received a keyframe yet
            if (this.needsKeyframe && !data.is_keyframe) {
                console.log('Skipping non-keyframe while waiting for keyframe');
                this.requestKeyframe();
                return;
            }

            if (data.is_keyframe) {
                this.needsKeyframe = false;
                console.log('Received H.264 keyframe, enabling playback');
            }

            // Process the frame via standard video frame handler
            this.handleVideoFrame(data);
            
            this.updateFrameStats();
            
        } catch (e) {
            console.error('Error handling WebRTC frame:', e);
            this.showError('WebRTC frame processing error');
        }
    }

    handleStreamingStats(data) {
        console.log('Streaming stats:', data);
        
        // Update network stats display
        if (this.networkStats) {
            this.networkStats.innerHTML = `
                <div>Frames: ${data.frames_sent || 0}</div>
                <div>Bitrate: ${data.current_bitrate_kbps || 0} kbps</div>
                <div>Latency: ~${this.latency || 0}ms</div>
            `;
        }
    }

    // WebRTC quality switching
    switchQuality(quality) {
        if (this.websocket && this.websocket.readyState === WebSocket.OPEN) {
            this.websocket.send(JSON.stringify({
                type: 'quality_change',
                quality: quality
            }));
            
            this.showNotification(`Quality changed to ${quality}`, 2000);
            console.log(`Quality switched to: ${quality}`);
        }
    }

    // Auto quality adaptation based on network stats
    autoAdaptQuality() {
        if (!this.config.adaptiveQuality) return;
        
        const stats = this.networkStats;
        let recommendedQuality = 'medium';
        
        // High quality: Good bandwidth (>6 Mbps), low latency (<50ms), minimal packet loss (<1%)
        if (stats.bandwidth > 6000 && stats.latency < 50 && stats.packetLoss < 1.0) {
            recommendedQuality = 'high';
        }
        // Low quality: Poor conditions
        else if (stats.bandwidth < 2000 || stats.latency > 200 || stats.packetLoss > 5.0) {
            recommendedQuality = 'low';
        }
        
        if (recommendedQuality !== this.currentQuality) {
            this.currentQuality = recommendedQuality;
            this.switchQuality(recommendedQuality);
        }
    }

    // Start network monitoring and adaptive quality
    startNetworkMonitoring() {
        // Send network stats every 5 seconds
        this.networkMonitoringInterval = setInterval(() => {
            this.sendNetworkStats();
            if (this.adaptiveQuality) {
                this.autoAdaptQuality();
            }
        }, 5000);
    }

    // Stop network monitoring
    stopNetworkMonitoring() {
        if (this.networkMonitoringInterval) {
            clearInterval(this.networkMonitoringInterval);
            this.networkMonitoringInterval = null;
        }
    }

    // Start connection health monitoring to detect stream freezes
    startConnectionHealthMonitoring() {
        // Check connection health every 3 seconds
        this.connectionHealthInterval = setInterval(() => {
            const timeSinceLastFrame = Date.now() - this.lastFrameTime;
            
            // If no frames received for more than 10 seconds, consider connection stale
            if (timeSinceLastFrame > 10000 && this.connected) {
                console.warn(`⚠️ No frames received for ${(timeSinceLastFrame / 1000).toFixed(1)}s - connection may be stale`);
                
                // Try to request a keyframe to refresh the stream
                if (this.ws && this.ws.readyState === WebSocket.OPEN) {
                    this.ws.send(JSON.stringify({ type: 'request_keyframe' }));
                    console.log('🔄 Requested keyframe to refresh stream');
                }
                
                // If still no frames after 20 seconds, attempt reconnection
                if (timeSinceLastFrame > 20000) {
                    console.error('❌ Stream appears frozen - attempting reconnection');
                    this.attemptReconnection();
                }
            }
        }, 3000);
    }

    // Stop connection health monitoring
    stopConnectionHealthMonitoring() {
        if (this.connectionHealthInterval) {
            clearInterval(this.connectionHealthInterval);
            this.connectionHealthInterval = null;
        }
    }

    // Attempt to reconnect when connection is stale
    attemptReconnection() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.error('❌ Max reconnection attempts reached');
            this.updateStatus('Connection Lost', 'Unable to reconnect after multiple attempts. Please refresh the page.');
            return;
        }
        
        this.reconnectAttempts++;
        console.log(`🔄 Reconnection attempt ${this.reconnectAttempts}/${this.maxReconnectAttempts}`);
        
        // Close existing connection
        if (this.ws) {
            this.ws.close();
        }
        
        // Wait a moment before reconnecting
        setTimeout(() => {
            if (!this.connected) {
                this.connect();
            }
        }, 1000);
    }

    // Update network stats display
    updateNetworkStats(stats) {
        this.networkStats = stats;
        
        // Update UI if stats are visible
        if (this.showStats) {
            const bandwidthDisplay = document.getElementById('bandwidth-display');
            const latencyDisplay = document.getElementById('latency-display');
            const packetLossDisplay = document.getElementById('packet-loss-display');
            
            if (bandwidthDisplay) {
                bandwidthDisplay.textContent = `${(stats.bandwidth / 1000).toFixed(1)} Mbps`;
            }
            if (latencyDisplay) {
                latencyDisplay.textContent = `${stats.latency}ms`;
            }
            if (packetLossDisplay) {
                packetLossDisplay.textContent = `${stats.packetLoss.toFixed(1)}%`;
            }
        }
    }

    normalizeCodec(codec) {
        // Always return H.264 since it's our only supported codec
        return 'h264';
    }

    getCodecConfigurations(codec) {
        // H.264 codec configurations for WebCodecs
        console.log('Getting H.264 codec configurations');
        
        return [
            'avc1.42E01F',  // H.264 Baseline Level 3.1
            'avc1.4D401F',  // H.264 Main Level 3.1
            'avc1.640028',  // H.264 High Level 4.0
        ];
    }

    updateStatus(title, message, showSpinner = false) {
        if (this.statusDisplay) {
            const titleElement = this.statusDisplay.querySelector('h2');
            const messageElement = this.statusDisplay.querySelector('p');
            const spinnerElement = this.statusDisplay.querySelector('.loading-spinner');
            
            if (titleElement) {
                titleElement.textContent = title;
            }
            if (messageElement) {
                messageElement.textContent = message;
            }
            if (spinnerElement) {
                spinnerElement.style.display = showSpinner ? 'block' : 'none';
            }
            
            // Show the status display
            this.statusDisplay.style.display = 'flex';
        }
    }

    hideStatusDisplay() {
        if (this.statusDisplay) {
            this.statusDisplay.style.display = 'none';
        }
   }

    showError(message) {
        console.error('KVM Error:', message);
        this.updateStatus('Error', message, false);
        
        // Also show as notification if available
        if (this.showNotification) {
            this.showNotification(message, 5000);
        }
    }

    // Utility methods
    base64ToArrayBuffer(base64) {
        // Remove data URL prefix if present
        const base64Data = base64.replace(/^data:.*,/, '');
        
        // Decode base64 string
        const binaryString = atob(base64Data);
        const bytes = new Uint8Array(binaryString.length);
        
        for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }
        
        return bytes.buffer;
    }
}

// Initialize the KVM client when the page loads
document.addEventListener('DOMContentLoaded', () => {
    // Get configuration from global variable set by the template
    const config = window.KVM_CONFIG || {
        stretch: false,
        mute: false,
        audio: false,
        remoteOnly: false,
        encryption: false,
        monitor: 0,
        codec: "h264"
    };

    // Initialize template components
    if (window.TemplateInitializer) {
        TemplateInitializer.initialize(config);
    } else {
        // Fallback initialization if template parts not loaded
        document.querySelectorAll('.toggle-switch').forEach(toggle => {
            const setting = toggle.dataset.setting;
            const checkbox = toggle.querySelector('input');
            
            if (checkbox) {
                toggle.addEventListener('click', () => {
                    checkbox.checked = !checkbox.checked;
                    toggle.classList.toggle('active', checkbox.checked);
                });
                
                toggle.classList.toggle('active', checkbox.checked);
            }
        });
    }

    // Initialize KVM client
    window.kvmClient = new KVMClient(config);
});

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
        this.qualityLevel = 50;
        this.availableMonitors = [];
        this.currentMonitor = config.monitor;
        this.currentCodec = config.codec || "vp9"; // VP9 via WebRTC DataChannel (RDEngine primary codec)
        this.videoQueue = [];
        this.showStats = false;

        // VP8/VP9 decoder (RustDesk-inspired, primary codec)
        this.vpxDecoder = null;
        this.serverCodec = 'vp9'; // Will be updated from server_info
        this.protocolVersion = 1; // v2 = rdengine binary protocol

        // rAF-based frame rendering for VP9 (vsync-aligned)
        this._pendingVpxFrame = null;
        this._vpxRafId = null;
        
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
        this.maxReconnectAttempts = Infinity; // Never give up reconnecting
        this._reconnectTimer = null;         // Pending reconnect setTimeout ID
        this._isReconnecting = false;        // Guard against overlapping reconnect attempts
        this._consecutiveFailures = 0;       // Track failures to trigger page reload fallback
        this._maxConsecutiveFailuresBeforeReload = 30; // ~60-90s of failures before reload
        
        // Multi-touch and gesture support
        this.touchIdentifiers = new Map();
        this.gestureInProgress = false;
        this.initialTouchDistance = 0;
        this.initialTouchAngle = 0;

        // WebRTC for audio
        this.peerConnection = null;
        this.audioStream = null;

        // ── WebRTC DataChannel Transport ─────────────────────────────────
        // When the server supports WebRTC (protocol_version >= 3), video
        // is delivered via a WebRTC *media track* for native <video>
        // element rendering. Audio and cursor use DataChannels.
        // WebSocket remains for signaling (SDP/ICE) and input (keyboard/mouse).
        //
        // Rendering modes (in priority order):
        //   1. WebRTC Media Track → <video>.srcObject (native, hw-accelerated)
        //   2. WebRTC DataChannel  → WebCodecs + canvas (fallback)
        //   3. WebSocket binary    → WebCodecs + canvas (legacy fallback)
        this.webrtcTransport = null;
        this.webrtcEnabled = false; // Will be set from server_info
        this.webrtcConnected = false;
        this.usingVideoElement = false; // True when using native <video> rendering

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

        // ── Client activity tracking ─────────────────────────────────────
        // When the client user is actively moving their mouse on the web
        // client, the host cursor overlay is hidden. The overlay only
        // appears when the remote host is the one moving the cursor.
        this.clientActive = false;
        this.clientActiveTimer = null;
        this.clientActiveTimeoutMs = 300; // ms of client inactivity before host cursor can reappear

        this.initializeElements();
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

    // Handle decoded VP8/VP9 frame with requestAnimationFrame for vsync-aligned rendering
    handleVpxFrame(frame) {
        // When using native <video> element rendering (WebRTC media track),
        // the browser handles decoding and rendering — skip canvas path
        if (this.usingVideoElement) {
            if (frame instanceof VideoFrame) {
                frame.close();
            }
            return;
        }

        if (!this.realCanvas || !this.realCtx) {
            this.initializeOptimizedCanvas(this.screenWidth, this.screenHeight);
        }
        
        if (frame instanceof VideoFrame) {
            // Drop any previous pending frame that hasn't been rendered yet
            // (we always want the newest frame, not old queued ones)
            if (this._pendingVpxFrame) {
                this._pendingVpxFrame.close();
            }
            this._pendingVpxFrame = frame;

            // Schedule rendering on the next display vsync if not already scheduled
            if (!this._vpxRafId) {
                this._vpxRafId = requestAnimationFrame(() => {
                    this._vpxRafId = null;
                    const f = this._pendingVpxFrame;
                    if (f) {
                        this._pendingVpxFrame = null;
                        this.realCtx.drawImage(f, 0, 0);
                        f.close();
                    }
                });
            }
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
            currentLevel: 'low',  // high, medium, low — default low for maximum smoothness
            performanceHistory: [],
            lastAdjustment: 0,
            adjustmentInterval: 2000  // Adjust every 2 seconds max
        };
        
        // Use OffscreenCanvas if available for background processing
        this.useOffscreenCanvas = typeof OffscreenCanvas !== 'undefined';
        if (this.useOffscreenCanvas) {
            console.log('Using OffscreenCanvas for background rendering');
        }
    }

    initializeElements() {
        // Main elements - VP9/VP8 uses canvas for display with WebCodecs decoder
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
        this.currentQuality = 'low';
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

        // Codec dropdown - VP9/VP8 selection
        if (this.codecDropdown) {
            this.codecDropdown.addEventListener('change', (e) => {
                const newCodec = e.target.value;
                if (newCodec !== this.currentCodec) {
                    this.currentCodec = newCodec;
                    this.serverCodec = newCodec;
                    console.log(`Switching codec to: ${newCodec}`);
                    this.showNotification(`Switched to ${newCodec.toUpperCase()}`, 2000);
                    // Reconnect to apply new codec
                    if (this.ws) {
                        this.ws.close();
                    }
                }
            });
        }

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

        // ── Client activity tracking ───────────────────────────────────
        // When the client moves their mouse, mark the client as active.
        // This hides the host cursor overlay so only one cursor is visible.
        if (e.type === 'mousemove') {
            this.setClientActive(true);
        }

        // ── Host control priority ──────────────────────────────────────
        // When the host is actively controlling the cursor, suppress
        // client mouse input to avoid conflicting cursor movements.
        // However, client mouse activity always takes precedence over
        // host control — if the client is moving, let them through.
        if (this.hostCursor.isHostControlling && !this.clientActive) {
            // Allow scroll events through (they don't move the cursor)
            if (e.type !== 'wheel') {
                return;
            }
        }
        
        // Use the appropriate element - prefer realCanvas (dynamically created), then fallbackCanvas, then videoScreen
        let targetElement = null;
        if (this.usingVideoElement && this.videoScreen) {
            // When using native <video> rendering, coordinate mapping uses the video element
            targetElement = this.videoScreen;
        } else if (this.realCanvas && this.realCanvas.parentElement) {
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
        
        // Use the appropriate target element - prefer native video element when active
        let targetElement = null;
        if (this.usingVideoElement && this.videoScreen) {
            targetElement = this.videoScreen;
        } else if (this.realCanvas && this.realCanvas.parentElement) {
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
        // Cancel any pending reconnection — user explicitly disconnected
        this._cancelPendingReconnect();
        this._isReconnecting = false;

        this.cleanupConnection();
        
        window.location.href = '/';
    }

    // ── Connection cleanup ─────────────────────────────────────────
    // Properly tears down the current WebSocket connection and all
    // associated state (intervals, decoder, pending frames) so that
    // a subsequent connect() call starts from a clean slate.
    cleanupConnection() {
        this.connected = false;

        // Stop all monitoring intervals
        if (this.pingInterval) {
            clearInterval(this.pingInterval);
            this.pingInterval = null;
        }
        this.stopNetworkMonitoring();
        this.stopConnectionHealthMonitoring();

        // Close the WebSocket and remove event handlers so the old
        // socket's onclose/onerror cannot fire after we create a new one.
        if (this.ws) {
            try {
                this.ws.onopen = null;
                this.ws.onmessage = null;
                this.ws.onclose = null;
                this.ws.onerror = null;
                if (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING) {
                    this.ws.close();
                }
            } catch (e) {
                // Ignore errors during cleanup
            }
            this.ws = null;
        }

        // Clean up pending VPX frames to avoid stale VideoFrame references
        if (this._pendingVpxFrame) {
            try { this._pendingVpxFrame.close(); } catch (e) {}
            this._pendingVpxFrame = null;
        }
        if (this._vpxRafId) {
            cancelAnimationFrame(this._vpxRafId);
            this._vpxRafId = null;
        }

        // Close WebRTC DataChannel transport
        if (this.webrtcTransport) {
            this.webrtcTransport.close();
            this.webrtcTransport = null;
        }
        this.webrtcConnected = false;
        this.webrtcEnabled = false;

        // Clear host cursor state
        if (this.hostControlTimer) {
            clearTimeout(this.hostControlTimer);
            this.hostControlTimer = null;
        }
        if (this.clientActiveTimer) {
            clearTimeout(this.clientActiveTimer);
            this.clientActiveTimer = null;
        }
        this.hostCursor.isHostControlling = false;
        this.clientActive = false;

        // Remove host cursor overlay from DOM
        const overlay = document.getElementById('host-cursor-overlay');
        if (overlay) overlay.remove();

        // Clear the video canvas to black so the user sees a blank screen
        // instead of a stale frozen frame while reconnecting.
        this.clearCanvasToBlack();
    }

    // Paint the rendering canvas solid black.
    // Called on disconnect/connection-lost so the last video frame
    // doesn't remain frozen on screen.
    clearCanvasToBlack() {
        if (this.realCanvas && this.realCtx) {
            this.realCtx.fillStyle = '#000';
            this.realCtx.fillRect(0, 0, this.realCanvas.width, this.realCanvas.height);
        }
        // Also clear the video element if visible
        if (this.videoScreen) {
            this.videoScreen.pause();
            this.videoScreen.removeAttribute('src');
            this.videoScreen.load();
        }
    }

    // Reset decoder state so a fresh connection gets a clean decoder
    // that will accept a new keyframe. Called before each reconnect.
    resetDecoderState() {
        this.needsKeyframe = true;
        this.frameLogCounter = 0;

        // Reset VPX decoder — close the old one and create a fresh instance
        if (this.vpxDecoder) {
            try { this.vpxDecoder.destroy(); } catch (e) {}
            this.vpxDecoder = null;
        }
        this.initializeVpxDecoder();

        console.log('Decoder state reset for reconnection');
    }

    // Cancel any pending reconnect timer
    _cancelPendingReconnect() {
        if (this._reconnectTimer) {
            clearTimeout(this._reconnectTimer);
            this._reconnectTimer = null;
        }
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

    // WebRTC setup for audio (legacy stub — replaced by WebRTC DataChannel transport)
    setupWebRTC(encryption) {
        if (!this.config.audio) return;
        console.log('Legacy WebRTC audio setup requested — using DataChannel transport instead');
    }

    /**
     * Handle a WebRTC SDP offer from the server.
     * Creates the WebRTC DataChannel transport for video/audio/cursor streaming.
     * The server creates DataChannels; we handle the offer/answer exchange.
     */
    async handleWebRTCOffer(data) {
        console.log('WebRTC offer received from server (DataChannel transport)');

        if (!data.sdp) {
            console.error('WebRTC offer missing SDP');
            return;
        }

        // Guard: don't create duplicate transports
        if (this.webrtcTransport) {
            console.warn('WebRTC transport already exists, closing old one');
            this.webrtcTransport.close();
            this.webrtcTransport = null;
        }

        // Check if WebRtcTransport class is available
        if (typeof WebRtcTransport === 'undefined') {
            console.warn('WebRtcTransport not loaded — falling back to WebSocket binary');
            return;
        }

        try {
            // Create the WebRTC transport with callbacks that route frames
            // to the existing binary frame handler (same parsing logic)
            this.webrtcTransport = new WebRtcTransport({
                // Primary video path: native media stream → <video> element
                onMediaStream: (stream) => {
                    console.log('WebRTC media stream received — switching to native <video> rendering');
                    this.activateVideoElementRendering(stream);
                },
                // Fallback video path: DataChannel → WebCodecs → canvas
                onVideoFrame: (arrayBuffer) => {
                    // Route through the existing binary video frame handler
                    this.lastFrameTime = Date.now();
                    this.handleBinaryVideoFrame(arrayBuffer);
                },
                onAudioFrame: (arrayBuffer) => {
                    // Route through the existing binary audio handler
                    this.handleBinaryVideoFrame(arrayBuffer);
                },
                onCursorUpdate: (arrayBuffer) => {
                    // Route through the existing cursor handler
                    this.handleCursorMessage(arrayBuffer);
                },
                onStateChange: (state) => {
                    console.log(`WebRTC transport state: ${state}`);
                    this.webrtcConnected = (state === 'connected');

                    if (state === 'connected') {
                        this.showNotification('WebRTC connected — low-latency mode active', 3000);
                    } else if (state === 'failed' || state === 'disconnected') {
                        this.webrtcConnected = false;
                        // Fall back to canvas rendering if media track was active
                        if (this.usingVideoElement) {
                            this.deactivateVideoElementRendering();
                        }
                        this.showNotification('WebRTC disconnected — using WebSocket fallback', 3000);
                    }
                },
                onIceCandidate: (candidateJson) => {
                    // Send ICE candidate to server via WebSocket signaling
                    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
                        this.ws.send(JSON.stringify({
                            type: 'webrtc_ice_candidate',
                            candidate: JSON.parse(candidateJson),
                        }));
                    }
                },
            });

            // Handle the SDP offer and get the answer
            const sdpAnswer = await this.webrtcTransport.handleOffer(data.sdp);

            // Send the SDP answer back to the server via WebSocket
            if (this.ws && this.ws.readyState === WebSocket.OPEN) {
                this.ws.send(JSON.stringify({
                    type: 'webrtc_answer',
                    sdp: sdpAnswer,
                }));
                console.log('WebRTC SDP answer sent to server');
            }

            this.webrtcEnabled = true;
        } catch (e) {
            console.error('Failed to set up WebRTC transport:', e);
            this.webrtcTransport = null;
            this.webrtcEnabled = false;
            // Fallback: continue using WebSocket binary transport
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
        // Guard: if already reconnecting, don't create parallel connections
        if (this._isReconnecting) {
            console.log('Reconnection already in progress, skipping duplicate connect()');
            return;
        }

        this.updateStatus(
            this.reconnectAttempts > 0 ? 'Reconnecting' : 'Connecting',
            this.reconnectAttempts > 0 ? 'Attempting to reach the server...' : 'Establishing connection...',
            true);
        
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        
        // Get the hostname from the current URL - this should preserve IP addresses and hostnames
        let hostname = window.location.hostname;
        
        // Check for manual server override in URL parameters
        const urlParams = new URLSearchParams(window.location.search);
        const serverOverride = urlParams.get('server');
        
        // Determine the WebSocket host
        let wsHost;
        if (serverOverride) {
            wsHost = serverOverride;
        } else if (window.location.port && window.location.port !== '80' && window.location.port !== '443') {
            wsHost = `${hostname}:9921`;
        } else {
            wsHost = `${hostname}:9921`;
        }
        
        const wsUrl = `${protocol}//${wsHost}/ws?monitor=${this.currentMonitor}&codec=${this.currentCodec}${this.config.audio ? '&audio=true' : ''}`;
        
        console.log(`Connecting to WebSocket: ${wsUrl} (attempt ${this.reconnectAttempts})`);
        
        // Clean up any existing connection state before creating a new one.
        // This prevents old onclose/onerror handlers from firing and creating
        // cascading reconnect attempts.
        this.cleanupConnection();

        try {
            this.ws = new WebSocket(wsUrl);
        } catch (e) {
            console.error('Failed to create WebSocket:', e);
            this._scheduleReconnect(wsHost);
            return;
        }
        this.ws.binaryType = 'arraybuffer';
        
        this.ws.onopen = () => {
            this.connected = true;
            this.reconnectAttempts = 0;
            this._consecutiveFailures = 0;
            this._isReconnecting = false;
            this.lastFrameTime = Date.now();
            this.updateStatus('Connected', 'Connection established successfully');
            console.log('WebSocket connection established');
            
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
                if (event.data instanceof ArrayBuffer) {
                    this.handleBinaryVideoFrame(event.data);
                } else if (event.data instanceof Blob) {
                    const arrayBuffer = await event.data.arrayBuffer();
                    this.handleBinaryVideoFrame(arrayBuffer);
                } else {
                    const data = JSON.parse(event.data);
                    this.handleMessage(data);
                }
            } catch (e) {
                console.error('Error handling WebSocket message:', e);
            }
        };
        
        this.ws.onclose = (event) => {
            const wasConnected = this.connected;
            this.connected = false;
            
            // Stop intervals (ping, network monitor, health check)
            if (this.pingInterval) {
                clearInterval(this.pingInterval);
                this.pingInterval = null;
            }
            this.stopNetworkMonitoring();
            this.stopConnectionHealthMonitoring();
            
            console.log('WebSocket closed. Code:', event.code, 'Reason:', event.reason,
                        wasConnected ? '(was connected)' : '(was not connected)');
            
            if (event.code === 1006) {
                this.updateStatus('Connection Lost',
                    'Lost connection to the server', true);
            } else if (event.code === 1000) {
                this.updateStatus('Reconnecting',
                    'Server closed the connection', true);
            } else {
                this.updateStatus('Reconnecting',
                    'Connection closed unexpectedly', true);
            }
            
            // Schedule automatic reconnection
            this._scheduleReconnect(wsHost);
        };
        
        this.ws.onerror = (error) => {
            // onerror always fires before onclose for connection failures,
            // so we only log here — onclose handles the reconnect scheduling.
            console.error('WebSocket error for:', wsUrl);
        };
    }

    // Schedule a reconnection attempt with exponential backoff.
    // Capped at 10 seconds. After many consecutive failures, falls back
    // to a full page reload (to re-accept any changed TLS certificates).
    _scheduleReconnect(wsHost) {
        // Don't schedule if there's already a pending reconnect
        if (this._reconnectTimer) return;

        this.reconnectAttempts++;
        this._consecutiveFailures++;

        // Exponential backoff: 1s, 2s, 3s, 4s, 5s, ... capped at 10s
        const delay = Math.min(1000 * Math.min(this.reconnectAttempts, 10), 10000);

        console.log(`Scheduling reconnect in ${delay}ms (attempt ${this.reconnectAttempts}, consecutive failures: ${this._consecutiveFailures})`);

        // After many consecutive failures the TLS certificate may have changed
        // (server was restarted and generated a new self-signed cert).
        // In that case, a full page reload is the only way for the browser
        // to re-accept the new certificate.
        if (this._consecutiveFailures >= this._maxConsecutiveFailuresBeforeReload) {
            console.warn('Too many consecutive failures — reloading page to re-accept TLS certificate');
            this.updateStatus('Reconnecting',
                'Reloading page...', true);
            setTimeout(() => window.location.reload(), 1000);
            return;
        }

        this.updateStatus('Reconnecting',
            'Attempting to reach the server...', true);

        this._reconnectTimer = setTimeout(() => {
            this._reconnectTimer = null;
            if (!this.connected) {
                // Reset decoder state before reconnect so the new connection
                // starts with a clean decoder ready to accept a keyframe.
                this.resetDecoderState();
                this.connect();
            }
        }, delay);
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
            case 'webrtc_ice_candidate':
                // Server sent an ICE candidate — add to our WebRTC transport
                if (this.webrtcTransport && data.candidate) {
                    this.webrtcTransport.addIceCandidate(data.candidate);
                }
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
        const serverCodec = (data.codec || 'vp9').toLowerCase();
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
        
        // Initialize VPX decoder for VP8/VP9 (the only supported codecs)
        if (serverCodec === 'vp8' || serverCodec === 'vp9') {
            if (this.vpxDecoder) {
                this.vpxDecoder.setCodec(serverCodec);
                this.vpxDecoder.setDimensions(this.screenWidth, this.screenHeight);
                console.log(`VPX decoder configured for ${serverCodec} ${this.screenWidth}x${this.screenHeight}`);
            } else {
                console.warn('VPX decoder not available, initializing...');
                this.initializeVpxDecoder();
            }
        } else {
            // Unsupported codec - fall back to VP9
            console.warn(`Unsupported codec '${serverCodec}', falling back to VP9`);
            this.serverCodec = 'vp9';
            if (this.vpxDecoder) {
                this.vpxDecoder.setCodec('vp9');
                this.vpxDecoder.setDimensions(this.screenWidth, this.screenHeight);
            } else {
                this.initializeVpxDecoder();
            }
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
        
        // Update codec dropdown to match server codec
        console.log('Using codec:', this.currentCodec);
        if (this.codecDropdown) {
            this.codecDropdown.value = this.currentCodec;
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
        
        console.log('[INFO] Initializing VP9 video streaming');
        
        // VP9 uses canvas-based rendering with WebCodecs decoder
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
        
        // Initialize optimized canvas for VP9 frame rendering
        this.initializeOptimizedCanvas(this.screenWidth, this.screenHeight);
    }

    processVideoQueue() {
        // Process queued video frames if any
        if (this.videoQueue.length === 0) {
            return;
        }
        
        // VP9 frames are processed directly by the VPX decoder
        const frame = this.videoQueue.shift();
        if (frame && this.vpxDecoder && this.vpxDecoder.isReady) {
            this.vpxDecoder.decode(frame.data, frame.metadata);
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
            
            // Fall back to RGBA/RLE frame parsing for legacy formats
            this.parseAndRenderFrame(binaryData);
            
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

        // Only render the host cursor overlay if the client is NOT
        // actively using their mouse. When the client is moving their
        // cursor on the web page, the host overlay is suppressed to
        // avoid two cursors competing for the user's attention.
        if (!this.clientActive) {
            this.renderHostCursor();
        } else {
            this.hideHostCursor();
        }
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
     * Mark the client as actively using their mouse.
     * While the client is active, the host cursor overlay is hidden.
     * After `clientActiveTimeoutMs` of inactivity, the client is marked
     * inactive and the host cursor overlay can reappear if the host is
     * still sending cursor updates.
     */
    setClientActive(active) {
        this.clientActive = active;

        // Clear any previous client-activity expiry timer
        if (this.clientActiveTimer) {
            clearTimeout(this.clientActiveTimer);
            this.clientActiveTimer = null;
        }

        if (active) {
            // Hide the host cursor overlay while the client is moving
            this.hideHostCursor();

            // Restore the client's native cursor (override host-control hiding)
            this.setClientCursorStyle('default');

            // After the client stops moving, allow the host cursor to reappear
            this.clientActiveTimer = setTimeout(() => {
                this.clientActive = false;

                // If the host is still in control, re-render the host cursor
                if (this.hostCursor.isHostControlling) {
                    this.renderHostCursor();
                    this.setClientCursorStyle('none');
                }
            }, this.clientActiveTimeoutMs);
        }
    }

    /**
     * Hide the host cursor overlay element with a smooth fade-out.
     */
    hideHostCursor() {
        if (this.hostCursorOverlay) {
            this.hostCursorOverlay.classList.add('host-cursor-hidden');
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
        this.hostCursorOverlay.classList.remove('host-cursor-hidden');

        // Convert host screen coordinates → CSS pixel position on the rendered content area
        const targetElement = (this.usingVideoElement && this.videoScreen)
            ? this.videoScreen
            : (this.realCanvas || this.videoScreen);
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
        
        // Codec IDs: 0x01=VP8, 0x02=VP9
        const codecName = codec === 0x01 ? 'vp8' : 'vp9';
        
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
        
        // Decode with VP8/VP9 decoder
        if (this.vpxDecoder && this.vpxDecoder.isReady) {
            this.vpxDecoder.decode(encodedData, {
                isKeyframe,
                timestamp: timestampMs * 1000, // Convert ms to us for WebCodecs
            });
            return;
        }
        
        // No suitable decoder available
        if (this.frameLogCounter < 5) {
            console.warn(`No decoder for codec: ${codecName}`);
        }
    }

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
                console.log(`[INFO] Screen dimensions updated from frame: ${width}x${height}`);
                
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
                console.log(`[INFO] RGBA frame: ${width}x${height}, frame #${frameNumber}, data: ${dataLength} bytes`);
            }
            
            if (dataView.byteLength < offset + dataLength) {
                console.error(`[ERROR] RGBA frame truncated: need ${offset + dataLength} bytes, got ${dataView.byteLength} bytes`);
                return;
            }
            
            // Direct RGBA data - MUST copy the data since ArrayBuffer may be reused
            const rgbaData = new Uint8Array(dataLength);
            rgbaData.set(new Uint8Array(arrayBuffer, offset, dataLength));
            
            // Log first frame for debugging
            if (frameNumber < 3n) {
                console.log(`[DEBUG] Frame ${frameNumber} RGBA data: first bytes = [${rgbaData[0]}, ${rgbaData[1]}, ${rgbaData[2]}, ${rgbaData[3]}]`);
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
            console.error('[ERROR] fastRenderFrame: No RGBA data provided');
            return;
        }
        
        const expectedSize = width * height * 4;
        if (rgbaData.length !== expectedSize) {
            console.warn(`[WARNING] RGBA data size mismatch: got ${rgbaData.length}, expected ${expectedSize}`);
        }
        
        // Initialize canvas with optimal settings
        if (!this.realCanvas || !this.realCtx) {
            console.log('[INFO] Initializing canvas for first frame render:', width, 'x', height);
            this.initializeOptimizedCanvas(width, height);
        }
        
        // Ensure canvas and context are available
        if (!this.realCanvas || !this.realCtx) {
            console.error('[ERROR] Failed to initialize canvas for rendering');
            return;
        }
        
        // Resize canvas if needed (rare case)
        if (this.realCanvas.width !== width || this.realCanvas.height !== height) {
            console.log('[INFO] Resizing canvas:', this.realCanvas.width, 'x', this.realCanvas.height, '->', width, 'x', height);
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
        console.log('[INFO] Initializing high-performance canvas renderer...');
        
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
        
        console.log(`[INFO] Optimized canvas initialized: ${width}x${height}`);
    }

    /**
     * Activate native <video> element rendering using a WebRTC MediaStream.
     *
     * When the server sends video via a WebRTC media track (instead of
     * DataChannel binary frames), the browser can decode and render it
     * natively — no WebCodecs VideoDecoder or canvas drawing needed.
     *
     * Benefits over canvas rendering:
     *   - Hardware-accelerated VP9/VP8 decoding by the browser
     *   - Browser handles jitter buffering and frame pacing
     *   - Native video scaling, compositing, and vsync
     *   - Lower CPU usage (no manual drawImage/putImageData calls)
     *   - No WebCodecs API dependency (broader browser support)
     *
     * @param {MediaStream} stream - The video MediaStream from WebRTC ontrack
     */
    activateVideoElementRendering(stream) {
        if (!this.videoScreen) {
            console.warn('Cannot activate video element rendering: <video> element not found');
            return;
        }

        // Assign the media stream to the video element
        this.videoScreen.srcObject = stream;
        this.videoScreen.style.display = 'block';
        this.videoScreen.style.cssText = `
            width: 100%;
            height: 100%;
            max-width: 100vw;
            max-height: 100vh;
            object-fit: contain;
            background-color: #000;
            display: block;
        `;

        // Attempt autoplay (required for WebRTC streams)
        this.videoScreen.play().catch(e => {
            console.warn('Video autoplay failed (user interaction may be required):', e.message);
        });

        // Update screen dimensions from the video metadata
        this.videoScreen.onloadedmetadata = () => {
            if (this.videoScreen.videoWidth > 0 && this.videoScreen.videoHeight > 0) {
                this.screenWidth = this.videoScreen.videoWidth;
                this.screenHeight = this.videoScreen.videoHeight;
                console.log(`Video element dimensions: ${this.screenWidth}x${this.screenHeight}`);
            }
        };

        // Also listen for resize events (resolution changes during streaming)
        this.videoScreen.onresize = () => {
            if (this.videoScreen.videoWidth > 0 && this.videoScreen.videoHeight > 0) {
                if (this.screenWidth !== this.videoScreen.videoWidth ||
                    this.screenHeight !== this.videoScreen.videoHeight) {
                    this.screenWidth = this.videoScreen.videoWidth;
                    this.screenHeight = this.videoScreen.videoHeight;
                    console.log(`Video resolution changed: ${this.screenWidth}x${this.screenHeight}`);
                }
            }
        };

        // Hide the canvas (not needed for native video rendering)
        if (this.realCanvas) {
            this.realCanvas.style.display = 'none';
        }

        // Cancel any pending VP9 requestAnimationFrame rendering
        if (this._vpxRafId) {
            cancelAnimationFrame(this._vpxRafId);
            this._vpxRafId = null;
        }
        if (this._pendingVpxFrame) {
            this._pendingVpxFrame.close();
            this._pendingVpxFrame = null;
        }

        // Mark that we're using native video rendering
        this.usingVideoElement = true;

        // Hide status display since we're now receiving video
        if (this.statusDisplay) {
            this.statusDisplay.style.display = 'none';
        }

        // FPS tracking for the video element
        if (this.videoScreen.requestVideoFrameCallback) {
            let lastTime = performance.now();
            let frameCounter = 0;
            const trackFps = (now, metadata) => {
                frameCounter++;
                // Keep lastFrameTime updated so the health monitor
                // knows we're still receiving frames via the media track
                this.lastFrameTime = Date.now();
                if (now - lastTime >= 1000) {
                    this.frameStats.currentFps = frameCounter;
                    frameCounter = 0;
                    lastTime = now;
                    // Update FPS display
                    const fpsElements = document.querySelectorAll('#fps');
                    fpsElements.forEach(el => el.textContent = this.frameStats.currentFps);
                }
                if (this.usingVideoElement) {
                    this.videoScreen.requestVideoFrameCallback(trackFps);
                }
            };
            this.videoScreen.requestVideoFrameCallback(trackFps);
        } else {
            // Fallback: use timeupdate event for browsers without requestVideoFrameCallback
            this.videoScreen.ontimeupdate = () => {
                this.lastFrameTime = Date.now();
            };
        }

        console.log('[INFO] Native <video> element rendering activated (WebRTC media track)');
    }

    /**
     * Deactivate native <video> element rendering and fall back to canvas.
     * Called when WebRTC connection drops or media track ends.
     */
    deactivateVideoElementRendering() {
        if (!this.usingVideoElement) return;

        console.log('Deactivating native <video> rendering, falling back to canvas');

        this.usingVideoElement = false;

        // Clear the video element
        if (this.videoScreen) {
            this.videoScreen.srcObject = null;
            this.videoScreen.style.display = 'none';
        }

        // Re-show the canvas for fallback rendering
        if (this.realCanvas) {
            this.realCanvas.style.display = 'block';
        }

        // Request a keyframe so the canvas decoder can start fresh
        this.requestKeyframe();
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
            console.warn(`[WARNING] Performance: decompress=${decompressTime.toFixed(1)}ms, render=${renderTime.toFixed(1)}ms, drops=${dropRate.toFixed(1)}%`);
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
        
        // Determine quality adjustment needed — bias toward LOW for smoothness
        if (processingTime > thresholds.good || dropRate > 3 || fps < 20) {
            // Performance is not great - reduce quality
            if (this.adaptiveQuality.currentLevel === 'high') {
                newLevel = 'medium';
            } else if (this.adaptiveQuality.currentLevel === 'medium') {
                newLevel = 'low';
            }
        } else if (processingTime < thresholds.excellent && dropRate < 0.5 && fps >= 22) {
            // Performance is excellent for sustained period - cautiously increase
            // Only go up to medium, never auto-promote to high
            if (this.adaptiveQuality.currentLevel === 'low') {
                newLevel = 'medium';
            }
        }
        
        // Apply quality change if needed
        if (newLevel !== this.adaptiveQuality.currentLevel) {
            this.applyQualityLevel(newLevel);
            this.adaptiveQuality.currentLevel = newLevel;
            this.adaptiveQuality.lastAdjustment = now;
            
            console.log(`[INFO] Adaptive quality: ${this.adaptiveQuality.currentLevel} (processing: ${processingTime.toFixed(1)}ms, drops: ${dropRate.toFixed(1)}%)`);
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
            this.ws.send(JSON.stringify({
                type: 'quality_update',
                quality: level
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
        console.warn('renderBinaryFrame called - this should not happen with VP9 frame decoding');
    }

    // Legacy video frame handler - VP9 frames are handled via handleRdEngineVideoFrame
    handleVideoFrame(data) {
        // VP9 binary frames are handled directly by handleBinaryVideoFrame
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
        
        ctx.fillText('VP9 Remote Desktop', 20, 30);
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
        // Basic validation for VP9 data
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

            // Default to VP9 codec if not specified
            if (!data.codec) {
                data.codec = 'vp9';
            }

            // Skip non-keyframes if we haven't received a keyframe yet
            if (this.needsKeyframe && !data.is_keyframe) {
                console.log('Skipping non-keyframe while waiting for keyframe');
                this.requestKeyframe();
                return;
            }

            if (data.is_keyframe) {
                this.needsKeyframe = false;
                console.log('Received keyframe, enabling playback');
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
                type: 'quality_update',
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
        let recommendedQuality = 'low';
        
        // High quality: Only under excellent conditions
        if (stats.bandwidth > 10000 && stats.latency < 20 && stats.packetLoss < 0.5) {
            recommendedQuality = 'high';
        }
        // Medium quality: Good conditions
        else if (stats.bandwidth > 4000 && stats.latency < 40 && stats.packetLoss < 2.0) {
            recommendedQuality = 'medium';
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
            // When using native <video> rendering, check the video element's
            // playback state instead of relying solely on lastFrameTime.
            // requestVideoFrameCallback already updates lastFrameTime, but
            // also check the element directly as an extra safety net.
            if (this.usingVideoElement && this.videoScreen) {
                const isPlaying = !this.videoScreen.paused && !this.videoScreen.ended
                    && this.videoScreen.readyState >= 2;
                if (isPlaying) {
                    // Video element is actively playing — stream is healthy
                    this.lastFrameTime = Date.now();
                    return;
                }
            }

            const timeSinceLastFrame = Date.now() - this.lastFrameTime;
            
            // If no frames received for more than 10 seconds, consider connection stale
            if (timeSinceLastFrame > 10000 && this.connected) {
                console.warn(`[WARNING] No frames received for ${(timeSinceLastFrame / 1000).toFixed(1)}s - connection may be stale`);
                
                // Try to request a keyframe to refresh the stream
                if (this.ws && this.ws.readyState === WebSocket.OPEN) {
                    this.ws.send(JSON.stringify({ type: 'request_keyframe' }));
                    console.log('[INFO] Requested keyframe to refresh stream');
                }
                
                // If still no frames after 20 seconds, attempt reconnection
                if (timeSinceLastFrame > 20000) {
                    console.error('[ERROR] Stream appears frozen - attempting reconnection');
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

    // Attempt to reconnect when connection is stale (called from health monitor)
    attemptReconnection() {
        console.log('Stream frozen — triggering reconnection');
        
        // Clean up the current (stale) connection and schedule a reconnect
        // via the unified _scheduleReconnect path.
        this.cleanupConnection();
        
        // Determine wsHost (same logic as connect)
        const hostname = window.location.hostname;
        const urlParams = new URLSearchParams(window.location.search);
        const serverOverride = urlParams.get('server');
        let wsHost;
        if (serverOverride) {
            wsHost = serverOverride;
        } else {
            wsHost = `${hostname}:9921`;
        }
        
        this._scheduleReconnect(wsHost);
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
        // VP9 is the primary codec, VP8 also supported
        if (codec === 'vp8') return 'vp8';
        return 'vp9';
    }

    getCodecConfigurations(codec) {
        // VP8/VP9 codec configurations for WebCodecs
        console.log('Getting VP9 codec configurations');
        
        return [
            'vp09.00.31.08',  // VP9 Profile 0, Level 3.1, 8-bit
            'vp09.00.41.08',  // VP9 Profile 0, Level 4.1, 8-bit
            'vp8',            // VP8 fallback
        ];
    }

    updateStatus(title, message, showSpinner = false) {
        if (this.statusDisplay) {
            const titleElement = this.statusDisplay.querySelector('h2');
            const messageElement = this.statusDisplay.querySelector('p');
            const spinnerElement = this.statusDisplay.querySelector('.loading-spinner');
            const tipsElement = this.statusDisplay.querySelector('.status-tips');
            
            if (titleElement) {
                titleElement.textContent = title;
            }
            if (messageElement) {
                messageElement.textContent = message;
            }
            if (spinnerElement) {
                spinnerElement.style.display = showSpinner ? 'block' : 'none';
            }

            // Show troubleshooting tips when reconnecting or connection lost
            if (tipsElement) {
                const isReconnecting = /reconnect|connection lost|disconnected/i.test(title);
                if (isReconnecting) {
                    tipsElement.innerHTML = `<ul>
                        <li>Check that the CLEVER KVM app is running on the host</li>
                        <li>Verify the host machine is reachable on the network</li>
                        <li>Ensure port 9921 is not blocked by a firewall</li>
                        <li>Try refreshing the page if the issue persists</li>
                    </ul>`;
                } else {
                    tipsElement.innerHTML = '';
                }
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
        codec: "vp9"
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

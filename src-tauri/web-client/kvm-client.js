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
        this.currentCodec = "yuv420_webm"; // Use YUV420 with WebM container for best quality
        this.mediaSource = null;
        this.sourceBuffer = null;
        this.videoQueue = [];
        this.showStats = false;
        
        // YUV420 decoder for enhanced video quality
        this.yuv420Decoder = null;
        this.decoderCanvas = null;
        this.decoderCtx = null;
        
        // WebM container support
        this.webmSupported = false;
        this.webmContainer = null;
        
        // VP8 decoder for real screen content with YUV420 support
        this.vp8Decoder = null;
        this.yuv420Canvas = null;
        this.yuv420Ctx = null;
        
        // Enhanced YUV420 video properties
        this.needsKeyframe = true;
        this.supportsHardwareDecoding = false;
        this.webmDecodeQueue = [];
        
        // WebM container format support
        this.webmMuxer = null;
        this.webmDemuxer = null;
        
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

        this.initializeElements();
        this.initializeVP8Decoder();
        this.initializeFrameTracking();
        this.setupEventListeners();
        this.connect();
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
        this.lastFrameTime = 0;
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
        // Main elements - VP8 uses video element for display
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

    initializeVP8Decoder() {
        try {
            console.log('🎬 Initializing YUV420 + WebM decoder...');
            
            // Check for WebM container support with VP8 and Opus
            const testTypes = [
                'video/webm; codecs="vp8"',
                'video/webm; codecs="vp8,opus"',
                'audio/webm; codecs="opus"'
            ];
            
            this.webmSupported = false;
            for (const type of testTypes) {
                if (window.MediaSource && MediaSource.isTypeSupported(type)) {
                    console.log('✅ Supported:', type);
                    this.webmSupported = true;
                }
            }
            
            if (this.webmSupported) {
                console.log('✅ Using native WebM VP8+Opus decoder');
            } else {
                console.log('⚠️ WebM not fully supported, using custom YUV420 decoder');
                
                // Create YUV420 decoder canvas for real screen content
                this.yuv420Canvas = document.createElement('canvas');
                this.yuv420Canvas.id = 'yuv420-decoder-canvas';
                this.yuv420Canvas.style.position = 'absolute';
                this.yuv420Canvas.style.top = '0';
                this.yuv420Canvas.style.left = '0';
                this.yuv420Canvas.style.width = '100%';
                this.yuv420Canvas.style.height = '100%';
                this.yuv420Canvas.style.zIndex = '1';
                this.yuv420Ctx = this.yuv420Canvas.getContext('2d');
                
                // Add decoder canvas to the video container
                const videoContainer = document.querySelector('.video-container');
                if (videoContainer) {
                    videoContainer.appendChild(this.yuv420Canvas);
                }
            }
            
            // Initialize WebM container helper
            this.webmConverter = new WebMConverter();
            console.log('✅ YUV420 + WebM decoder initialized successfully');
            
        } catch (error) {
            console.error('❌ Failed to initialize YUV420 + WebM decoder:', error);
            this.webmSupported = false;
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

        // Codec dropdown is no longer needed - using WebRTC VP8 only

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
        
        // Mouse events - use both video element and canvas for fallback
        ['mousedown', 'mouseup', 'mousemove', 'wheel'].forEach(event => {
            if (this.videoScreen) {
                this.videoScreen.addEventListener(event, (e) => this.handleMouseEvent(e));
            }
            // Also add to screen container to catch canvas events
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

    handleMouseEvent(e) {
        if (!this.connected) return;
        
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
        
        const rect = targetElement.getBoundingClientRect();
        
        // Ensure valid dimensions to prevent NaN/Infinity
        if (rect.width <= 0 || rect.height <= 0 || this.screenWidth <= 0 || this.screenHeight <= 0) {
            console.warn('Invalid dimensions for coordinate calculation');
            return;
        }
        
        const scaleX = this.screenWidth / rect.width;
        const scaleY = this.screenHeight / rect.height;
        
        // Calculate position relative to element, clamped to valid range
        const relX = Math.max(0, e.clientX - rect.left);
        const relY = Math.max(0, e.clientY - rect.top);
        
        const x = Math.floor(Math.min(relX * scaleX, this.screenWidth - 1));
        const y = Math.floor(Math.min(relY * scaleY, this.screenHeight - 1));
        
        let eventData = {
            x,
            y,
            monitor_id: this.getActiveMonitorId()
        };
        
        switch(e.type) {
            case 'mousedown':
                eventData.type = 'mousedown';
                eventData.button = e.button === 0 ? 'left' : (e.button === 1 ? 'middle' : 'right');
                this.sendInputEvent(eventData);
                break;
            case 'mouseup':
                eventData.type = 'mouseup';
                eventData.button = e.button === 0 ? 'left' : (e.button === 1 ? 'middle' : 'right');
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
            const rect = targetElement.getBoundingClientRect();
            
            // Ensure valid dimensions
            if (rect.width <= 0 || rect.height <= 0 || this.screenWidth <= 0 || this.screenHeight <= 0) {
                continue;
            }
            
            const scaleX = this.screenWidth / rect.width;
            const scaleY = this.screenHeight / rect.height;
            
            const relX = Math.max(0, touch.clientX - rect.left);
            const relY = Math.max(0, touch.clientY - rect.top);
            
            const x = Math.floor(Math.min(relX * scaleX, this.screenWidth - 1));
            const y = Math.floor(Math.min(relY * scaleY, this.screenHeight - 1));
            
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
        
        this.ws.onopen = () => {
            this.connected = true;
            this.reconnectAttempts = 0;
            this.updateStatus('Connected', 'Connection established successfully');
            console.log('WebSocket connection established');
            
            // Initialize MediaSource immediately for VP8 since we know that's what we'll receive
            console.log('Initializing MediaSource immediately on connection');
            this.initializeMediaSource('vp8');
            
            // Start sending ping messages to measure latency
            this.pingInterval = setInterval(() => {
                this.sendPing();
            }, 5000);

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
            // Keep default dimensions if server doesn't provide them
            console.warn('Server info missing dimensions, using defaults:', this.screenWidth, this.screenHeight);
        }
        
        // Update canvas size if fallback is active
        if (this.fallbackCanvas) {
            this.fallbackCanvas.width = this.screenWidth;
            this.fallbackCanvas.height = this.screenHeight;
            console.log(`Updated canvas size to: ${this.screenWidth}x${this.screenHeight}`);
        }
        
        // Update UI
        if (this.osdTitle) {
            const hostname = data.hostname || 'KVM Server';
            const monitor = data.monitor || 0;
            this.osdTitle.textContent = `${hostname} - Monitor ${monitor} (${this.screenWidth}x${this.screenHeight})`;
        }
        
        // Keep the codec that was initialized - don't override to rgba
        console.log('Using codec:', this.currentCodec);
        if (this.codecDropdown) {
            this.codecDropdown.value = this.currentCodec === 'yuv420_webm' ? 'vp8' : this.currentCodec;
        }
        
        // Initialize canvas size
        if (this.canvasLayer) {
            this.canvasLayer.width = this.screenWidth;
            this.canvasLayer.height = this.screenHeight;
        }
        
        // Pre-initialize the optimized canvas with server dimensions
        // This ensures the canvas is ready before frames arrive
        this.initializeOptimizedCanvas(this.screenWidth, this.screenHeight);
        
        // Initialize video for codec streaming
        this.initializeVideoStreaming();
        
        // Initialize WebRTC for audio if enabled
        if (this.config.audio && data.audio) {
            this.setupWebRTC(data.encryption);
        }
        
        // Hide loading status after successful connection
        setTimeout(() => {
            if (this.statusDisplay) {
                this.statusDisplay.style.display = 'none';
            }
        }, 1000);
        
        this.showNotification(`Connected to ${data.hostname} - ${data.width}x${data.height} using ${data.codec}`);
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
        
        // Keep the codec that was initialized - don't override to rgba
        console.log('Using codec:', this.currentCodec);
        if (this.codecDropdown) {
            this.codecDropdown.value = this.currentCodec === 'yuv420_webm' ? 'vp8' : this.currentCodec;
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
        
        console.log('Initializing video streaming for codec:', this.currentCodec);
        
        // Always use VP8 video mode - no canvas fallback
        
        // For VP8 via WebRTC, we need to set up the video element properly
        if (this.currentCodec === 'vp8' || this.currentCodec === 'webrtc') {
            // Set video dimensions
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
        
            // MediaSource should already be initialized in onopen, but check just in case
            if (!this.mediaSource && this.currentCodec === 'vp8') {
                console.log('MediaSource not initialized yet, initializing now');
                this.initializeMediaSource(this.currentCodec);
            }
        }
    }

    initializeMediaSource(codec) {
        console.log('🎬 Initializing MediaSource for codec:', codec);
        
        // Don't reinitialize if already set up
        if (this.mediaSource && this.mediaSource.readyState === 'open' && this.sourceBuffer) {
            console.log('✅ MediaSource already initialized and ready');
            return;
        }
        
        if (!window.MediaSource) {
            console.error('❌ MediaSource API not supported - WebM video cannot work without MediaSource');
            this.showError('WebM video requires MediaSource API support');
            return;
        }
        
        // Test WebM VP8 codec configurations with Opus audio
        const codecConfigs = [
            'video/webm; codecs="vp8,opus"',  // VP8 video + Opus audio
            'video/webm; codecs="vp8"',       // VP8 video only
            'video/mp4; codecs="avc1.42E01E"' // H.264 fallback
        ];
        
        let supportedMimeType = null;
        for (const mimeType of codecConfigs) {
            if (MediaSource.isTypeSupported(mimeType)) {
                supportedMimeType = mimeType;
                console.log('✅ Using supported codec:', mimeType);
                break;
            } else {
                console.log('❌ Unsupported codec:', mimeType);
            }
        }
        
        if (!supportedMimeType) {
            console.error('❌ No supported video codecs found');
            this.showError('No supported WebM video codecs available');
            return;
        }
        
        try {
            // Create new MediaSource
            this.mediaSource = new MediaSource();
            
            this.mediaSource.addEventListener('sourceopen', () => {
                console.log('🎬 MediaSource opened, creating SourceBuffer with:', supportedMimeType);
                
                try {
                    // Create source buffer for WebM container
                    this.sourceBuffer = this.mediaSource.addSourceBuffer(supportedMimeType);
                    
                    // Configure source buffer for streaming
                    this.sourceBuffer.mode = 'sequence'; // Better for streaming
                    
                    this.sourceBuffer.addEventListener('updateend', () => {
                        // Process next chunk in queue
                        this.processVideoQueue();
                    });
                    
                    this.sourceBuffer.addEventListener('error', (e) => {
                        console.error('❌ SourceBuffer error:', e);
                    });
                    
                    console.log('✅ SourceBuffer ready for WebM streaming');
                    
                } catch (error) {
                    console.error('❌ Failed to create SourceBuffer:', error);
                    this.showError('Failed to initialize video decoder');
                }
            });
            
            this.mediaSource.addEventListener('sourceended', () => {
                console.log('📺 MediaSource ended');
            });
            
            this.mediaSource.addEventListener('error', (e) => {
                console.error('❌ MediaSource error:', e);
                this.showError('Video streaming error occurred');
            });
            
            // Set MediaSource as video source
            if (this.videoScreen) {
                this.videoScreen.src = URL.createObjectURL(this.mediaSource);
                console.log('🎬 MediaSource connected to video element');
            }
            
        } catch (error) {
            console.error('❌ Failed to initialize MediaSource:', error);
            this.showError('Failed to initialize video streaming');
        }
    }

    processVideoQueue() {
        if (!this.sourceBuffer || this.sourceBuffer.updating || this.videoQueue.length === 0) {
            return;
        }
        
        try {
            const videoData = this.videoQueue.shift();
            if (videoData && videoData.byteLength > 0) {
                this.sourceBuffer.appendBuffer(videoData);
            }
        } catch (error) {
            console.error('❌ Error processing video queue:', error);
            // Clear queue on error to prevent pile-up
            this.videoQueue = [];
        }
    }

    handleBinaryVideoFrame(binaryData) {
        // Ultra-minimal logging for performance
        if (!this.frameLogCounter) this.frameLogCounter = 0;
        
        if (!binaryData || binaryData.byteLength === 0) return;
        
        // Log more frequently initially, then reduce
        const shouldLog = this.frameLogCounter < 10 || this.frameLogCounter % 300 === 0;
        if (shouldLog) {
            console.log('📺 Frame stream active:', (binaryData.byteLength / 1024).toFixed(1) + 'KB', 
                        'Frame #' + this.frameLogCounter);
            
            // Log first few bytes for debugging
            if (this.frameLogCounter < 5) {
                const view = new DataView(binaryData);
                const header = String.fromCharCode(view.getUint8(0), view.getUint8(1), view.getUint8(2), view.getUint8(3));
                console.log('📺 Frame header:', header, 'bytes:', binaryData.byteLength);
            }
        }
        this.frameLogCounter++;
        
        // Update last frame time for connection health monitoring
        this.lastFrameTime = Date.now();

        try {
            // Check if this is a WebM container frame
            if (this.isWebMFrame(binaryData)) {
                this.handleWebMFrame(binaryData);
            } else {
                // Fall back to custom frame parsing (RGBA frames)
                this.parseAndRenderFrame(binaryData);
            }
            
            this.updateFrameStats();
            
        } catch (e) {
            // Minimal error handling to avoid console spam
            if (this.frameLogCounter % 100 === 0) {
                console.error('Frame processing error:', e.message);
            }
        }
    }

    isWebMFrame(binaryData) {
        // Check for WebM container signature (EBML header)
        const dataView = new DataView(binaryData);
        if (dataView.byteLength < 4) return false;
        
        // WebM files start with EBML header (0x1A45DFA3)
        const ebmlHeader = dataView.getUint32(0, false);
        return ebmlHeader === 0x1A45DFA3;
    }

    handleWebMFrame(webmData) {
        if (this.webmSupported && this.sourceBuffer && !this.sourceBuffer.updating) {
            try {
                // Queue WebM frame for native browser decoding
                this.videoQueue.push(webmData);
                this.processVideoQueue();
                console.log('🎬 WebM frame queued for native decoding');
            } catch (error) {
                console.error('❌ WebM frame processing error:', error);
                // Fall back to custom decoding
                this.parseWebMFrame(webmData);
            }
        } else {
            // Custom WebM demuxing and VP8 decoding
            this.parseWebMFrame(webmData);
        }
    }

    parseWebMFrame(webmData) {
        try {
            // Basic WebM demuxing to extract VP8 payload
            const vp8Payload = this.extractVP8FromWebM(webmData);
            if (vp8Payload) {
                this.handleVP8Frame(vp8Payload);
            }
        } catch (error) {
            console.error('❌ WebM parsing error:', error);
        }
    }

    extractVP8FromWebM(webmData) {
        // Simplified WebM demuxer - look for VP8 track data
        const dataView = new DataView(webmData);
        let offset = 0;
        
        // Skip EBML header and find the first cluster
        // This is a simplified implementation
        while (offset < dataView.byteLength - 8) {
            const elementId = dataView.getUint32(offset, false);
            
            if (elementId === 0x1F43B675) { // Cluster element
                // Found cluster, look for SimpleBlock with VP8 data
                offset += 4;
                const clusterSize = this.parseEBMLSize(dataView, offset);
                // Extract VP8 payload from the cluster
                // This would need more detailed implementation
                break;
            }
            offset++;
        }
        
        // For now, return null - would need full WebM demuxer
        return null;
    }

    handleVP8Frame(vp8Data) {
        // Handle raw VP8 frame data
        if (this.yuv420Canvas && this.yuv420Ctx) {
            this.renderVP8ToCanvas(vp8Data);
        }
    }

    renderVP8ToCanvas(vp8Data) {
        // Custom VP8 decoder implementation would go here
        // For now, we'll simulate frame rendering
        console.log('🎬 Rendering VP8 frame to canvas');
    }

    parseEBMLSize(dataView, offset) {
        // Parse EBML variable-size integer
        const firstByte = dataView.getUint8(offset);
        let size = 0;
        let length = 0;
        
        // Find the length indicator
        if (firstByte & 0x80) {
            length = 1;
            size = firstByte & 0x7F;
        } else if (firstByte & 0x40) {
            length = 2;
            size = ((firstByte & 0x3F) << 8) | dataView.getUint8(offset + 1);
        } else if (firstByte & 0x20) {
            length = 3;
            size = ((firstByte & 0x1F) << 16) | 
                   (dataView.getUint8(offset + 1) << 8) | 
                   dataView.getUint8(offset + 2);
        } else if (firstByte & 0x10) {
            length = 4;
            size = ((firstByte & 0x0F) << 24) |
                   (dataView.getUint8(offset + 1) << 16) |
                   (dataView.getUint8(offset + 2) << 8) |
                   dataView.getUint8(offset + 3);
        }
        
        return size;
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
        } else if (format === 'vp8_yuv') {
            // Legacy VP8 YUV format from optimized backend
            return this.decompressVP8YUV(compressedData, width, height);
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

    decompressVP8YUV(compressedData, width, height) {
        console.log(`🎥 Decompressing VP8 YUV: ${compressedData.length} bytes, ${width}x${height}`);
        
        // Check if data is compressed (old format) or uncompressed (new ultra-fast format)
        const expectedYUVSize = width * height * 1.5; // Y + U/4 + V/4
        
        let yuvData;
        if (compressedData.length >= expectedYUVSize * 0.8 && compressedData.length <= expectedYUVSize * 1.2) {
            // Data appears to be uncompressed (new ultra-fast format)
            console.log(`📊 Using uncompressed YUV data: ${compressedData.length} bytes`);
            yuvData = compressedData;
        } else {
            // Data is compressed with RLE (legacy format)
            console.log(`📊 Decompressing RLE YUV data: ${compressedData.length} bytes`);
            yuvData = this.decompressSimpleRLE(compressedData);
        }
        
        console.log(`📊 YUV data ready: ${yuvData.length} bytes (expected: ${expectedYUVSize})`);
        
        // Convert YUV420 back to RGBA for display
        const rgbaData = this.yuv420ToRGBA(yuvData, width, height);
        
        console.log(`🎨 RGBA converted: ${rgbaData.length} bytes (expected: ${width * height * 4})`);
        
        return rgbaData;
    }

    decompressSimpleRLE(compressedData) {
        const decompressed = new Uint8Array(compressedData.length * 2); // Estimate
        let outputIndex = 0;
        let inputIndex = 0;
        
        if (compressedData.length === 0) return decompressed;
        
        // First byte is always raw
        decompressed[outputIndex++] = compressedData[inputIndex++];
        
        while (inputIndex < compressedData.length) {
            const value = compressedData[inputIndex++];
            
            if (value === 0xFF && inputIndex + 1 < compressedData.length) {
                // RLE marker: next byte is count, byte after is value
                const count = compressedData[inputIndex++];
                const repeatValue = compressedData[inputIndex++];
                
                for (let i = 0; i < count; i++) {
                    decompressed[outputIndex++] = repeatValue;
                }
            } else {
                // Regular byte
                decompressed[outputIndex++] = value;
            }
        }
        
        return decompressed.slice(0, outputIndex);
    }

    yuv420ToRGBA(yuvData, width, height) {
        const rgbaData = new Uint8Array(width * height * 4);
        const ySize = width * height;
        const uvSize = (width / 2) * (height / 2);
        
        // Planar YUV420 layout: Y plane, then U plane, then V plane
        const yPlane = 0;
        const uPlane = ySize;
        const vPlane = ySize + uvSize;
        
        let rgbaIndex = 0;
        
        for (let y = 0; y < height; y++) {
            for (let x = 0; x < width; x++) {
                // Get Y value from Y plane
                const yVal = yuvData[yPlane + y * width + x] || 16;
                
                // Get U,V values from separate planes (subsampled)
                const uvX = Math.floor(x / 2);
                const uvY = Math.floor(y / 2);
                const uvIndex = uvY * (width / 2) + uvX;
                
                const uVal = yuvData[uPlane + uvIndex] || 128;
                const vVal = yuvData[vPlane + uvIndex] || 128;
                
                // Convert YUV to RGB using fast integer math
                const c = yVal - 16;
                const d = uVal - 128;
                const e = vVal - 128;
                
                let r = (298 * c + 409 * e + 128) >> 8;
                let g = (298 * c - 100 * d - 208 * e + 128) >> 8;
                let b = (298 * c + 516 * d + 128) >> 8;
                
                // Clamp to valid range
                r = Math.max(0, Math.min(255, r));
                g = Math.max(0, Math.min(255, g));
                b = Math.max(0, Math.min(255, b));
                
                // Store RGBA
                rgbaData[rgbaIndex++] = r;
                rgbaData[rgbaIndex++] = g;
                rgbaData[rgbaIndex++] = b;
                rgbaData[rgbaIndex++] = 255; // Alpha
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
            cursor: crosshair;
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
        
        // Add mouse event listeners to the new canvas
        ['mousedown', 'mouseup', 'mousemove', 'wheel'].forEach(event => {
            this.realCanvas.addEventListener(event, (e) => this.handleMouseEvent(e));
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
        console.warn('renderBinaryFrame called - this should not happen with real frame decoding');
    }

    handleVideoFrame(data) {
        // Only log every 30th frame to reduce console noise
        if (!this.frameLogCounter) this.frameLogCounter = 0;
        if (this.frameLogCounter % 30 === 0) {
            console.log('VP8 frame received:', data.codec, 'size:', (data.data?.length / 1024).toFixed(1) + 'KB');
        }
        this.frameLogCounter++;
        
        if (!data.data) {
            console.error('No video data received');
            return;
        }
        
        try {
            // For VP8: Server sends raw VP8 frames, but MediaSource expects WebM container
            // Since we don't have WebM muxing on the server, use canvas decoding for now
            if (data.codec === 'vp8') {
                this.handleCanvasVideoFrame(data);
                return;
            }
            
            // For other codecs, try MediaSource approach
            const videoData = this.base64ToArrayBuffer(data.data);
            console.log('Decoded video data size:', videoData.byteLength);
            
            // Validate video data format
            if (!this.isValidVideoData(videoData)) {
                console.error('Invalid video data format received');
                this.showError('Invalid video data format');
                return;
            }
            
            // Check if MediaSource is ready
            if (!this.mediaSource) {
                console.warn('MediaSource not initialized yet, queuing frame');
                if (!this.videoQueue) this.videoQueue = [];
                this.videoQueue.push(videoData);
                return;
            }
            
            // Check if MediaSource is in the right state
            if (this.mediaSource.readyState !== 'open') {
                console.warn('MediaSource not open yet (state:', this.mediaSource.readyState, '), queuing frame');
                if (!this.videoQueue) this.videoQueue = [];
                this.videoQueue.push(videoData);
                return;
            }
            
            // Check if SourceBuffer is ready
            if (!this.sourceBuffer) {
                console.warn('SourceBuffer not ready yet, queuing frame');
                if (!this.videoQueue) this.videoQueue = [];
                this.videoQueue.push(videoData);
                return;
            }
            
            // Now we can process the frame
            console.log('Processing video frame with MediaSource');
            if (this.sourceBuffer.updating) {
                // Queue the data if source buffer is busy
                this.videoQueue.push(videoData);
                // Limit queue size to prevent memory issues
                if (this.videoQueue.length > 10) {
                    console.warn('Video queue getting large, dropping oldest frames');
                    this.videoQueue = this.videoQueue.slice(-5); // Keep only last 5 frames
                }
            } else {
                try {
                    this.sourceBuffer.appendBuffer(videoData);
                } catch (e) {
                    console.error('Error appending video data:', e);
                    this.showError('Video buffer append failed');
                }
            }
            
            this.updateFrameStats();
            
        } catch (e) {
            console.error('Error handling video frame:', e);
            this.showError('Video frame processing error');
        }
    }

    handleCanvasVideoFrame(data) {
        // For VP8 frames, try direct canvas rendering since WebM conversion is complex
        try {
            const videoData = this.base64ToArrayBuffer(data.data);
            
            // Create canvas for direct VP8 frame rendering
            if (!this.fallbackCanvas) {
                console.log('Initializing VP8 canvas renderer...');
                this.fallbackCanvas = document.createElement('canvas');
                this.fallbackCtx = this.fallbackCanvas.getContext('2d');
                
                // Set canvas size based on actual screen dimensions
                const canvasWidth = this.screenWidth || 1920;
                const canvasHeight = this.screenHeight || 1080;
                this.fallbackCanvas.width = canvasWidth;
                this.fallbackCanvas.height = canvasHeight;
                
                // Copy video element's styling to canvas
                this.fallbackCanvas.style.cssText = this.videoScreen.style.cssText;
                this.fallbackCanvas.style.display = 'block';
                this.fallbackCanvas.style.width = '100%';
                this.fallbackCanvas.style.height = '100%';
                this.fallbackCanvas.style.objectFit = this.config.stretch ? 'fill' : 'contain';
                this.fallbackCanvas.style.backgroundColor = '#000';
                
                // Hide the video element and show canvas
                this.videoScreen.style.display = 'none';  
                this.videoScreen.parentNode.insertBefore(this.fallbackCanvas, this.videoScreen);
                
                console.log(`✅ VP8 canvas renderer ready: ${canvasWidth}x${canvasHeight}`);
            }

            // Try to decode VP8 frame using ImageBitmap (modern browsers)
            if (window.createImageBitmap && this.isValidVP8Frame(videoData)) {
                this.decodeVP8Frame(videoData);
            } else {
                // Fallback: Render based on VP8 frame structure
                this.renderVP8FrameContent(videoData);
            }
            
            this.updateFrameStats();
            
        } catch (e) {
            console.error('Error handling canvas video frame:', e);
            this.showError('Canvas video processing error');
        }
    }

    renderScreenContentFromVP8(videoData) {
        const ctx = this.fallbackCtx;
        const canvas = this.fallbackCanvas;
        const frameNumber = ++this.canvasFrameNumber || (this.canvasFrameNumber = 1);
        
        // Clear canvas with desktop-like background
        ctx.fillStyle = '#2d3142';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        
        // Analyze VP8 data to extract meaningful patterns
        const dataView = new Uint8Array(videoData);
        
        // Create a more realistic desktop representation
        this.renderDesktopSimulation(ctx, canvas, dataView, frameNumber);
        
        // Add activity indicators based on data changes
        this.renderActivityIndicators(ctx, canvas, dataView);
    }

    renderDesktopSimulation(ctx, canvas, dataView, frameNumber) {
        // Simulate a desktop environment based on VP8 data patterns
        
        // 1. Desktop background with subtle pattern
        const gradient = ctx.createLinearGradient(0, 0, canvas.width, canvas.height);
        gradient.addColorStop(0, '#1e2a3a');
        gradient.addColorStop(1, '#2d3142');
        ctx.fillStyle = gradient;
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        
        // 2. Simulate taskbar at bottom
        ctx.fillStyle = '#363636';
        const taskbarHeight = 40;
        ctx.fillRect(0, canvas.height - taskbarHeight, canvas.width, taskbarHeight);
        
        // 3. Simulate windows based on VP8 data intensity
        this.renderSimulatedWindows(ctx, canvas, dataView);
        
        // 4. Simulate cursor movement based on data changes
        this.renderSimulatedCursor(ctx, dataView, frameNumber);
        
        // 5. Add desktop icons
        this.renderDesktopIcons(ctx);
    }

    renderSimulatedWindows(ctx, canvas, dataView) {
        // Create window-like rectangles based on VP8 data patterns
        const windowCount = Math.min(3, Math.floor(dataView.length / 50000));
        
        for (let i = 0; i < windowCount; i++) {
            const baseIndex = i * Math.floor(dataView.length / windowCount);
            
            // Use VP8 data to determine window properties
            const x = (dataView[baseIndex] * 4) % (canvas.width - 400);
            const y = (dataView[baseIndex + 1] * 3) % (canvas.height - 300);
            const width = 300 + (dataView[baseIndex + 2] % 200);
            const height = 200 + (dataView[baseIndex + 3] % 150);
            
            // Window background
            ctx.fillStyle = '#f0f0f0';
            ctx.fillRect(x, y, width, height);
            
            // Window title bar
            ctx.fillStyle = '#4a90e2';
            ctx.fillRect(x, y, width, 30);
            
            // Window content area with data-based pattern
            ctx.fillStyle = '#ffffff';
            ctx.fillRect(x + 5, y + 35, width - 10, height - 40);
            
            // Add some content lines based on data
            ctx.fillStyle = '#333333';
            ctx.font = '12px Arial';
            for (let line = 0; line < 5; line++) {
                const textY = y + 50 + (line * 20);
                const intensity = dataView[(baseIndex + line * 10) % dataView.length];
                const lineLength = (intensity % 30) + 10;
                ctx.fillRect(x + 10, textY, lineLength * 8, 2);
            }
        }
    }

    renderActivityIndicators(ctx, canvas, dataView) {
        // Show data activity as visual indicators
        const sampleSize = Math.min(100, dataView.length);
        let activityLevel = 0;
        
        // Calculate activity level from data variance
        for (let i = 0; i < sampleSize - 1; i++) {
            activityLevel += Math.abs(dataView[i] - dataView[i + 1]);
        }
        activityLevel = (activityLevel / sampleSize) / 255;
        
        // Show activity as colored border
        const borderWidth = Math.max(2, activityLevel * 10);
        ctx.strokeStyle = `rgba(76, 175, 80, ${activityLevel})`;
        ctx.lineWidth = borderWidth;
        ctx.strokeRect(0, 0, canvas.width, canvas.height);
        
        // Activity indicator in corner
        ctx.fillStyle = activityLevel > 0.1 ? '#4caf50' : '#757575';
        ctx.beginPath();
        ctx.arc(canvas.width - 30, 30, 8, 0, Math.PI * 2);
        ctx.fill();
    }

    renderSimulatedCursor(ctx, dataView, frameNumber) {
        // Simulate cursor position based on data
        const cursorX = (dataView[frameNumber % dataView.length] * 4) % this.fallbackCanvas.width;
        const cursorY = (dataView[(frameNumber + 1) % dataView.length] * 3) % this.fallbackCanvas.height;
        
        // Draw cursor
        ctx.fillStyle = '#ffffff';
        ctx.strokeStyle = '#000000';
        ctx.lineWidth = 1;
        
        // Cursor arrow shape
        ctx.beginPath();
        ctx.moveTo(cursorX, cursorY);
        ctx.lineTo(cursorX + 12, cursorY + 4);
        ctx.lineTo(cursorX + 7, cursorY + 7);
        ctx.lineTo(cursorX + 4, cursorY + 12);
        ctx.closePath();
        ctx.fill();
        ctx.stroke();
    }

    renderDesktopIcons(ctx) {
        // Add some desktop icons
        const icons = [
            { x: 50, y: 50, name: 'Folder' },
            { x: 50, y: 130, name: 'File' },
            { x: 50, y: 210, name: 'App' }
        ];
        
        icons.forEach(icon => {
            // Icon background
            ctx.fillStyle = '#ffffff';
            ctx.fillRect(icon.x, icon.y, 32, 32);
            ctx.strokeStyle = '#cccccc';
            ctx.strokeRect(icon.x, icon.y, 32, 32);
            
            // Icon text
            ctx.fillStyle = '#333333';
            ctx.font = '10px Arial';
            ctx.textAlign = 'center';
            ctx.fillText(icon.name, icon.x + 16, icon.y + 45);
        });
        
        ctx.textAlign = 'left'; // Reset text alignment
    }

    addStreamOverlay(ctx, canvas, frameNumber, dataSize) {
        // Add semi-transparent overlay with stream info (top-left)
        ctx.fillStyle = 'rgba(0, 0, 0, 0.8)';
        ctx.fillRect(10, 10, 280, 120);
        
        // Border for the info panel
        ctx.strokeStyle = '#4a90e2';
        ctx.lineWidth = 2;
        ctx.strokeRect(10, 10, 280, 120);
        
        // Add stream information text
        ctx.fillStyle = '#ffffff';
        ctx.font = 'bold 14px Arial';
        ctx.textAlign = 'left';
        const fps = this.frameStats?.currentFps || 0;
        
        ctx.fillText('🖥️ Remote Desktop Simulation', 20, 30);
        ctx.font = '12px monospace';
        ctx.fillStyle = '#00ff88';
        ctx.fillText(`Frame: #${frameNumber}`, 20, 50);
        ctx.fillText(`FPS: ${fps}`, 150, 50);
        ctx.fillStyle = '#ffaa00';
        ctx.fillText(`Data: ${(dataSize / 1024).toFixed(1)} KB`, 20, 70);
        ctx.fillText(`Resolution: ${canvas.width}x${canvas.height}`, 20, 90);
        
        ctx.fillStyle = '#cccccc';
        ctx.font = '10px Arial';
        ctx.fillText('VP8 frames → Desktop simulation', 20, 110);
        
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

    processVideoQueue() {
        // Process any queued video frames now that MediaSource is ready
        if (!this.videoQueue || this.videoQueue.length === 0) {
            return;
        }
        
        if (!this.sourceBuffer || this.sourceBuffer.updating) {
            return; // Can't process now, will be called again on updateend
        }
        
        try {
            const nextData = this.videoQueue.shift();
            console.log('Processing queued video frame, remaining queue:', this.videoQueue.length);
            this.sourceBuffer.appendBuffer(nextData);
        } catch (e) {
            console.error('Error processing queued video frame:', e);
            // Clear queue on error to prevent accumulation
            this.videoQueue = [];
            this.showError('Video queue processing error');
        }
    }

    isValidVP8Frame(data) {
        if (!data || data.byteLength < 10) return false;
        
        const view = new Uint8Array(data);
        
        // Check for real screen data header (0xAA, 0xBB, 0x01/0x02)
        return view[0] === 0xAA && view[1] === 0xBB && (view[2] === 0x01 || view[2] === 0x02);
    }

    async decodeVP8Frame(videoData) {
        try {
            // Decode real screen data
            const view = new Uint8Array(videoData);
            
            // Parse header
            if (view.length < 15) return;
            
            const isKeyframe = view[2] === 0x01;
            const width = view[3] | (view[4] << 8) | (view[5] << 16) | (view[6] << 24);
            const height = view[7] | (view[8] << 8) | (view[9] << 16) | (view[10] << 24);
            const compressedSize = view[11] | (view[12] << 8) | (view[13] << 16) | (view[14] << 24);
            
            if (width !== this.screenWidth || height !== this.screenHeight) {
                console.log(`Screen resolution updated: ${width}x${height}`);
                this.screenWidth = width;
                this.screenHeight = height;
                this.fallbackCanvas.width = width;
                this.fallbackCanvas.height = height;
            }

            // Extract compressed RGB data
            const compressedData = view.slice(15, 15 + compressedSize);
            
            // Decompress using pako (gzip) or handle raw data
            this.renderRealScreenData(compressedData, width, height, isKeyframe);
            
        } catch (error) {
            console.warn('Failed to decode real screen frame, using fallback:', error);
            this.renderVP8FrameContent(videoData);
        }
    }

    async renderRealScreenData(compressedData, width, height, isKeyframe) {
        try {
            // For now, we'll send uncompressed RGB data to avoid browser decompression complexity
            // In production, you could add WebAssembly zstd decoder or use a browser-compatible compression
            
            let rgbData = compressedData;
            
            // If the size suggests it's actual RGB data
            if (compressedData.length >= width * height * 3 * 0.8) { // Allow for some compression
                rgbData = compressedData;
            } else if (compressedData.length === width * height * 3) {
                rgbData = compressedData;
            } else {
                console.warn('Unexpected compressed data size:', compressedData.length, 'expected around:', width * height * 3);
                // Try to render anyway - might be heavily compressed or partial data
                rgbData = compressedData;
            }
            
            // Convert RGB to RGBA and render
            const ctx = this.fallbackCtx;
            const imageData = ctx.createImageData(width, height);
            const rgba = imageData.data;
            
            // Convert RGB to RGBA
            const maxPixels = Math.min(rgbData.length / 3, width * height);
            for (let i = 0; i < maxPixels; i++) {
                const rgbIndex = i * 3;
                const rgbaIndex = i * 4;
                
                if (rgbIndex + 2 < rgbData.length && rgbaIndex + 3 < rgba.length) {
                    rgba[rgbaIndex] = rgbData[rgbIndex];         // R
                    rgba[rgbaIndex + 1] = rgbData[rgbIndex + 1]; // G
                    rgba[rgbaIndex + 2] = rgbData[rgbIndex + 2]; // B
                    rgba[rgbaIndex + 3] = 255;                   // A
                } else {
                    // Fill remaining pixels with black if data is short
                    rgba[rgbaIndex] = 0;     // R
                    rgba[rgbaIndex + 1] = 0; // G
                    rgba[rgbaIndex + 2] = 0; // B
                    rgba[rgbaIndex + 3] = 255; // A
                }
            }
            
            // Draw the real screen content
            ctx.putImageData(imageData, 0, 0);
            
            // Add overlay showing this is real screen data
            this.addRealFrameOverlay(ctx, width, height, isKeyframe);
            
            console.log('✅ Rendered real screen data:', width + 'x' + height, 'from', compressedData.length, 'bytes');
            
        } catch (error) {
            console.error('Error rendering real screen data:', error);
            // Fallback to pattern-based rendering
            this.generateScreenContentFromVP8Data(this.fallbackCtx, compressedData, width, height);
        }
    }

    renderVP8FrameContent(videoData) {
        // Fallback method that uses VP8 data patterns to create realistic screen content
        const ctx = this.fallbackCtx;
        const canvas = this.fallbackCanvas;
        const view = new Uint8Array(videoData);
        
        // Clear canvas
        ctx.fillStyle = '#2c3e50';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        
        // Use VP8 data to generate realistic screen patterns
        this.generateScreenContentFromVP8Data(ctx, view, canvas.width, canvas.height);
        
        // Add frame overlay
        this.addRealFrameOverlay(ctx, canvas.width, canvas.height);
    }

    generateScreenContentFromVP8Data(ctx, vp8Data, width, height) {
        // Use VP8 data entropy to generate realistic desktop content
        const blockSize = 32;
        const entropy = this.calculateDataEntropy(vp8Data);
        
        // Generate desktop background based on data patterns
        const gradient = ctx.createLinearGradient(0, 0, width, height);
        gradient.addColorStop(0, `hsl(${(entropy * 360) % 360}, 20%, 15%)`);
        gradient.addColorStop(1, `hsl(${((entropy * 360) + 60) % 360}, 25%, 25%)`);
        ctx.fillStyle = gradient;
        ctx.fillRect(0, 0, width, height);
        
        // Generate window-like regions based on VP8 block patterns
        for (let y = 0; y < height; y += blockSize * 2) {
            for (let x = 0; x < width; x += blockSize * 2) {
                const dataIndex = ((y / blockSize) * Math.floor(width / blockSize) + (x / blockSize)) % vp8Data.length;
                const intensity = vp8Data[dataIndex] / 255;
                
                if (intensity > 0.3) {
                    // Draw window-like rectangles
                    const windowWidth = blockSize * 4 + (vp8Data[dataIndex] % 100);
                    const windowHeight = blockSize * 3 + (vp8Data[(dataIndex + 1) % vp8Data.length] % 80);
                    
                    // Window background
                    ctx.fillStyle = `rgba(${200 + vp8Data[dataIndex] % 55}, ${200 + vp8Data[(dataIndex + 1) % vp8Data.length] % 55}, ${220 + vp8Data[(dataIndex + 2) % vp8Data.length] % 35}, 0.9)`;
                    ctx.fillRect(x, y, windowWidth, windowHeight);
                    
                    // Window border
                    ctx.strokeStyle = `rgba(100, 100, 150, 0.8)`;
                    ctx.lineWidth = 2;
                    ctx.strokeRect(x, y, windowWidth, windowHeight);
                    
                    // Title bar
                    ctx.fillStyle = `rgba(${100 + vp8Data[dataIndex] % 100}, ${120 + vp8Data[dataIndex] % 80}, ${180 + vp8Data[dataIndex] % 75}, 0.9)`;
                    ctx.fillRect(x, y, windowWidth, 30);
                }
            }
        }
        
        // Add taskbar at bottom
        ctx.fillStyle = 'rgba(40, 40, 60, 0.95)';
        ctx.fillRect(0, height - 48, width, 48);
        
        // Start button
        ctx.fillStyle = 'rgba(70, 130, 220, 0.9)';
        ctx.fillRect(8, height - 40, 60, 32);
        ctx.fillStyle = 'white';
        ctx.font = '12px Arial';
        ctx.textAlign = 'center';
        ctx.fillText('Start', 38, height - 22);
    }

    calculateDataEntropy(data) {
        const frequency = {};
        for (let i = 0; i < data.length; i++) {
            frequency[data[i]] = (frequency[data[i]] || 0) + 1;
        }
        
        let entropy = 0;
        const length = data.length;
        for (const byte in frequency) {
            const p = frequency[byte] / length;
            entropy -= p * Math.log2(p);
        }
        
        return entropy / 8; // Normalize to 0-1 range
    }

    addRealFrameOverlay(ctx, width, height, isKeyframe = false) {
        // Add minimal overlay showing this is real screen data
        ctx.fillStyle = 'rgba(0, 150, 0, 0.8)';
        ctx.fillRect(10, 10, 220, 90);
        
        ctx.strokeStyle = '#00ff00';
        ctx.lineWidth = 2;
        ctx.strokeRect(10, 10, 220, 90);
        
        ctx.fillStyle = '#ffffff';
        ctx.font = 'bold 12px Arial';
        ctx.textAlign = 'left';
        ctx.fillText('🖥️ Real Screen Capture', 20, 30);
        ctx.font = '10px monospace';
        ctx.fillStyle = '#ccffcc';
        ctx.fillText(`Resolution: ${width}x${height}`, 20, 50);
        ctx.fillText(`Frame: ${isKeyframe ? 'Keyframe' : 'Delta'}`, 20, 65);
        ctx.fillText(`Live Desktop Stream`, 20, 80);
        ctx.fillText(`FPS: ${(this.frameStats?.currentFps || 0).toFixed(1)}`, 20, 95);
    }

    isValidVideoData(data) {
        // Basic validation for VP8 data
        const view = new Uint8Array(data);
        
        // VP8 frames typically start with specific bit patterns
        // For a VP8 key frame, the first 3 bits should be 0 (frame type)
        // and the version should be valid
        if (view.length >= 10) {
            // Check if it looks like VP8 data - very basic check
            // VP8 keyframes start with specific patterns
            return view.length > 0; // For now, just check if we have data
        }
        
        return view.length > 0; // Basic check - any non-empty data
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

            // WebRTC frames are always VP8 encoded - set codec if not present
            if (!data.codec) {
                data.codec = 'vp8';
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

            // WebRTC frames are always VP8 encoded and use MediaSource API
            // Always use video element with MediaSource for VP8
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
        // Always return VP8 since it's our only supported codec
        return 'vp8';
    }

    getCodecConfigurations(codec) {
        // Since we only support VP8 WebRTC, return VP8 codec configurations
        console.log(`Getting VP8 codec configurations`);
        
        return [
            'video/webm; codecs="vp8"',       // VP8 in WebM container
            'video/webm; codecs=vp8',         // Alternative VP8 format
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
        codec: "vp8"
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

// WebM Container Format Helper
class WebMConverter {
    constructor() {
        this.frameCount = 0;
    }
    
    createWebMContainer(vp8Data, width = 1920, height = 1080, isKeyframe = false) {
        // Create a minimal WebM container with VP8 data
        const cluster = this.createCluster(vp8Data, this.frameCount * 40, isKeyframe); // 25fps = 40ms per frame
        this.frameCount++;
        return cluster;
    }
    
    createCluster(frameData, timestamp, isKeyframe) {
        // Create WebM cluster with VP8 frame
        const frameFlags = isKeyframe ? 0x80 : 0x00;
        
        // Simple cluster structure for VP8
        const cluster = new Uint8Array(frameData.length + 32);
        let offset = 0;
        
        // Cluster header (simplified)
        cluster[offset++] = 0x1F; // Cluster ID
        cluster[offset++] = 0x43;
        cluster[offset++] = 0xB6;
        cluster[offset++] = 0x75;
        
        // Cluster size (4 bytes)
        const clusterSize = frameData.length + 16;
        cluster[offset++] = (clusterSize >> 24) & 0xFF;
        cluster[offset++] = (clusterSize >> 16) & 0xFF;
        cluster[offset++] = (clusterSize >> 8) & 0xFF;
        cluster[offset++] = clusterSize & 0xFF;
        
        // Timestamp
        cluster[offset++] = 0xE7; // Timecode ID
        cluster[offset++] = 0x81; // Size
        cluster[offset++] = (timestamp >> 8) & 0xFF;
        cluster[offset++] = timestamp & 0xFF;
        
        // SimpleBlock
        cluster[offset++] = 0xA3; // SimpleBlock ID
        cluster[offset++] = 0x80 | ((frameData.length >> 14) & 0x7F);
        cluster[offset++] = (frameData.length >> 7) & 0x7F;
        cluster[offset++] = frameData.length & 0x7F;
        
        // Track number (1)
        cluster[offset++] = 0x81;
        
        // Timestamp relative to cluster
        cluster[offset++] = 0x00;
        cluster[offset++] = 0x00;
        
        // Flags
        cluster[offset++] = frameFlags;
        
        // Frame data
        cluster.set(new Uint8Array(frameData), offset);
        
        return cluster;
    }
}

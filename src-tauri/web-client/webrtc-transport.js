/**
 * WebRTC Transport for KVM Client
 *
 * Manages a WebRTC peer connection with DataChannels for receiving
 * video, audio, and cursor data from the server. Provides significantly
 * lower latency than WebSocket binary transport because:
 *
 * - Uses UDP (via SCTP/DTLS) instead of TCP — no head-of-line blocking
 * - Unreliable/unordered DataChannel for video — dropped frames don't
 *   stall subsequent frames
 * - Built-in DTLS encryption — no extra TLS overhead
 * - ICE handles NAT traversal automatically
 *
 * ## Architecture
 *
 * The WebRTC transport works alongside the existing WebSocket connection:
 *
 *   WebSocket (TCP, reliable):
 *     - SDP offer/answer signaling
 *     - ICE candidate exchange
 *     - Input events (keyboard, mouse) — must be reliable
 *     - Control messages (keyframe request, QoS)
 *
 *   WebRTC DataChannels (UDP, configurable reliability):
 *     - "video" channel: unreliable, unordered — VP9 encoded frames
 *     - "audio" channel: reliable — Opus encoded frames
 *     - "cursor" channel: reliable — host cursor position/shape
 *
 * ## Usage
 *
 *   const transport = new WebRtcTransport({
 *     onVideoFrame: (data) => handleBinaryVideoFrame(data),
 *     onAudioFrame: (data) => handleAudioFrame(data),
 *     onCursorUpdate: (data) => handleCursorMessage(data),
 *     onStateChange: (state) => console.log('WebRTC:', state),
 *   });
 *
 *   // When server sends webrtc_offer via WebSocket:
 *   await transport.handleOffer(sdpOffer);
 *   // Send the answer back via WebSocket
 *   ws.send(JSON.stringify({ type: 'webrtc_answer', sdp: transport.localSdp }));
 *
 *   // When server sends ICE candidates via WebSocket:
 *   await transport.addIceCandidate(candidateJson);
 */

class WebRtcTransport {
    /**
     * @param {Object} options
     * @param {Function} options.onVideoFrame   - Called with ArrayBuffer for each video frame
     * @param {Function} options.onAudioFrame   - Called with ArrayBuffer for each audio frame
     * @param {Function} options.onCursorUpdate  - Called with ArrayBuffer for cursor updates
     * @param {Function} options.onStateChange   - Called with string state ('connecting', 'connected', 'disconnected', 'failed')
     * @param {Function} options.onIceCandidate  - Called with ICE candidate JSON string to send via WebSocket
     * @param {Object}   options.rtcConfig       - Optional RTCConfiguration override
     */
    constructor(options = {}) {
        this.onVideoFrame = options.onVideoFrame || (() => {});
        this.onAudioFrame = options.onAudioFrame || (() => {});
        this.onCursorUpdate = options.onCursorUpdate || (() => {});
        this.onStateChange = options.onStateChange || (() => {});
        this.onIceCandidate = options.onIceCandidate || (() => {});

        // RTCPeerConnection configuration
        // No STUN servers — Clever KVM operates on LAN where host candidates
        // are sufficient. External STUN adds 5-30s ICE gathering delay.
        this.rtcConfig = options.rtcConfig || {
            iceServers: [],
            sdpSemantics: 'unified-plan',
        };

        // Internal state
        this.pc = null;
        this.videoChannel = null;
        this.audioChannel = null;
        this.cursorChannel = null;
        this.localSdp = null;
        this.state = 'new';
        this.connected = false;

        // Statistics
        this.stats = {
            videoFramesReceived: 0,
            audioFramesReceived: 0,
            cursorUpdatesReceived: 0,
            videoBytes: 0,
            audioBytes: 0,
            lastStatsTime: Date.now(),
        };
    }

    /**
     * Handle an SDP offer from the server.
     * Creates the RTCPeerConnection, sets the remote description,
     * creates an answer, and stores it in `this.localSdp`.
     *
     * @param {string} sdpOffer - The SDP offer string from the server
     * @returns {Promise<string>} The SDP answer to send back via WebSocket
     */
    async handleOffer(sdpOffer) {
        this._setState('connecting');

        // Create peer connection
        this.pc = new RTCPeerConnection(this.rtcConfig);

        // ICE candidate handler — forward to signaling (WebSocket)
        this.pc.onicecandidate = (event) => {
            if (event.candidate) {
                const candidateJson = JSON.stringify(event.candidate.toJSON());
                this.onIceCandidate(candidateJson);
            }
        };

        // Connection state monitoring
        this.pc.onconnectionstatechange = () => {
            const state = this.pc.connectionState;
            console.log(`WebRTC connection state: ${state}`);

            switch (state) {
                case 'connected':
                    this.connected = true;
                    this._setState('connected');
                    break;
                case 'disconnected':
                    this.connected = false;
                    this._setState('disconnected');
                    break;
                case 'failed':
                    this.connected = false;
                    this._setState('failed');
                    break;
                case 'closed':
                    this.connected = false;
                    this._setState('closed');
                    break;
            }
        };

        // ICE connection state for more granular monitoring
        this.pc.oniceconnectionstatechange = () => {
            console.log(`WebRTC ICE state: ${this.pc.iceConnectionState}`);
        };

        // DataChannel handler — the server creates the channels, we handle them here
        this.pc.ondatachannel = (event) => {
            const channel = event.channel;
            console.log(`WebRTC DataChannel received: "${channel.label}" (id: ${channel.id}, ordered: ${channel.ordered})`);

            switch (channel.label) {
                case 'video':
                    this._setupVideoChannel(channel);
                    break;
                case 'audio':
                    this._setupAudioChannel(channel);
                    break;
                case 'cursor':
                    this._setupCursorChannel(channel);
                    break;
                default:
                    console.warn(`Unknown DataChannel: "${channel.label}"`);
            }
        };

        // Set remote description (server's offer)
        await this.pc.setRemoteDescription(
            new RTCSessionDescription({ type: 'offer', sdp: sdpOffer })
        );

        // Create answer
        const answer = await this.pc.createAnswer();
        await this.pc.setLocalDescription(answer);
        this.localSdp = answer.sdp;

        console.log(`WebRTC answer created (${this.localSdp.length} bytes)`);
        return this.localSdp;
    }

    /**
     * Add a remote ICE candidate from the server (received via WebSocket).
     *
     * @param {string|Object} candidateJson - ICE candidate as JSON string or object
     */
    async addIceCandidate(candidateJson) {
        if (!this.pc) {
            console.warn('WebRTC: cannot add ICE candidate — no peer connection');
            return;
        }

        try {
            const candidate = typeof candidateJson === 'string'
                ? JSON.parse(candidateJson)
                : candidateJson;

            await this.pc.addIceCandidate(new RTCIceCandidate(candidate));
            console.log('WebRTC: remote ICE candidate added');
        } catch (e) {
            console.warn('WebRTC: failed to add ICE candidate:', e);
        }
    }

    /**
     * Check if the video DataChannel is open and ready to receive.
     * @returns {boolean}
     */
    isVideoReady() {
        return this.videoChannel && this.videoChannel.readyState === 'open';
    }

    /**
     * Check if the WebRTC connection is established.
     * @returns {boolean}
     */
    isConnected() {
        return this.connected && this.pc && this.pc.connectionState === 'connected';
    }

    /**
     * Get transport statistics.
     * @returns {Object}
     */
    getStats() {
        return { ...this.stats };
    }

    /**
     * Get detailed WebRTC stats from the browser.
     * @returns {Promise<Object>}
     */
    async getDetailedStats() {
        if (!this.pc) return {};

        const stats = await this.pc.getStats();
        const result = {};
        stats.forEach((report) => {
            if (report.type === 'data-channel' || report.type === 'candidate-pair') {
                result[report.type] = report;
            }
        });
        return result;
    }

    /**
     * Close the WebRTC transport and release all resources.
     */
    close() {
        console.log('Closing WebRTC transport');

        if (this.videoChannel) {
            try { this.videoChannel.close(); } catch (e) {}
            this.videoChannel = null;
        }
        if (this.audioChannel) {
            try { this.audioChannel.close(); } catch (e) {}
            this.audioChannel = null;
        }
        if (this.cursorChannel) {
            try { this.cursorChannel.close(); } catch (e) {}
            this.cursorChannel = null;
        }
        if (this.pc) {
            try { this.pc.close(); } catch (e) {}
            this.pc = null;
        }

        this.connected = false;
        this._setState('closed');
    }

    // ── Private helpers ──────────────────────────────────────────────────

    /**
     * Set up the video DataChannel.
     * Binary mode (arraybuffer), low-latency: unordered, unreliable.
     */
    _setupVideoChannel(channel) {
        channel.binaryType = 'arraybuffer';

        channel.onopen = () => {
            console.log('WebRTC video DataChannel opened');
            this.videoChannel = channel;
        };

        channel.onmessage = (event) => {
            this.stats.videoFramesReceived++;
            this.stats.videoBytes += event.data.byteLength;
            // Pass the raw ArrayBuffer — same format as WebSocket binary messages
            this.onVideoFrame(event.data);
        };

        channel.onclose = () => {
            console.log('WebRTC video DataChannel closed');
            this.videoChannel = null;
        };

        channel.onerror = (error) => {
            console.error('WebRTC video DataChannel error:', error);
        };
    }

    /**
     * Set up the audio DataChannel.
     * Binary mode, reliable delivery for Opus frames.
     */
    _setupAudioChannel(channel) {
        channel.binaryType = 'arraybuffer';

        channel.onopen = () => {
            console.log('WebRTC audio DataChannel opened');
            this.audioChannel = channel;
        };

        channel.onmessage = (event) => {
            this.stats.audioFramesReceived++;
            this.stats.audioBytes += event.data.byteLength;
            this.onAudioFrame(event.data);
        };

        channel.onclose = () => {
            console.log('WebRTC audio DataChannel closed');
            this.audioChannel = null;
        };

        channel.onerror = (error) => {
            console.error('WebRTC audio DataChannel error:', error);
        };
    }

    /**
     * Set up the cursor DataChannel.
     * Binary mode, reliable delivery for cursor position/shape.
     */
    _setupCursorChannel(channel) {
        channel.binaryType = 'arraybuffer';

        channel.onopen = () => {
            console.log('WebRTC cursor DataChannel opened');
            this.cursorChannel = channel;
        };

        channel.onmessage = (event) => {
            this.stats.cursorUpdatesReceived++;
            this.onCursorUpdate(event.data);
        };

        channel.onclose = () => {
            console.log('WebRTC cursor DataChannel closed');
            this.cursorChannel = null;
        };

        channel.onerror = (error) => {
            console.error('WebRTC cursor DataChannel error:', error);
        };
    }

    /**
     * Update internal state and notify listeners.
     */
    _setState(state) {
        this.state = state;
        this.onStateChange(state);
    }
}

// Export for use in kvm-client.js
if (typeof window !== 'undefined') {
    window.WebRtcTransport = WebRtcTransport;
}

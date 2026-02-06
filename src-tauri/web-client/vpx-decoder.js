/**
 * VP8/VP9 Video Decoder for KVM Client
 * 
 * Uses the browser's WebCodecs API for hardware-accelerated VP8/VP9 decoding.
 * This is the RustDesk-inspired approach — VP9 is widely supported by all
 * modern browsers with hardware acceleration.
 * 
 * Key features:
 * - WebCodecs VP8/VP9 decoding (hardware accelerated)
 * - Ultra-low latency (<5ms decode on modern GPUs)
 * - Automatic keyframe detection
 * - Frame queue management
 * - Performance statistics
 */

class VpxDecoder {
    /**
     * @param {Object} options
     * @param {number} options.width - Initial frame width
     * @param {number} options.height - Initial frame height
     * @param {string} options.codec - 'vp8' or 'vp9'
     * @param {Function} options.onFrame - Callback for decoded frames (VideoFrame)
     * @param {Function} options.onError - Error callback
     * @param {Function} options.onReady - Ready callback
     */
    constructor(options = {}) {
        this.width = options.width || 1920;
        this.height = options.height || 1080;
        this.codec = options.codec || 'vp9';
        this.onFrame = options.onFrame || (() => {});
        this.onError = options.onError || console.error;
        this.onReady = options.onReady || (() => {});

        // Decoder state
        this.decoder = null;
        this.isReady = false;
        this.useWebCodecs = false;

        // Performance tracking
        this.stats = {
            framesDecoded: 0,
            framesDropped: 0,
            avgDecodeTime: 0,
            lastDecodeTime: 0,
            totalDecodeTime: 0,
        };

        // Canvas for fallback rendering
        this.canvas = null;
        this.ctx = null;

        this.initialize();
    }

    /**
     * Check if WebCodecs is available (requires secure context)
     */
    isSecureContext() {
        return window.isSecureContext || 
               location.hostname === 'localhost' || 
               location.hostname === '127.0.0.1' ||
               location.protocol === 'https:';
    }

    /**
     * Get the WebCodecs codec string for the selected codec
     */
    getCodecString() {
        switch (this.codec) {
            case 'vp8':
                return 'vp8';
            case 'vp9':
                return 'vp09.00.10.08'; // VP9 Profile 0, Level 1.0, 8-bit
            default:
                return 'vp09.00.10.08';
        }
    }

    /**
     * Initialize the decoder
     */
    async initialize() {
        if (!this.isSecureContext()) {
            console.warn('VPX decoder: Not in secure context, WebCodecs unavailable');
            this.useWebCodecs = false;
            this.isReady = true;
            this.onReady();
            return;
        }

        if (typeof VideoDecoder === 'undefined') {
            console.warn('VPX decoder: WebCodecs API not available');
            this.useWebCodecs = false;
            this.isReady = true;
            this.onReady();
            return;
        }

        try {
            const codecString = this.getCodecString();
            const support = await VideoDecoder.isConfigSupported({
                codec: codecString,
                codedWidth: this.width,
                codedHeight: this.height,
                hardwareAcceleration: 'prefer-hardware',
            });

            if (!support.supported) {
                console.warn(`VPX decoder: ${this.codec} not supported by WebCodecs`);
                this.useWebCodecs = false;
                this.isReady = true;
                this.onReady();
                return;
            }

            this.createDecoder();
            console.log(`VPX decoder: ${this.codec} WebCodecs decoder initialized (${this.width}x${this.height})`);
        } catch (e) {
            console.error('VPX decoder init error:', e);
            this.useWebCodecs = false;
            this.isReady = true;
            this.onReady();
        }
    }

    /**
     * Create the WebCodecs VideoDecoder
     */
    createDecoder() {
        if (this.decoder) {
            try { this.decoder.close(); } catch(e) {}
        }

        this.decoder = new VideoDecoder({
            output: (frame) => {
                this.stats.framesDecoded++;
                this.onFrame(frame);
                // IMPORTANT: Caller must call frame.close() after rendering!
            },
            error: (e) => {
                console.error('VPX VideoDecoder error:', e);
                this.onError(e);
            }
        });

        const codecString = this.getCodecString();
        this.decoder.configure({
            codec: codecString,
            codedWidth: this.width,
            codedHeight: this.height,
            optimizeForLatency: true,
            hardwareAcceleration: 'prefer-hardware',
        });

        this.useWebCodecs = true;
        this.isReady = true;
        this.onReady();
    }

    /**
     * Update frame dimensions (reconfigures decoder if needed)
     */
    setDimensions(width, height) {
        if (this.width === width && this.height === height) return;

        console.log(`VPX decoder: dimensions changed to ${width}x${height}`);
        this.width = width;
        this.height = height;

        if (this.useWebCodecs) {
            this.createDecoder();
        }
    }

    /**
     * Set the codec type
     */
    setCodec(codec) {
        if (this.codec === codec) return;
        console.log(`VPX decoder: codec changed to ${codec}`);
        this.codec = codec;
        if (this.useWebCodecs) {
            this.createDecoder();
        }
    }

    /**
     * Decode a VP8/VP9 encoded frame
     * @param {Uint8Array} data - Encoded VP8/VP9 frame data
     * @param {Object} options
     * @param {boolean} options.isKeyframe - Whether this is a keyframe
     * @param {number} options.timestamp - Frame timestamp in microseconds
     */
    decode(data, options = {}) {
        const startTime = performance.now();

        if (!this.useWebCodecs || !this.decoder) {
            // No WebCodecs — cannot decode VP8/VP9 without it
            return false;
        }

        if (this.decoder.state === 'closed') {
            console.warn('VPX decoder is closed, reinitializing...');
            this.createDecoder();
            return false;
        }

        try {
            // Check for decoder queue buildup — drop frames if too many queued
            if (this.decoder.decodeQueueSize > 3) {
                this.stats.framesDropped++;
                return false;
            }

            const chunk = new EncodedVideoChunk({
                type: options.isKeyframe ? 'key' : 'delta',
                timestamp: options.timestamp || 0,
                data: data,
            });

            this.decoder.decode(chunk);

            const decodeTime = performance.now() - startTime;
            this.stats.lastDecodeTime = decodeTime;
            this.stats.totalDecodeTime += decodeTime;
            this.stats.avgDecodeTime = this.stats.totalDecodeTime / this.stats.framesDecoded || 0;

            return true;
        } catch (e) {
            console.error('VPX decode error:', e);
            this.stats.framesDropped++;

            // If decode fails on a delta frame, request keyframe
            if (!options.isKeyframe) {
                this.onError(new Error('delta_decode_failed'));
            }

            return false;
        }
    }

    /**
     * Flush the decoder (wait for all pending frames)
     */
    async flush() {
        if (this.decoder && this.decoder.state !== 'closed') {
            try {
                await this.decoder.flush();
            } catch (e) {
                console.warn('VPX decoder flush error:', e);
            }
        }
    }

    /**
     * Reset the decoder (e.g., after seeking or error recovery)
     */
    reset() {
        if (this.decoder && this.decoder.state !== 'closed') {
            try {
                this.decoder.reset();
                this.createDecoder();
            } catch (e) {
                console.warn('VPX decoder reset error:', e);
                this.createDecoder();
            }
        }
    }

    /**
     * Get decoder statistics
     */
    getStats() {
        return { ...this.stats };
    }

    /**
     * Destroy the decoder and free resources
     */
    destroy() {
        if (this.decoder && this.decoder.state !== 'closed') {
            try {
                this.decoder.close();
            } catch (e) {}
        }
        this.decoder = null;
        this.isReady = false;
    }
}

// Export for use in kvm-client.js
if (typeof window !== 'undefined') {
    window.VpxDecoder = VpxDecoder;
}

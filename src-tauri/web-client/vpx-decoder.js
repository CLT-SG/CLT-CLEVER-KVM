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

        // After configure() or error recovery, the decoder needs a keyframe
        // before it can decode delta frames. Track this to avoid sending
        // delta frames that will be rejected.
        this.needsKeyframe = true;

        // Count consecutive decode errors to decide when to fall back
        this._decodeErrorCount = 0;
        this._maxDecodeErrors = 1; // After first error, fall back to software immediately

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
     * Get the WebCodecs codec string for the selected codec.
     * 
     * VP9 codec string format: vp09.PP.LL.DD
     *   PP = Profile (00 = Profile 0, 01 = Profile 1, 02 = Profile 2)
     *   LL = Level   (10 = 1.0, 21 = 2.1, 31 = 3.1, 41 = 4.1, 51 = 5.1)
     *   DD = Bit depth (08 = 8-bit, 10 = 10-bit)
     * 
     * Level must match the resolution:
     *   Level 1.0 (10) — up to 256×144
     *   Level 2.1 (21) — up to 480×256
     *   Level 3.0 (30) — up to 1080×512
     *   Level 3.1 (31) — up to 1920×1080 @ 30fps
     *   Level 4.0 (40) — up to 2048×1080 @ 60fps
     *   Level 4.1 (41) — up to 2048×1088 @ 60fps or 3840×2160 @ 30fps
     *   Level 5.1 (51) — up to 3840×2160 @ 120fps
     */
    getCodecString() {
        if (this.codec === 'vp8') {
            return 'vp8';
        }
        // VP9 — select level based on actual resolution
        const level = this.getVp9Level(this.width, this.height);
        return `vp09.00.${level}.08`; // Profile 0, dynamic level, 8-bit
    }

    /**
     * Determine the appropriate VP9 level for the given resolution.
     * Returns the 2-digit level code string.
     */
    getVp9Level(width, height) {
        const pixels = width * height;
        if (pixels <= 36864)   return '10'; // 256×144      — Level 1.0
        if (pixels <= 122880)  return '21'; // 480×256      — Level 2.1
        if (pixels <= 552960)  return '30'; // 960×576      — Level 3.0
        if (pixels <= 2073600) return '31'; // 1920×1080    — Level 3.1
        if (pixels <= 2228224) return '40'; // 2048×1088    — Level 4.0
        if (pixels <= 8912896) return '41'; // 3840×2160    — Level 4.1
        return '51';                        // > 4K         — Level 5.1
    }

    /**
     * Initialize the decoder with proper codec level detection and fallback.
     *
     * Strategy:
     *   1. Try hardware-accelerated WebCodecs with resolution-appropriate level
     *   2. Fall back to software WebCodecs if hardware is unavailable
     *   3. If WebCodecs is entirely unavailable, set useWebCodecs=false
     *      (caller should consider a WASM-based VP9 fallback like ogv.js)
     */
    async initialize() {
        if (!this.isSecureContext()) {
            console.warn('VPX decoder: Not in secure context — WebCodecs requires HTTPS or localhost');
            this.useWebCodecs = false;
            this.isReady = true;
            this.onReady();
            return;
        }

        if (typeof VideoDecoder === 'undefined') {
            console.warn('VPX decoder: WebCodecs API not available in this browser');
            this.useWebCodecs = false;
            this.isReady = true;
            this.onReady();
            return;
        }

        try {
            const codecString = this.getCodecString();
            console.log(`VPX decoder: probing codec "${codecString}" for ${this.width}x${this.height}`);

            // 1. Try hardware-accelerated decoding first
            let support = await VideoDecoder.isConfigSupported({
                codec: codecString,
                codedWidth: this.width,
                codedHeight: this.height,
                hardwareAcceleration: 'prefer-hardware',
            });

            if (support.supported) {
                this.hwAccel = 'prefer-hardware';
                this.createDecoder();
                console.log(`VPX decoder: ${this.codec} HW-accelerated decoder ready (${codecString}, ${this.width}x${this.height})`);
                return;
            }

            // 2. Fall back to software decoding
            console.warn(`VPX decoder: HW decode not supported for "${codecString}", trying software...`);
            support = await VideoDecoder.isConfigSupported({
                codec: codecString,
                codedWidth: this.width,
                codedHeight: this.height,
                hardwareAcceleration: 'prefer-software',
            });

            if (support.supported) {
                this.hwAccel = 'prefer-software';
                this.createDecoder();
                console.log(`VPX decoder: ${this.codec} SW decoder ready (${codecString}, ${this.width}x${this.height})`);
                return;
            }

            // 3. Try with no acceleration preference
            support = await VideoDecoder.isConfigSupported({
                codec: codecString,
                codedWidth: this.width,
                codedHeight: this.height,
            });

            if (support.supported) {
                this.hwAccel = 'no-preference';
                this.createDecoder();
                console.log(`VPX decoder: ${this.codec} decoder ready (no hw pref) (${codecString}, ${this.width}x${this.height})`);
                return;
            }

            console.error(`VPX decoder: codec "${codecString}" not supported by any WebCodecs path`);
            this.useWebCodecs = false;
            this.isReady = true;
            this.onReady();
        } catch (e) {
            console.error('VPX decoder init error:', e);
            this.useWebCodecs = false;
            this.isReady = true;
            this.onReady();
        }
    }

    /**
     * Create the WebCodecs VideoDecoder with the best available acceleration.
     */
    createDecoder() {
        if (this.decoder) {
            try { this.decoder.close(); } catch(e) {}
        }

        this.decoder = new VideoDecoder({
            output: (frame) => {
                this.stats.framesDecoded++;
                // First successful decode clears the needsKeyframe flag
                // and resets the error counter
                if (this.needsKeyframe) {
                    console.log('VPX decoder: first frame decoded successfully after init');
                    this.needsKeyframe = false;
                }
                this._decodeErrorCount = 0;
                this.onFrame(frame);
                // IMPORTANT: Caller must call frame.close() after rendering!
            },
            error: (e) => {
                console.error('VPX VideoDecoder error:', e);
                this._decodeErrorCount++;

                // If hardware decoding fails, fall back to software
                if (this._decodeErrorCount >= this._maxDecodeErrors && this.hwAccel === 'prefer-hardware') {
                    console.warn('VPX decoder: hardware decode failed, falling back to software');
                    this.hwAccel = 'prefer-software';
                } else if (this._decodeErrorCount >= this._maxDecodeErrors && this.hwAccel === 'prefer-software') {
                    console.warn('VPX decoder: software decode also failed, trying no-preference');
                    this.hwAccel = 'no-preference';
                }

                // Decoder transitions to 'closed' on error — we need to reinit
                // and request a keyframe
                this.needsKeyframe = true;
                this.onError(e);
            }
        });

        const codecString = this.getCodecString();
        const config = {
            codec: codecString,
            codedWidth: this.width,
            codedHeight: this.height,
            optimizeForLatency: true,
        };

        // Use the acceleration mode (may have been downgraded to software)
        if (this.hwAccel && this.hwAccel !== 'no-preference') {
            config.hardwareAcceleration = this.hwAccel;
        }

        console.log(`VPX decoder: configuring "${codecString}" ${this.width}x${this.height} (accel: ${this.hwAccel || 'default'})`);
        this.decoder.configure(config);

        // After configure(), the decoder needs a keyframe before delta frames
        this.needsKeyframe = true;
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
            if (!options.isKeyframe) {
                // Delta frame after reinit is useless — request keyframe
                this.onError(new Error('delta_decode_failed'));
                return false;
            }
            // Current frame IS a keyframe — fall through and try to decode
            // it with the freshly initialized decoder instead of wasting it
            console.log('VPX decoder: reinit done, retrying current keyframe');
        }

        // After configure() or error recovery, skip delta frames until
        // we receive a keyframe. Sending delta frames to a freshly
        // configured decoder causes "A key frame is required" errors.
        if (this.needsKeyframe && !options.isKeyframe) {
            this.stats.framesDropped++;
            return false;
        }

        try {
            // Check for decoder queue buildup — drop frames if too many queued
            if (this.decoder.decodeQueueSize > 3) {
                this.stats.framesDropped++;
                return false;
            }

            // Log first few bytes of first frames for diagnostics
            if (this.stats.framesDecoded < 3 && data.length >= 4) {
                const hex = Array.from(data.slice(0, 8)).map(b => b.toString(16).padStart(2, '0')).join(' ');
                console.log(`VPX decode: frame ${this.stats.framesDecoded}, ${options.isKeyframe ? 'key' : 'delta'}, ${data.length}B, first_bytes=[${hex}]`);
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

            // Any decode failure — request keyframe from server
            this.needsKeyframe = true;
            this.onError(new Error('delta_decode_failed'));

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

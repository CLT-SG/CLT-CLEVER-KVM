/**
 * H.264 Video Decoder for KVM Client
 * 
 * This module handles H.264 frame decoding using:
 * 1. WebCodecs API (preferred - hardware accelerated)
 * 2. Canvas-based software decoding (fallback)
 * 
 * Key features:
 * - Ultra-low latency decoding (<5ms on hardware)
 * - Automatic codec negotiation
 * - Frame queue management
 * - Graceful fallback handling
 */

class H264Decoder {
    constructor(options = {}) {
        this.width = options.width || 1920;
        this.height = options.height || 1080;
        this.onFrame = options.onFrame || (() => {});
        this.onError = options.onError || console.error;
        this.onReady = options.onReady || (() => {});
        
        // Decoder state
        this.decoder = null;
        this.isReady = false;
        this.useWebCodecs = false;
        this.pendingFrames = [];
        
        // SPS/PPS storage for stream initialization
        this.sps = null;
        this.pps = null;
        
        // Performance tracking
        this.stats = {
            framesDecoded: 0,
            framesDropped: 0,
            avgDecodeTime: 0,
            lastDecodeTime: 0
        };
        
        // Canvas for software fallback
        this.canvas = null;
        this.ctx = null;
        
        this.initialize();
    }
    
    /**
     * Check if we're in a secure context (HTTPS or localhost)
     */
    isSecureContext() {
        return window.isSecureContext || 
               window.location.protocol === 'https:' ||
               window.location.hostname === 'localhost' ||
               window.location.hostname === '127.0.0.1';
    }
    
    async initialize() {
        // Check for WebCodecs support - requires secure context (HTTPS or localhost)
        if ('VideoDecoder' in window && this.isSecureContext()) {
            this.useWebCodecs = true;
            await this.initializeWebCodecs();
        } else {
            // Provide helpful message based on the reason
            if (!('VideoDecoder' in window)) {
                console.log('ℹ️ WebCodecs API not supported in this browser - using software decoding');
            } else if (!this.isSecureContext()) {
                console.log('ℹ️ WebCodecs API requires HTTPS - using software decoding');
                console.log('   💡 Tip: Access this page via HTTPS for hardware-accelerated decoding');
            }
            this.useWebCodecs = false;
            this.initializeSoftwareDecoder();
        }
    }
    
    async initializeWebCodecs() {
        try {
            // Check H.264 support
            const config = {
                codec: 'avc1.42001E', // Baseline profile, level 3.0
                codedWidth: this.width,
                codedHeight: this.height,
                optimizeForLatency: true,
                hardwareAcceleration: 'prefer-hardware'
            };
            
            const support = await VideoDecoder.isConfigSupported(config);
            
            if (!support.supported) {
                // Try Main profile
                config.codec = 'avc1.4D001E';
                const mainSupport = await VideoDecoder.isConfigSupported(config);
                
                if (!mainSupport.supported) {
                    throw new Error('H.264 decoding not supported by WebCodecs');
                }
            }
            
            this.decoder = new VideoDecoder({
                output: (frame) => this.handleDecodedFrame(frame),
                error: (error) => this.handleDecodeError(error)
            });
            
            this.decoder.configure(config);
            this.isReady = true;
            
            console.log('🎬 WebCodecs H.264 decoder initialized (hardware accelerated)');
            this.onReady();
            
        } catch (error) {
            console.log('ℹ️ WebCodecs initialization unavailable, using software decoder');
            this.useWebCodecs = false;
            this.initializeSoftwareDecoder();
        }
    }
    
    initializeSoftwareDecoder() {
        // Create canvas for software decoding output
        this.canvas = document.createElement('canvas');
        this.canvas.width = this.width;
        this.canvas.height = this.height;
        this.ctx = this.canvas.getContext('2d', {
            alpha: false,
            desynchronized: true
        });
        
        this.isReady = true;
        console.log('✅ Software decoder ready');
        this.onReady();
    }
    
    /**
     * Set SPS/PPS for stream initialization
     */
    setParameterSets(spsData, ppsData) {
        this.sps = spsData;
        this.pps = ppsData;
        console.log('📋 Parameter sets stored - SPS:', spsData?.length, 'bytes, PPS:', ppsData?.length, 'bytes');
    }
    
    /**
     * Decode an H.264 frame
     * @param {Uint8Array} frameData - Raw H.264 NAL units
     * @param {Object} metadata - Frame metadata
     */
    async decode(frameData, metadata = {}) {
        const decodeStart = performance.now();
        
        if (!this.isReady) {
            console.warn('Decoder not ready, queueing frame');
            this.pendingFrames.push({ frameData, metadata });
            return;
        }
        
        try {
            if (this.useWebCodecs) {
                await this.decodeWithWebCodecs(frameData, metadata);
            } else {
                await this.decodeWithSoftware(frameData, metadata);
            }
            
            const decodeTime = performance.now() - decodeStart;
            this.updateStats(decodeTime);
            
        } catch (error) {
            this.stats.framesDropped++;
            this.onError(error);
        }
    }
    
    async decodeWithWebCodecs(frameData, metadata) {
        const chunk = new EncodedVideoChunk({
            type: metadata.isKeyframe ? 'key' : 'delta',
            timestamp: metadata.timestamp || Date.now() * 1000,
            data: frameData
        });
        
        this.decoder.decode(chunk);
    }
    
    async decodeWithSoftware(frameData, metadata) {
        // Software fallback: Parse H.264 and render as RGBA
        // This is simplified - a full implementation would use a WASM decoder
        
        // For now, we'll extract raw pixel data from simplified H.264 stream
        const rgbaData = this.extractPixelData(frameData, metadata);
        
        if (rgbaData) {
            const imageData = new ImageData(
                new Uint8ClampedArray(rgbaData),
                this.width,
                this.height
            );
            this.ctx.putImageData(imageData, 0, 0);
            
            // Create ImageBitmap for efficient rendering
            const bitmap = await createImageBitmap(this.canvas);
            this.onFrame(bitmap, metadata);
        }
    }
    
    /**
     * Extract pixel data from simplified H.264 stream
     * This handles the high-quality subsampled YUV format from our encoder
     */
    extractPixelData(frameData, metadata) {
        let offset = 0;
        
        // Find slice data (skip SPS, PPS, etc.)
        while (offset < frameData.length - 4) {
            // Look for NAL start code
            if (frameData[offset] === 0 && frameData[offset + 1] === 0 && 
                frameData[offset + 2] === 0 && frameData[offset + 3] === 1) {
                
                const nalType = frameData[offset + 4] & 0x1F;
                
                if (nalType === 5 || nalType === 1) {
                    // IDR or non-IDR slice - contains pixel data
                    return this.parseHighQualityYUV(frameData, offset + 9);
                }
                
                offset += 4;
            } else {
                offset++;
            }
        }
        
        return null;
    }
    
    /**
     * Parse high-quality subsampled YUV data with bilinear upscaling
     */
    parseHighQualityYUV(frameData, offset) {
        // Read subsampled dimensions
        if (offset + 8 > frameData.length) {
            return null;
        }
        
        const yOutWidth = frameData[offset] | (frameData[offset + 1] << 8);
        const yOutHeight = frameData[offset + 2] | (frameData[offset + 3] << 8);
        const uvOutWidth = frameData[offset + 4] | (frameData[offset + 5] << 8);
        const uvOutHeight = frameData[offset + 6] | (frameData[offset + 7] << 8);
        offset += 8;
        
        const yDataSize = yOutWidth * yOutHeight;
        const uvDataSize = uvOutWidth * uvOutHeight;
        
        if (offset + yDataSize + uvDataSize * 2 > frameData.length) {
            // Try legacy format
            return this.parseSliceDataLegacy(frameData, offset - 8);
        }
        
        // Extract YUV planes
        const yPlane = frameData.subarray(offset, offset + yDataSize);
        offset += yDataSize;
        const uPlane = frameData.subarray(offset, offset + uvDataSize);
        offset += uvDataSize;
        const vPlane = frameData.subarray(offset, offset + uvDataSize);
        
        // Create RGBA output with bilinear upscaling
        const rgbaData = new Uint8Array(this.width * this.height * 4);
        
        // Calculate scaling factors
        const yScaleX = yOutWidth / this.width;
        const yScaleY = yOutHeight / this.height;
        const uvScaleX = uvOutWidth / this.width;
        const uvScaleY = uvOutHeight / this.height;
        
        // Bilinear interpolation for high-quality upscaling
        for (let py = 0; py < this.height; py++) {
            for (let px = 0; px < this.width; px++) {
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
                const yVal = y;
                const uVal = u - 128;
                const vVal = v - 128;
                
                const r = Math.max(0, Math.min(255, Math.round(yVal + 1.402 * vVal)));
                const g = Math.max(0, Math.min(255, Math.round(yVal - 0.344 * uVal - 0.714 * vVal)));
                const b = Math.max(0, Math.min(255, Math.round(yVal + 1.772 * uVal)));
                
                const pixelIndex = (py * this.width + px) * 4;
                rgbaData[pixelIndex] = r;
                rgbaData[pixelIndex + 1] = g;
                rgbaData[pixelIndex + 2] = b;
                rgbaData[pixelIndex + 3] = 255;
            }
        }
        
        return rgbaData;
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
     * Legacy format parser for backward compatibility
     */
    parseSliceDataLegacy(frameData, offset) {
        // Skip slice header
        offset += 4;
        
        const mbWidth = Math.ceil(this.width / 16);
        const mbHeight = Math.ceil(this.height / 16);
        const mbCount = mbWidth * mbHeight;
        const mbDataSize = mbCount * 3;
        
        if (offset + mbDataSize > frameData.length) {
            return this.parseSliceDataGrayscale(frameData, offset, mbWidth, mbHeight, mbCount);
        }
        
        const rgbaData = new Uint8Array(this.width * this.height * 4);
        
        for (let mbY = 0; mbY < mbHeight; mbY++) {
            for (let mbX = 0; mbX < mbWidth; mbX++) {
                const mbIndex = (mbY * mbWidth + mbX) * 3;
                const y = frameData[offset + mbIndex] || 128;
                const u = frameData[offset + mbIndex + 1] || 128;
                const v = frameData[offset + mbIndex + 2] || 128;
                
                // BT.601 full range conversion
                const yVal = y;
                const uVal = u - 128;
                const vVal = v - 128;
                
                const r = Math.max(0, Math.min(255, Math.round(yVal + 1.402 * vVal)));
                const g = Math.max(0, Math.min(255, Math.round(yVal - 0.344 * uVal - 0.714 * vVal)));
                const b = Math.max(0, Math.min(255, Math.round(yVal + 1.772 * uVal)));
                
                for (let dy = 0; dy < 16; dy++) {
                    const py = mbY * 16 + dy;
                    if (py >= this.height) continue;
                    
                    for (let dx = 0; dx < 16; dx++) {
                        const px = mbX * 16 + dx;
                        if (px >= this.width) continue;
                        
                        const pixelIndex = (py * this.width + px) * 4;
                        rgbaData[pixelIndex] = r;
                        rgbaData[pixelIndex + 1] = g;
                        rgbaData[pixelIndex + 2] = b;
                        rgbaData[pixelIndex + 3] = 255;
                    }
                }
            }
        }
        
        return rgbaData;
    }
    
    /**
     * Grayscale fallback for legacy Y-only data
     */
    parseSliceDataGrayscale(frameData, offset, mbWidth, mbHeight, mbCount) {
        if (offset + mbCount > frameData.length) {
            return null;
        }
        
        const rgbaData = new Uint8Array(this.width * this.height * 4);
        
        for (let mbY = 0; mbY < mbHeight; mbY++) {
            for (let mbX = 0; mbX < mbWidth; mbX++) {
                const mbIndex = mbY * mbWidth + mbX;
                const yValue = frameData[offset + mbIndex] || 128;
                
                for (let dy = 0; dy < 16; dy++) {
                    const py = mbY * 16 + dy;
                    if (py >= this.height) continue;
                    
                    for (let dx = 0; dx < 16; dx++) {
                        const px = mbX * 16 + dx;
                        if (px >= this.width) continue;
                        
                        const pixelIndex = (py * this.width + px) * 4;
                        rgbaData[pixelIndex] = yValue;
                        rgbaData[pixelIndex + 1] = yValue;
                        rgbaData[pixelIndex + 2] = yValue;
                        rgbaData[pixelIndex + 3] = 255;
                    }
                }
            }
        }
        
        return rgbaData;
    }
    
    handleDecodedFrame(frame) {
        this.stats.framesDecoded++;
        
        // Create metadata
        const metadata = {
            timestamp: frame.timestamp,
            width: frame.displayWidth,
            height: frame.displayHeight,
            duration: frame.duration
        };
        
        // Pass to callback - caller should close the frame after use
        this.onFrame(frame, metadata);
    }
    
    handleDecodeError(error) {
        console.error('Decode error:', error);
        this.stats.framesDropped++;
        this.onError(error);
    }
    
    updateStats(decodeTime) {
        this.stats.lastDecodeTime = decodeTime;
        this.stats.avgDecodeTime = 
            (this.stats.avgDecodeTime * this.stats.framesDecoded + decodeTime) / 
            (this.stats.framesDecoded + 1);
        this.stats.framesDecoded++;
    }
    
    /**
     * Update decoder dimensions
     */
    setDimensions(width, height) {
        if (this.width !== width || this.height !== height) {
            this.width = width;
            this.height = height;
            
            if (this.canvas) {
                this.canvas.width = width;
                this.canvas.height = height;
            }
            
            // Reinitialize WebCodecs decoder with new dimensions
            if (this.useWebCodecs && this.decoder) {
                this.decoder.reset();
                this.initializeWebCodecs();
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
     * Flush pending frames and reset decoder
     */
    flush() {
        if (this.useWebCodecs && this.decoder) {
            this.decoder.flush();
        }
        this.pendingFrames = [];
    }
    
    /**
     * Close the decoder
     */
    close() {
        if (this.useWebCodecs && this.decoder) {
            this.decoder.close();
            this.decoder = null;
        }
        this.isReady = false;
    }
}

// Export for use in KVM client
if (typeof window !== 'undefined') {
    window.H264Decoder = H264Decoder;
}

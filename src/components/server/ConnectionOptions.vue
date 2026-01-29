<template>
  <div class="connection-options">
    <h2>VNC Connection Guide</h2>
    
    <div class="guide-section">
      <h3>Desktop VNC Clients</h3>
      <p>Connect to the VNC server using any standard VNC client:</p>
      
      <div class="client-examples">
        <div class="client-example">
          <h4>TigerVNC</h4>
          <code>vncviewer &lt;hostname&gt;:&lt;port&gt;</code>
          <p class="description">Lightweight and fast VNC client for Linux, macOS, and Windows</p>
        </div>
        
        <div class="client-example">
          <h4>RealVNC</h4>
          <code>vnc://&lt;hostname&gt;:&lt;port&gt;</code>
          <p class="description">Popular cross-platform VNC solution with extra features</p>
        </div>
        
        <div class="client-example">
          <h4>VNC Viewer</h4>
          <p class="description">Enter <code>&lt;hostname&gt;:&lt;port&gt;</code> in the connection dialog</p>
        </div>
      </div>
    </div>
    
    <div class="guide-section highlight-section">
      <h3>🌐 NoVNC (Browser-Based Client)</h3>
      <p>Connect using NoVNC for browser-based access via WebSocket:</p>
      <p><strong>WebSocket URL format:</strong> <code>ws://&lt;hostname&gt;:&lt;websockify_port&gt;/</code></p>
      <p><em>Example:</em> <code>ws://workstation-1:6080/</code></p>
      <div class="info-box">
        <p><strong>Note:</strong> NoVNC requires a WebSocket proxy (websockify) between the browser and VNC server.</p>
        <p>The websockify runs on port 6080+ (not the VNC port). The exact URL is shown in the server status after starting.</p>
      </div>
    </div>
    
    <div class="guide-section">
      <h3>🔊 Audio Streaming</h3>
      <p>Audio streams via <strong>WebSocket</strong> using Opus encoding for low latency:</p>
      <p><strong>Audio URL format:</strong> <code>ws://&lt;hostname&gt;:6900/audio</code></p>
      <p><em>Example:</em> <code>ws://workstation-1:6900/audio</code></p>
      <div class="info-box">
        <p><strong>Shared Audio:</strong> All monitors share a single audio stream on port 6900 since system audio is identical across all displays.</p>
        <p><strong>Encoding:</strong> 48kHz stereo Opus with low-latency mode (5-30ms latency)</p>
        <p><strong>Note:</strong> VNC clients don't include audio. Use a separate WebSocket audio client or integrate with your video wall software.</p>
      </div>
    </div>
    
    <div class="guide-section">
      <h3>Multi-Monitor Support</h3>
      <p>Each monitor gets its own VNC server with automatic port assignment:</p>
      <ul>
        <li><strong>Monitor 0 (Primary):</strong> VNC port 5900, WebSocket ws://hostname:6080/</li>
        <li><strong>Monitor 1:</strong> VNC port 5901, WebSocket ws://hostname:6081/</li>
        <li><strong>Monitor 2:</strong> VNC port 5902, WebSocket ws://hostname:6082/</li>
        <li><strong>Audio (Shared):</strong> WebSocket ws://hostname:6900/audio</li>
      </ul>
      <p class="description">Note: Use the websockify port (608x) for NoVNC, not the VNC port (590x)</p>
    </div>
    
    <div class="features-section">
      <h3>Features</h3>
      <ul>
        <li><strong>Standard VNC Protocol:</strong> RFB 3.8 compatible with all VNC clients</li>
        <li><strong>WebSocket Support:</strong> NoVNC browser-based access with websockify URLs</li>
        <li><strong>Low-Latency Audio:</strong> WebSocket audio streaming with Opus codec (5-30ms)</li>
        <li><strong>Multi-Client Support:</strong> Up to 10 simultaneous connections per monitor</li>
        <li><strong>Hardware Acceleration:</strong> GPU-accelerated screen capture when available</li>
        <li><strong>Full Control:</strong> Complete keyboard and mouse control</li>
        <li><strong>Exact Positioning:</strong> Preserves monitor layout and positioning</li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.connection-options {
  max-width: 800px;
  margin: 0 auto;
}

h2 {
  color: #2c3e50;
  margin-top: 0;
  font-size: 1.5rem;
}

h3 {
  color: #2c3e50;
  font-size: 1.2rem;
  margin-top: 1.5rem;
  margin-bottom: 0.75rem;
}

h4 {
  color: #495057;
  font-size: 1rem;
  margin: 0.5rem 0 0.25rem 0;
}

.guide-section {
  margin-bottom: 2rem;
  padding: 1.5rem;
  background-color: #f8f9fa;
  border-radius: 8px;
  border: 1px solid #dee2e6;
}

.guide-section.highlight-section {
  background-color: #e7f3ff;
  border: 2px solid #0066cc;
}

.guide-section p {
  margin: 0.5rem 0;
  line-height: 1.6;
}

.info-box {
  margin-top: 1rem;
  padding: 1rem;
  background-color: rgba(255, 255, 255, 0.6);
  border-left: 3px solid #0066cc;
  border-radius: 4px;
}

.info-box p {
  margin: 0.5rem 0;
  font-size: 0.9rem;
}

.client-examples {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 1rem;
  margin-top: 1rem;
}

.client-example {
  padding: 1rem;
  background-color: #ffffff;
  border-radius: 6px;
  border: 1px solid #dee2e6;
}

.description {
  font-size: 0.85rem;
  color: #6c757d;
  margin-top: 0.5rem;
}

code {
  background-color: #e9ecef;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  font-family: 'Courier New', monospace;
  font-size: 0.9rem;
  color: #c7254e;
}

.features-section {
  margin-top: 2rem;
}

.features-section ul {
  padding-left: 1.5rem;
  line-height: 1.8;
}

.features-section li {
  margin-bottom: 0.5rem;
}

.features-section strong {
  color: #0066cc;
}

.guide-section ul {
  padding-left: 1.5rem;
  line-height: 1.8;
}

.guide-section li {
  margin-bottom: 0.5rem;
}

@media (max-width: 768px) {
  .client-examples {
    grid-template-columns: 1fr;
  }
  
  .guide-section {
    padding: 1rem;
  }
}
</style>

<template>
  <div class="connection-options">
    <h2>VNC Connection Guide</h2>
    
    <div class="guide-section">
      <h3>🖥️ Using VNC Clients</h3>
      <p>Connect to the VNC server using any standard VNC client:</p>
      
      <div class="client-examples">
        <div class="client-example">
          <h4>TigerVNC</h4>
          <code>vncviewer &lt;ip-address&gt;:&lt;port&gt;</code>
          <p class="description">Lightweight and fast VNC client for Linux, macOS, and Windows</p>
        </div>
        
        <div class="client-example">
          <h4>RealVNC</h4>
          <code>vnc://&lt;ip-address&gt;:&lt;port&gt;</code>
          <p class="description">Popular cross-platform VNC solution with extra features</p>
        </div>
        
        <div class="client-example">
          <h4>VNC Viewer</h4>
          <p class="description">Enter <code>&lt;ip-address&gt;:&lt;port&gt;</code> in the connection dialog</p>
        </div>
      </div>
    </div>
    
    <div class="guide-section">
      <h3>🎵 Audio Streaming</h3>
      <p>When audio is enabled, it streams separately via RTSP on the configured audio port (default: 6900).</p>
      <p>Audio URL format: <code>rtsp://&lt;ip-address&gt;:&lt;audio-port&gt;/audio</code></p>
      <p><strong>Note:</strong> Most VNC clients don't support audio. You'll need a separate RTSP client or use MediaMTX for integration.</p>
    </div>
    
    <div class="guide-section">
      <h3>🔧 Multi-Monitor Support</h3>
      <p>Each monitor gets its own VNC server with automatic port assignment:</p>
      <ul>
        <li><strong>Monitor 1 (Primary):</strong> Port 5900</li>
        <li><strong>Monitor 2:</strong> Port 5901</li>
        <li><strong>Monitor 3:</strong> Port 5902</li>
        <li>And so on...</li>
      </ul>
    </div>
    
    <div class="guide-section">
      <h3>🎛️ Integration with MediaMTX</h3>
      <p>For video wall deployments, configure MediaMTX to relay VNC streams:</p>
      <pre class="code-block">paths:
  workstation_1_screen1:
    source: vnc://192.168.1.100:5900
    sourceProtocol: vnc
  workstation_1_audio:
    source: rtsp://192.168.1.100:&lt;audio-port&gt;/audio
    # Default audio port is 6900, but check your settings</pre>
    </div>
    
    <div class="features-section">
      <h3>✨ Features</h3>
      <ul>
        <li><strong>Standard VNC Protocol:</strong> RFB 3.8 compatible with all VNC clients</li>
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

.guide-section p {
  margin: 0.5rem 0;
  line-height: 1.6;
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

.code-block {
  background-color: #2d2d2d;
  color: #f8f8f2;
  padding: 1rem;
  border-radius: 6px;
  overflow-x: auto;
  font-family: 'Courier New', monospace;
  font-size: 0.9rem;
  line-height: 1.5;
  margin: 1rem 0;
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

@media (max-width: 768px) {
  .client-examples {
    grid-template-columns: 1fr;
  }
  
  .guide-section {
    padding: 1rem;
  }
}
</style>

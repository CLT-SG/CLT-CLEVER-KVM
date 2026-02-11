<template>
  <div class="connection-options">
    <h2 class="section-title">Connection Parameters</h2>
    <p class="section-description">Add these query parameters to customize your connection</p>
    
    <div class="options-grid">
      <div class="option-card" v-for="option in options" :key="option.param">
        <div class="option-header">
          <code class="option-param">{{ option.param }}</code>
          <span v-if="option.badge" class="option-badge" :class="option.badge.type">
            {{ option.badge.text }}
          </span>
        </div>
        <p class="option-desc">{{ option.description }}</p>
      </div>
    </div>
    
    <div class="example-section">
      <h3>Example URL</h3>
      <div class="example-url">
        <code>http://hostname:9921/kvm?stretch=true;monitor=1</code>
        <button class="copy-btn" @click="copyExample" title="Copy example">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
          </svg>
        </button>
      </div>
    </div>
    
    <div class="features-section">
      <h3>Advanced Features</h3>
      <div class="features-grid">
        <div class="feature-card" v-for="feature in features" :key="feature.name">
          <span class="feature-icon" v-html="feature.icon"></span>
          <div class="feature-content">
            <strong>{{ feature.name }}</strong>
            <p>{{ feature.description }}</p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
const options = [
  { param: 'stretch=true', description: 'Stretch screen to fit window', badge: null },
  { param: 'mute=true', description: 'Mute audio on connect', badge: null },
  { param: 'audio=true', description: 'Enable audio streaming', badge: { text: 'WebRTC', type: 'info' } },
  { param: 'remoteOnly=true', description: 'Only show remote screen (no toolbar)', badge: null },
  { param: 'encryption=true', description: 'Enable encrypted connection', badge: { text: 'Secure', type: 'success' } },
  { param: 'monitor=1', description: 'Select specific monitor to display', badge: null }
];

const features = [
  { 
    name: 'Delta Encoding', 
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/></svg>', 
    description: 'Only sends changed parts of the screen' 
  },
  { 
    name: 'Adaptive Quality', 
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/></svg>', 
    description: 'Adjusts quality based on network conditions' 
  },
  { 
    name: 'Encryption', 
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>', 
    description: 'Secures connections between client and server' 
  },
  { 
    name: 'WebRTC Audio', 
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/></svg>', 
    description: 'Low-latency audio streaming' 
  },
  { 
    name: 'H.264 Codec', 
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/></svg>', 
    description: 'Optimized WebRTC H.264 encoding' 
  },
  { 
    name: 'Hardware Accel', 
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>', 
    description: 'GPU encoding for reduced CPU usage' 
  },
  { 
    name: 'Multi-Monitor', 
    icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>', 
    description: 'Choose which monitor to share' 
  }
];

function copyExample() {
  navigator.clipboard.writeText('http://hostname:9921/kvm?stretch=true;monitor=1');
}
</script>

<style scoped>
.connection-options {
  max-width: 100%;
}

.section-title {
  margin: 0 0 var(--spacing-xs) 0;
  font-size: 1.35rem;
  color: var(--text-primary);
}

.section-description {
  margin: 0 0 var(--spacing-xl) 0;
  color: var(--text-muted);
}

/* Options Grid */
.options-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-xl);
}

.option-card {
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  padding: var(--spacing-md);
  transition: border-color var(--transition-fast);
}

.option-card:hover {
  border-color: var(--border-light);
}

.option-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-sm);
}

.option-param {
  font-family: var(--font-mono);
  font-size: 0.85rem;
  color: var(--color-primary);
  background: var(--bg-primary);
  padding: 0.25rem 0.5rem;
  border-radius: var(--radius-sm);
}

.option-badge {
  font-size: 0.65rem;
  padding: 0.2rem 0.5rem;
  border-radius: var(--radius-sm);
  text-transform: uppercase;
  font-weight: 600;
  letter-spacing: 0.03em;
}

.option-badge.info {
  background: rgba(0, 180, 255, 0.15);
  color: var(--color-info);
}

.option-badge.success {
  background: rgba(0, 255, 65, 0.15);
  color: var(--color-success);
}

.option-desc {
  margin: 0;
  font-size: 0.85rem;
  color: var(--text-secondary);
}

/* Example Section */
.example-section {
  margin-bottom: var(--spacing-xl);
}

.example-section h3 {
  margin: 0 0 var(--spacing-md) 0;
  font-size: 1.1rem;
  color: var(--text-primary);
}

.example-url {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  padding: var(--spacing-sm) var(--spacing-md);
}

.example-url code {
  flex: 1;
  font-family: var(--font-mono);
  font-size: 0.85rem;
  color: var(--color-primary);
  background: transparent;
  padding: 0;
  word-break: break-all;
}

.copy-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-sm);
  background: var(--bg-hover);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.copy-btn:hover {
  color: var(--color-primary);
  border-color: var(--color-primary);
}

/* Features Section */
.features-section h3 {
  margin: 0 0 var(--spacing-lg) 0;
  font-size: 1.1rem;
  color: var(--text-primary);
}

.features-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: var(--spacing-md);
}

.feature-card {
  display: flex;
  gap: var(--spacing-md);
  padding: var(--spacing-md);
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  transition: border-color var(--transition-fast);
}

.feature-card:hover {
  border-color: var(--border-light);
}

.feature-icon {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: var(--color-primary);
  background: rgba(0, 255, 65, 0.1);
  border-radius: var(--radius-md);
  padding: 6px;
}

.feature-icon :deep(svg) {
  width: 100%;
  height: 100%;
}

.feature-content strong {
  display: block;
  color: var(--text-primary);
  font-size: 0.9rem;
  margin-bottom: var(--spacing-xs);
}

.feature-content p {
  margin: 0;
  font-size: 0.8rem;
  color: var(--text-muted);
  line-height: 1.4;
}

/* Responsive */
@media (max-width: 600px) {
  .options-grid,
  .features-grid {
    grid-template-columns: 1fr;
  }
}
</style>

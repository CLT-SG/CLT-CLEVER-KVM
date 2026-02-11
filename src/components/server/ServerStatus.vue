<template>
  <div class="server-status">
    <!-- Status Display - Clickable -->
    <div class="status-display">
      <button 
        class="status-ring" 
        :class="{ active: serverStatus, loading: loading }"
        @click="toggleServer"
        :disabled="loading"
        :title="serverStatus ? 'Click to stop server' : 'Click to start server'"
      >
        <div class="status-inner">
          <div class="status-icon">
            <span v-if="loading" class="spinner-large"></span>
            <svg v-else-if="serverStatus" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="6" y="6" width="12" height="12" rx="2" />
            </svg>
            <svg v-else viewBox="0 0 24 24" fill="currentColor">
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
          </div>
          <p class="status-text">{{ loading ? (serverStatus ? 'Stopping...' : 'Starting...') : (serverStatus ? 'Running' : 'Stopped') }}</p>
          <p class="status-hint">{{ loading ? '' : (serverStatus ? 'Click to stop' : 'Click to start') }}</p>
        </div>
      </button>
    </div>
    
    <!-- Server URL Info -->
    <div v-if="serverStatus" class="server-info">
      <div class="info-card">
        <div class="info-header">
          <span class="info-label">Direct Access URL</span>
          <span class="info-badge">Local Network</span>
        </div>
        <div class="url-display">
          <code class="url-text">{{ displayUrl }}</code>
          <div class="url-actions">
            <button class="action-btn" @click="openUrl" title="Open in browser">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
                <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
                <polyline points="15 3 21 3 21 9" />
                <line x1="10" y1="14" x2="21" y2="3" />
              </svg>
              Open
            </button>
            <button class="action-btn" @click="copyUrl" title="Copy URL">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
              </svg>
              Copy
            </button>
          </div>
        </div>
        <p class="info-hint">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="16" x2="12" y2="12" />
            <line x1="12" y1="8" x2="12.01" y2="8" />
          </svg>
          Connect from any device on the same network
        </p>
      </div>
    </div>

    <!-- Error Message -->
    <div v-if="errorMessage" class="error-message">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
        <circle cx="12" cy="12" r="10" />
        <line x1="15" y1="9" x2="9" y2="15" />
        <line x1="9" y1="9" x2="15" y2="15" />
      </svg>
      {{ errorMessage }}
    </div>
  </div>
</template>

<script setup>
const props = defineProps({
  serverStatus: Boolean,
  serverUrl: String,
  vncInfo: Object,
  loading: Boolean,
  errorMessage: String,
  startServer: Function,
  stopServer: Function,
  copyUrl: Function
});

const displayUrl = computed(() => {
  if (!props.serverUrl) return '';
  return props.serverUrl;
});

const toggleServer = () => {
  if (props.loading) return;
  if (props.serverStatus) {
    props.stopServer();
  } else {
    props.startServer();
  }
};
</script>

<style scoped>
.server-status {
  margin-top: 5%;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-xl);
}

/* Status Display */
.status-display {
  display: flex;
  justify-content: center;
}

.status-ring {
  position: relative;
  width: 160px;
  height: 160px;
  border-radius: 50%;
  background: var(--bg-tertiary);
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  cursor: pointer;
  transition: all var(--transition-fast);
  font-family: inherit;
}

.status-ring:hover:not(:disabled) {
  transform: scale(1.02);
}

.status-ring:active:not(:disabled) {
  transform: scale(0.98);
}

.status-ring:disabled {
  cursor: wait;
}

.status-ring::before {
  content: '';
  position: absolute;
  inset: 4px;
  border-radius: 50%;
  border: 3px solid var(--border-color);
  transition: all var(--transition-normal);
}

.status-ring:hover:not(:disabled)::before {
  border-color: var(--border-light);
}

.status-ring.active::before {
  border-color: var(--color-primary);
  box-shadow: 0 0 20px var(--color-primary), inset 0 0 20px rgba(0, 255, 65, 0.1);
  animation: pulseRing 2s ease-in-out infinite;
}

.status-ring.active:hover:not(:disabled)::before {
  border-color: var(--color-error);
  box-shadow: 0 0 20px rgba(255, 61, 61, 0.5), inset 0 0 20px rgba(255, 61, 61, 0.1);
  animation: none;
}

.status-ring:not(.active):hover:not(:disabled)::before {
  border-color: var(--color-primary);
  box-shadow: 0 0 15px rgba(0, 255, 65, 0.3);
}

.status-ring.loading::before {
  animation: loadingPulse 1s ease-in-out infinite;
}

@keyframes pulseRing {
  0%, 100% {
    box-shadow: 0 0 20px var(--color-primary), inset 0 0 20px rgba(0, 255, 65, 0.1);
  }
  50% {
    box-shadow: 0 0 35px var(--color-primary), inset 0 0 30px rgba(0, 255, 65, 0.15);
  }
}

@keyframes loadingPulse {
  0%, 100% {
    border-color: var(--border-color);
    box-shadow: none;
  }
  50% {
    border-color: var(--color-warning);
    box-shadow: 0 0 15px rgba(255, 193, 7, 0.3);
  }
}

.status-inner {
  text-align: center;
}

.status-icon {
  width: 48px;
  height: 48px;
  margin: 0 auto var(--spacing-xs);
  color: var(--text-muted);
  transition: color var(--transition-normal);
}

.status-ring:not(.active):hover:not(:disabled) .status-icon {
  color: var(--color-primary);
}

.status-ring.active .status-icon {
  color: var(--color-success);
}

.status-ring.active:hover:not(:disabled) .status-icon {
  color: var(--color-error);
}

.status-icon svg {
  width: 100%;
  height: 100%;
}

.status-text {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
  color: var(--text-secondary);
  transition: color var(--transition-fast);
}

.status-ring.active .status-text {
  color: var(--color-success);
}

.status-ring.active:hover:not(:disabled) .status-text {
  color: var(--color-error);
}

.status-ring.loading .status-text {
  color: var(--color-warning);
}

.status-hint {
  margin: var(--spacing-xs) 0 0 0;
  font-size: 0.7rem;
  color: var(--text-muted);
  opacity: 0;
  transition: opacity var(--transition-fast);
}

.status-ring:hover:not(:disabled) .status-hint {
  opacity: 1;
}

/* Spinner Large */
.spinner-large {
  display: block;
  width: 48px;
  height: 48px;
  border: 3px solid transparent;
  border-top-color: var(--color-warning);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

/* Server Info */
.server-info {
  width: 100%;
  max-width: 500px;
}

.info-card {
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--spacing-lg);
  transition: border-color var(--transition-fast);
}

.info-card:hover {
  border-color: var(--border-light);
}

.info-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-md);
}

.info-label {
  font-weight: 500;
  color: var(--text-primary);
}

.info-badge {
  font-size: 0.7rem;
  padding: 0.25rem 0.6rem;
  background: rgba(0, 255, 65, 0.1);
  color: var(--color-primary);
  border-radius: var(--radius-sm);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.url-display {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  background: var(--bg-primary);
  padding: var(--spacing-sm) var(--spacing-md);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-color);
}

.url-text {
  flex: 1;
  font-family: var(--font-mono);
  font-size: 0.85rem;
  color: var(--color-primary);
  word-break: break-all;
  background: transparent;
  padding: 0;
}

.url-actions {
  display: flex;
  gap: var(--spacing-xs);
  flex-shrink: 0;
}

.action-btn {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  padding: 0.4rem 0.7rem;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: 0.75rem;
  font-weight: 500;
  color: var(--text-secondary);
  transition: all var(--transition-fast);
}

.action-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
  border-color: var(--border-light);
}

.action-btn svg {
  flex-shrink: 0;
}

.info-hint {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  margin: var(--spacing-md) 0 0 0;
  font-size: 0.8rem;
  color: var(--text-muted);
}

/* Spinner */
@keyframes spin {
  to { transform: rotate(360deg); }
}

/* Error Message */
.error-message {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-md);
  background: rgba(255, 61, 61, 0.1);
  border: 1px solid rgba(255, 61, 61, 0.3);
  border-radius: var(--radius-md);
  color: var(--color-error);
  font-size: 0.9rem;
  max-width: 500px;
  width: 100%;
}

/* Responsive */
@media (max-width: 600px) {
  .status-ring {
    width: 140px;
    height: 140px;
  }
  
  .status-icon {
    width: 40px;
    height: 40px;
  }

  .spinner-large {
    width: 40px;
    height: 40px;
  }
  
  .url-display {
    flex-direction: column;
    gap: var(--spacing-md);
  }
  
  .url-actions {
    width: 100%;
  }
  
  .action-btn {
    flex: 1;
    justify-content: center;
  }
}
</style>

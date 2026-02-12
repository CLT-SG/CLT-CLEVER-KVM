<template>
  <div class="log-viewer">
    <div class="log-header">
      <h2 class="section-title">Application Logs</h2>
      <div class="log-actions">
        <button @click="refreshLogs" class="action-btn" :disabled="loading">
          <svg :class="{ spinning: loading }" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
            <path d="M23 4v6h-6" />
            <path d="M1 20v-6h6" />
            <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
          </svg>
          {{ loading ? 'Loading...' : 'Refresh' }}
        </button>
        <button @click="clearLogs" class="action-btn danger">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
            <polyline points="3 6 5 6 21 6" />
            <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
          </svg>
          Clear
        </button>
      </div>
    </div>
    
    <div class="logs-container">
      <!-- Error Log -->
      <div class="log-section">
        <div class="log-label">
          <span class="log-icon error">!</span>
          <span>Error Log</span>
          <span class="log-count" v-if="errorLines">{{ errorLines }} lines</span>
        </div>
        <div class="log-content">
          <pre v-if="errorLog">{{ errorLog }}</pre>
          <div v-else class="log-empty">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24">
              <circle cx="12" cy="12" r="10" />
              <path d="M8 14s1.5 2 4 2 4-2 4-2" />
              <line x1="9" y1="9" x2="9.01" y2="9" />
              <line x1="15" y1="9" x2="15.01" y2="9" />
            </svg>
            <span>No errors logged</span>
          </div>
        </div>
      </div>
      
      <!-- Debug Log -->
      <div class="log-section">
        <div class="log-label">
          <span class="log-icon debug">i</span>
          <span>Debug Log</span>
          <span class="log-count" v-if="debugLines">{{ debugLines }} lines</span>
        </div>
        <div class="log-content">
          <pre v-if="debugLog">{{ debugLog }}</pre>
          <div v-else class="log-empty">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <polyline points="14 2 14 8 20 8" />
              <line x1="16" y1="13" x2="8" y2="13" />
              <line x1="16" y1="17" x2="8" y2="17" />
              <polyline points="10 9 9 9 8 9" />
            </svg>
            <span>No debug logs available</span>
          </div>
        </div>
      </div>
      
      <!-- Log File Paths Info -->
      <div class="log-paths" v-if="logPaths.debug || logPaths.error">
        <div class="log-paths-header">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="16" x2="12" y2="12" />
            <line x1="12" y1="8" x2="12.01" y2="8" />
          </svg>
          <span>Log Files Location</span>
        </div>
        <div class="log-path-item" v-if="logPaths.debug">
          <span class="path-label">Debug:</span>
          <code class="path-value">{{ logPaths.debug }}</code>
        </div>
        <div class="log-path-item" v-if="logPaths.error">
          <span class="path-label">Error:</span>
          <code class="path-value">{{ logPaths.error }}</code>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue';
import { invoke } from "@tauri-apps/api/tauri";

const debugLog = ref("");
const errorLog = ref("");
const loading = ref(false);
const logPaths = ref({ debug: "", error: "" });

const debugLines = computed(() => debugLog.value ? debugLog.value.split('\n').filter(l => l.trim()).length : 0);
const errorLines = computed(() => errorLog.value ? errorLog.value.split('\n').filter(l => l.trim()).length : 0);

async function refreshLogs() {
  loading.value = true;
  try {
    const [debug, error] = await invoke("get_logs");
    debugLog.value = debug;
    errorLog.value = error;
    
    // Fetch log file paths for debugging
    try {
      const [debugPath, errorPath] = await invoke("get_log_file_paths");
      logPaths.value = { debug: debugPath, error: errorPath };
    } catch (pathError) {
      console.warn("Could not get log file paths:", pathError);
    }
  } catch (error) {
    console.error(`Failed to load logs: ${error}`);
    debugLog.value = `Failed to load logs: ${error}`;
    errorLog.value = "";
  } finally {
    loading.value = false;
  }
}

async function clearLogs() {
  try {
    // Clear logs on the backend (files and memory buffers)
    await invoke("clear_app_logs");
    // Clear the UI immediately
    debugLog.value = "Logs cleared successfully. New logs will appear here.";
    errorLog.value = "";
  } catch (error) {
    console.error(`Failed to clear logs: ${error}`);
    // Still clear UI even if backend clear fails
    debugLog.value = `Failed to clear logs on disk: ${error}`;
    errorLog.value = "";
  }
}

onMounted(() => {
  refreshLogs();
});
</script>

<style scoped>
.log-viewer {
  max-width: 100%;
}

.log-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-lg);
}

.section-title {
  margin: 0;
  font-size: 1.35rem;
  color: var(--text-primary);
}

.log-actions {
  display: flex;
  gap: var(--spacing-sm);
}

.action-btn {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  padding: 0.5rem 1rem;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  font-size: 0.85rem;
  font-family: inherit;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.action-btn:hover:not(:disabled) {
  color: var(--color-primary);
  border-color: var(--color-primary);
}

.action-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.action-btn.danger:hover:not(:disabled) {
  color: var(--color-error);
  border-color: var(--color-error);
}

.action-btn svg.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.logs-container {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-lg);
}

.log-section {
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.log-label {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-md);
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  font-weight: 500;
  color: var(--text-primary);
}

.log-icon {
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-full);
  font-size: 0.75rem;
  font-weight: 700;
}

.log-icon.error {
  background: rgba(255, 61, 61, 0.2);
  color: var(--color-error);
}

.log-icon.debug {
  background: rgba(0, 180, 255, 0.2);
  color: var(--color-info);
}

.log-count {
  margin-left: auto;
  font-size: 0.75rem;
  color: var(--text-muted);
  font-weight: normal;
}

.log-content {
  max-height: 300px;
  overflow-y: auto;
}

.log-content pre {
  margin: 0;
  padding: var(--spacing-md);
  font-family: var(--font-mono);
  font-size: 0.8rem;
  line-height: 1.5;
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-all;
}

.log-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-xl);
  color: var(--text-muted);
  gap: var(--spacing-sm);
}

.log-empty svg {
  opacity: 0.5;
}

.log-empty span {
  font-size: 0.9rem;
}

/* Responsive */
@media (max-width: 600px) {
  .log-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-md);
  }
  
  .log-actions {
    width: 100%;
  }
  
  .action-btn {
    flex: 1;
    justify-content: center;
  }
}

/* Log file paths info section */
.log-paths {
  margin-top: var(--spacing-md);
  padding: var(--spacing-md);
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  font-size: 0.75rem;
}

.log-paths-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  color: var(--text-muted);
  margin-bottom: var(--spacing-sm);
  font-weight: 500;
}

.log-path-item {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-xs);
}

.log-path-item:last-child {
  margin-bottom: 0;
}

.path-label {
  color: var(--text-muted);
  flex-shrink: 0;
}

.path-value {
  font-family: var(--font-mono);
  font-size: 0.7rem;
  color: var(--text-secondary);
  background: var(--bg-tertiary);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  word-break: break-all;
}
</style>

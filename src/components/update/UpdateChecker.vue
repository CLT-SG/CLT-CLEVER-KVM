<template>
  <div class="update-checker">
    <div class="update-card">
      <div class="update-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24">
          <path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8" />
          <path d="M21 3v5h-5" />
        </svg>
      </div>
      
      <div class="update-content">
        <h3 class="update-title">Software Updates</h3>
        <p class="update-description">Keep your application up to date for the best experience</p>
        
        <div v-if="lastChecked" class="last-checked">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14">
            <circle cx="12" cy="12" r="10" />
            <polyline points="12 6 12 12 16 14" />
          </svg>
          Last checked: {{ formatDate(lastChecked) }}
        </div>
      </div>

      <button 
        @click="checkForUpdates" 
        :disabled="isChecking"
        class="btn-check"
      >
        <svg v-if="isChecking" class="spinning" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
          <path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8" />
        </svg>
        <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
          <path d="M23 4v6h-6" />
          <path d="M1 20v-6h6" />
          <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
        </svg>
        {{ isChecking ? 'Checking...' : 'Check Now' }}
      </button>
    </div>
    
    <div v-if="updateStatus" class="update-status" :class="updateStatus.type">
      <div class="status-icon">
        <svg v-if="updateStatus.type === 'success'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
          <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10z" />
          <path d="M8 12l2.5 2.5L16 9" />
        </svg>
        <svg v-else-if="updateStatus.type === 'info'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
          <circle cx="12" cy="12" r="10" />
          <line x1="12" y1="16" x2="12" y2="12" />
          <line x1="12" y1="8" x2="12.01" y2="8" />
        </svg>
        <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
          <circle cx="12" cy="12" r="10" />
          <line x1="15" y1="9" x2="9" y2="15" />
          <line x1="9" y1="9" x2="15" y2="15" />
        </svg>
      </div>
      <span>{{ updateStatus.message }}</span>
    </div>
  </div>
</template>

<script>
import { ref } from 'vue'
import { checkUpdate } from '@tauri-apps/api/updater'

export default {
  name: 'UpdateChecker',
  emits: ['update-found'],
  setup(props, { emit }) {
    const isChecking = ref(false)
    const lastChecked = ref(null)
    const updateStatus = ref(null)

    const checkForUpdates = async () => {
      if (isChecking.value) return

      isChecking.value = true
      updateStatus.value = null

      try {
        console.log('Manually checking for updates...')
        const update = await checkUpdate()
        
        lastChecked.value = new Date()
        
        if (update.shouldUpdate) {
          updateStatus.value = {
            type: 'success',
            message: `Update available: v${update.manifest?.version}`
          }
          emit('update-found', update)
        } else {
          updateStatus.value = {
            type: 'info',
            message: 'You are running the latest version'
          }
        }
      } catch (error) {
        console.error('Failed to check for updates:', error)
        lastChecked.value = new Date()
        
        if (error.message && error.message.includes('Could not fetch update')) {
          updateStatus.value = {
            type: 'info',
            message: 'No updates available (offline or no releases)'
          }
        } else {
          updateStatus.value = {
            type: 'error',
            message: `Update check failed: ${error.message}`
          }
        }
      } finally {
        isChecking.value = false
      }
    }

    const formatDate = (date) => {
      return date.toLocaleString()
    }

    return {
      isChecking,
      lastChecked,
      updateStatus,
      checkForUpdates,
      formatDate
    }
  }
}
</script>

<style scoped>
.update-checker {
  max-width: 100%;
}

.update-card {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  padding: var(--spacing-lg);
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
}

.update-icon {
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 255, 65, 0.1);
  border-radius: var(--radius-md);
  color: var(--color-primary);
  flex-shrink: 0;
}

.update-content {
  flex: 1;
  min-width: 0;
}

.update-title {
  margin: 0 0 var(--spacing-xs) 0;
  font-size: 1rem;
  font-weight: 600;
  color: var(--text-primary);
}

.update-description {
  margin: 0;
  font-size: 0.85rem;
  color: var(--text-muted);
}

.last-checked {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  margin-top: var(--spacing-sm);
  font-size: 0.75rem;
  color: var(--text-muted);
}

.btn-check {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: 0.6rem 1.25rem;
  background: var(--color-primary);
  border: none;
  border-radius: var(--radius-md);
  color: var(--bg-primary);
  font-size: 0.85rem;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: all var(--transition-fast);
  white-space: nowrap;
}

.btn-check:hover:not(:disabled) {
  background: var(--color-primary-hover);
  transform: translateY(-1px);
}

.btn-check:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.btn-check svg.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.update-status {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-md);
  margin-top: var(--spacing-md);
  border-radius: var(--radius-md);
  font-size: 0.9rem;
}

.status-icon {
  flex-shrink: 0;
}

.update-status.success {
  background: rgba(0, 255, 65, 0.1);
  border: 1px solid rgba(0, 255, 65, 0.3);
  color: var(--color-primary);
}

.update-status.info {
  background: rgba(0, 180, 255, 0.1);
  border: 1px solid rgba(0, 180, 255, 0.3);
  color: var(--color-info);
}

.update-status.error {
  background: rgba(255, 61, 61, 0.1);
  border: 1px solid rgba(255, 61, 61, 0.3);
  color: var(--color-error);
}

/* Responsive */
@media (max-width: 600px) {
  .update-card {
    flex-direction: column;
    text-align: center;
  }
  
  .update-content {
    width: 100%;
  }
  
  .last-checked {
    justify-content: center;
  }
  
  .btn-check {
    width: 100%;
    justify-content: center;
  }
}
</style>

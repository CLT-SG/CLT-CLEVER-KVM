<template>
  <Transition name="modal">
    <div v-if="showUpdateDialog" class="update-dialog-overlay" @click.self="closeDialog">
      <div class="update-dialog">
        <div class="dialog-header">
          <div class="dialog-icon" :class="updateStatus">
            <svg v-if="updateStatus === 'available'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24">
              <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10z" />
              <path d="M12 8v4" />
              <path d="M12 16h.01" />
            </svg>
            <svg v-else-if="updateStatus === 'downloading'" class="spinning" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24">
              <path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8" />
            </svg>
            <svg v-else-if="updateStatus === 'ready'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24">
              <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10z" />
              <path d="M8 12l2.5 2.5L16 9" />
            </svg>
            <svg v-else-if="updateStatus === 'error'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24">
              <circle cx="12" cy="12" r="10" />
              <line x1="15" y1="9" x2="9" y2="15" />
              <line x1="9" y1="9" x2="15" y2="15" />
            </svg>
          </div>
          <h3>{{ updateTitle }}</h3>
        </div>
        
        <p class="dialog-message">{{ updateMessage }}</p>
        
        <div v-if="updateStatus === 'available'" class="update-actions">
          <button @click="installUpdate" class="btn-primary">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
            Install Update
          </button>
          <button @click="closeDialog" class="btn-secondary">
            Later
          </button>
        </div>
        
        <div v-if="updateStatus === 'downloading'" class="update-progress">
          <div class="progress-bar">
            <div class="progress-fill" :style="{ width: downloadProgress + '%' }"></div>
          </div>
          <div class="progress-info">
            <span>Downloading update...</span>
            <span class="progress-percent">{{ downloadProgress }}%</span>
          </div>
        </div>
        
        <div v-if="updateStatus === 'ready'" class="update-actions">
          <button @click="restartApp" class="btn-primary">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
              <path d="M23 4v6h-6" />
              <path d="M1 20v-6h6" />
              <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
            </svg>
            Restart & Install
          </button>
        </div>
        
        <div v-if="updateStatus === 'error'" class="update-error">
          <div class="error-message">{{ errorMessage }}</div>
          <button @click="closeDialog" class="btn-secondary">
            Close
          </button>
        </div>
        
        <button class="close-btn" @click="closeDialog">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>
    </div>
  </Transition>
</template>

<script>
import { ref, onMounted, onUnmounted } from 'vue'
import { checkUpdate, installUpdate } from '@tauri-apps/api/updater'
import { relaunch } from '@tauri-apps/api/process'

export default {
  name: 'UpdaterDialog',
  setup() {
    const showUpdateDialog = ref(false)
    const updateStatus = ref('checking')
    const updateTitle = ref('')
    const updateMessage = ref('')
    const downloadProgress = ref(0)
    const errorMessage = ref('')
    let unlisten = null

    const checkForUpdates = async () => {
      try {
        console.log('Checking for updates...')
        const update = await checkUpdate()
        
        if (update.shouldUpdate) {
          console.log('Update available:', update.manifest?.version)
          updateStatus.value = 'available'
          updateTitle.value = 'Update Available'
          updateMessage.value = `Version ${update.manifest?.version} is ready to install. This update includes bug fixes and improvements.`
          showUpdateDialog.value = true
        } else {
          console.log('App is up to date')
          updateStatus.value = 'none'
        }
      } catch (error) {
        console.error('Failed to check for updates:', error)
        if (error.message && !error.message.includes('Could not fetch update')) {
          updateStatus.value = 'error'
          updateTitle.value = 'Update Check Failed'
          errorMessage.value = error.message
          showUpdateDialog.value = true
        }
      }
    }

    const performUpdate = async () => {
      try {
        updateStatus.value = 'downloading'
        updateTitle.value = 'Downloading Update'
        updateMessage.value = 'Please wait while we download the latest version...'
        
        if (unlisten) unlisten()
        unlisten = await installUpdate()
        
        updateStatus.value = 'ready'
        updateTitle.value = 'Ready to Install'
        updateMessage.value = 'Update downloaded successfully. Restart the application to complete the installation.'
        
      } catch (error) {
        console.error('Failed to install update:', error)
        updateStatus.value = 'error'
        updateTitle.value = 'Update Failed'
        errorMessage.value = error.message
      }
    }

    const restartApp = async () => {
      try {
        await relaunch()
      } catch (error) {
        console.error('Failed to restart app:', error)
        updateStatus.value = 'error'
        errorMessage.value = 'Failed to restart the application'
      }
    }

    const closeDialog = () => {
      showUpdateDialog.value = false
      updateStatus.value = 'none'
    }

    onMounted(() => {
      setTimeout(checkForUpdates, 2000)
    })

    onUnmounted(() => {
      if (unlisten) {
        unlisten()
      }
    })

    return {
      showUpdateDialog,
      updateStatus,
      updateTitle,
      updateMessage,
      downloadProgress,
      errorMessage,
      installUpdate: performUpdate,
      restartApp,
      closeDialog,
      checkForUpdates
    }
  }
}
</script>

<style scoped>
.update-dialog-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.8);
  backdrop-filter: blur(4px);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 1000;
}

.update-dialog {
  position: relative;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-xl);
  padding: var(--spacing-xl);
  max-width: 420px;
  width: 90%;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
}

.dialog-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-md);
}

.dialog-icon {
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-full);
  flex-shrink: 0;
}

.dialog-icon.available {
  background: rgba(255, 177, 66, 0.15);
  color: var(--color-warning);
}

.dialog-icon.downloading {
  background: rgba(0, 180, 255, 0.15);
  color: var(--color-info);
}

.dialog-icon.ready {
  background: rgba(0, 255, 65, 0.15);
  color: var(--color-primary);
}

.dialog-icon.error {
  background: rgba(255, 61, 61, 0.15);
  color: var(--color-error);
}

.dialog-icon svg.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.update-dialog h3 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--text-primary);
}

.dialog-message {
  margin: 0 0 var(--spacing-lg) 0;
  color: var(--text-muted);
  line-height: 1.6;
  font-size: 0.95rem;
}

.update-actions {
  display: flex;
  gap: var(--spacing-sm);
  justify-content: flex-end;
}

.btn-primary,
.btn-secondary {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  padding: 0.65rem 1.25rem;
  border: none;
  border-radius: var(--radius-md);
  font-size: 0.9rem;
  font-weight: 500;
  font-family: inherit;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.btn-primary {
  background: var(--color-primary);
  color: var(--bg-primary);
}

.btn-primary:hover {
  background: var(--color-primary-hover);
  transform: translateY(-1px);
}

.btn-secondary {
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border: 1px solid var(--border-color);
}

.btn-secondary:hover {
  border-color: var(--border-light);
  color: var(--text-primary);
}

.update-progress {
  margin-bottom: var(--spacing-md);
}

.progress-bar {
  width: 100%;
  height: 6px;
  background: var(--bg-tertiary);
  border-radius: var(--radius-full);
  overflow: hidden;
  margin-bottom: var(--spacing-sm);
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--color-primary), var(--color-primary-hover));
  border-radius: var(--radius-full);
  transition: width 0.3s ease;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.85rem;
  color: var(--text-muted);
}

.progress-percent {
  font-family: var(--font-mono);
  color: var(--color-primary);
}

.update-error {
  text-align: center;
}

.error-message {
  background: rgba(255, 61, 61, 0.1);
  border: 1px solid rgba(255, 61, 61, 0.3);
  border-radius: var(--radius-md);
  padding: var(--spacing-md);
  color: var(--color-error);
  font-size: 0.9rem;
  margin-bottom: var(--spacing-md);
}

.close-btn {
  position: absolute;
  top: var(--spacing-md);
  right: var(--spacing-md);
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: var(--radius-md);
  color: var(--text-muted);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.close-btn:hover {
  background: var(--bg-tertiary);
  color: var(--text-primary);
}

/* Transitions */
.modal-enter-active,
.modal-leave-active {
  transition: all 0.3s ease;
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

.modal-enter-from .update-dialog,
.modal-leave-to .update-dialog {
  transform: scale(0.95) translateY(20px);
}
</style>

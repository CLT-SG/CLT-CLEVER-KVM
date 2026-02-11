<template>
  <div class="settings-panel">
    <!-- Application Updates Section -->
    <div class="settings-section updates-section">
      <div class="section-header">
        <div class="section-icon updates">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="7 10 12 15 17 10" />
            <line x1="12" y1="15" x2="12" y2="3" />
          </svg>
        </div>
        <div class="section-info">
          <h3>Application Updates</h3>
          <p>Check for and install application updates</p>
        </div>
      </div>
      <div class="update-checker-wrapper">
        <UpdateChecker />
      </div>
    </div>

    <div class="settings-header">
      <h2>Application Settings</h2>
      <p class="settings-description">Configure application behavior and preferences</p>
    </div>

    <!-- Startup Settings -->
    <div class="settings-section">
      <div class="section-header">
        <div class="section-icon startup">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polygon points="5 3 19 12 5 21 5 3" />
          </svg>
        </div>
        <div class="section-info">
          <h3>Startup</h3>
          <p>Configure how the application starts</p>
        </div>
      </div>
      
      <div class="settings-group">
        <div class="setting-item">
          <div class="setting-content">
            <label class="setting-label">Launch at system startup</label>
            <p class="setting-description">Automatically start Clever KVM when you log in</p>
          </div>
          <label class="toggle-switch">
            <input 
              type="checkbox" 
              v-model="autoStartEnabled"
              @change="toggleAutoStart"
            />
            <span class="toggle-slider"></span>
          </label>
        </div>

        <div class="setting-item">
          <div class="setting-content">
            <label class="setting-label">Start minimized to tray</label>
            <p class="setting-description">Start the application hidden in the system tray</p>
          </div>
          <label class="toggle-switch">
            <input 
              type="checkbox" 
              v-model="startMinimized"
              @change="saveSettings"
            />
            <span class="toggle-slider"></span>
          </label>
        </div>

        <div class="setting-item">
          <div class="setting-content">
            <label class="setting-label">Auto-start server</label>
            <p class="setting-description">Automatically start the KVM server when the application launches</p>
          </div>
          <label class="toggle-switch">
            <input 
              type="checkbox" 
              v-model="autoStartServer"
              @change="saveSettings"
            />
            <span class="toggle-slider"></span>
          </label>
        </div>
      </div>
    </div>

    <!-- System Tray Settings -->
    <div class="settings-section">
      <div class="section-header">
        <div class="section-icon tray">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" />
            <path d="M13.73 21a2 2 0 0 1-3.46 0" />
          </svg>
        </div>
        <div class="section-info">
          <h3>System Tray</h3>
          <p>Customize tray behavior and notifications</p>
        </div>
      </div>
      
      <div class="settings-group">
        <div class="setting-item">
          <div class="setting-content">
            <label class="setting-label">Show tray icon</label>
            <p class="setting-description">Display icon in the system tray area</p>
          </div>
          <label class="toggle-switch">
            <input 
              type="checkbox" 
              v-model="showTrayIcon"
              @change="saveSettings"
            />
            <span class="toggle-slider"></span>
          </label>
        </div>

        <div class="setting-item">
          <div class="setting-content">
            <label class="setting-label">Minimize to tray on close</label>
            <p class="setting-description">Keep running in background when window is closed</p>
          </div>
          <label class="toggle-switch">
            <input 
              type="checkbox" 
              v-model="minimizeToTray"
              @change="saveSettings"
            />
            <span class="toggle-slider"></span>
          </label>
        </div>

        <div class="setting-item">
          <div class="setting-content">
            <label class="setting-label">Show notifications</label>
            <p class="setting-description">Display system notifications for important events</p>
          </div>
          <label class="toggle-switch">
            <input 
              type="checkbox" 
              v-model="showNotifications"
              @change="saveSettings"
            />
            <span class="toggle-slider"></span>
          </label>
        </div>
      </div>
    </div>

    <!-- Status Message -->
    <transition name="fade">
      <div v-if="statusMessage" class="status-message" :class="statusType">
        <span class="status-icon">
          <svg v-if="statusType === 'success'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="20 6 9 17 4 12" />
          </svg>
          <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
        </span>
        {{ statusMessage }}
      </div>
    </transition>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import { invoke } from "@tauri-apps/api/tauri";
import UpdateChecker from '../update/UpdateChecker.vue';

// Settings state
const autoStartEnabled = ref(false);
const startMinimized = ref(false);
const autoStartServer = ref(true);
const showTrayIcon = ref(true);
const minimizeToTray = ref(true);
const showNotifications = ref(true);

// Status message
const statusMessage = ref('');
const statusType = ref('success');

let statusTimeout = null;

function showStatus(message, type = 'success') {
  statusMessage.value = message;
  statusType.value = type;
  
  if (statusTimeout) {
    clearTimeout(statusTimeout);
  }
  
  statusTimeout = setTimeout(() => {
    statusMessage.value = '';
  }, 3000);
}

async function loadSettings() {
  try {
    const settings = await invoke('get_app_settings');
    if (settings) {
      autoStartEnabled.value = settings.auto_start ?? false;
      startMinimized.value = settings.start_minimized ?? false;
      autoStartServer.value = settings.auto_start_server ?? true;
      showTrayIcon.value = settings.show_tray_icon ?? true;
      minimizeToTray.value = settings.minimize_to_tray ?? true;
      showNotifications.value = settings.show_notifications ?? true;
    }
  } catch (error) {
    console.warn('Could not load settings:', error);
    // Use defaults if settings command is not available
  }
}

async function saveSettings() {
  try {
    await invoke('save_app_settings', {
      settings: {
        auto_start: autoStartEnabled.value,
        start_minimized: startMinimized.value,
        auto_start_server: autoStartServer.value,
        show_tray_icon: showTrayIcon.value,
        minimize_to_tray: minimizeToTray.value,
        show_notifications: showNotifications.value
      }
    });
    showStatus('Settings saved successfully');
  } catch (error) {
    console.error('Failed to save settings:', error);
    showStatus('Failed to save settings', 'error');
  }
}

async function toggleAutoStart() {
  try {
    await invoke('set_auto_start', { enabled: autoStartEnabled.value });
    showStatus(autoStartEnabled.value ? 'Auto-start enabled' : 'Auto-start disabled');
    await saveSettings();
  } catch (error) {
    console.error('Failed to toggle auto-start:', error);
    showStatus('Failed to configure auto-start', 'error');
    // Revert the toggle
    autoStartEnabled.value = !autoStartEnabled.value;
  }
}

onMounted(() => {
  loadSettings();
});
</script>

<style scoped>
.settings-panel {
  max-width: 700px;
  margin: 0 auto;
}

.settings-header {
  margin-bottom: var(--spacing-xl);
}

.settings-header h2 {
  margin: 0 0 var(--spacing-xs) 0;
  font-size: 1.5rem;
  color: var(--text-primary);
}

.settings-description {
  margin: 0;
  color: var(--text-muted);
  font-size: 0.9rem;
}

/* Updates Section */
.updates-section {
  margin-bottom: var(--spacing-xl);
}

.update-checker-wrapper {
  padding-top: var(--spacing-sm);
}

/* Section Styles */
.settings-section {
  background: var(--bg-card);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--spacing-lg);
  margin-bottom: var(--spacing-lg);
  transition: border-color var(--transition-fast);
}

.settings-section:hover {
  border-color: var(--border-light);
}

.section-header {
  display: flex;
  align-items: flex-start;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-lg);
  padding-bottom: var(--spacing-md);
  border-bottom: 1px solid var(--border-color);
}

.section-icon {
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-md);
  flex-shrink: 0;
}

.section-icon svg {
  width: 20px;
  height: 20px;
}

.section-icon.startup {
  background: rgba(0, 255, 65, 0.15);
  color: var(--color-primary);
}

.section-icon.updates {
  background: rgba(136, 71, 255, 0.15);
  color: #8847ff;
}

.section-icon.tray {
  background: rgba(255, 177, 66, 0.15);
  color: var(--color-warning);
}

.section-info h3 {
  margin: 0 0 var(--spacing-xs) 0;
  font-size: 1.1rem;
  color: var(--text-primary);
}

.section-info p {
  margin: 0;
  font-size: 0.85rem;
  color: var(--text-muted);
}

/* Settings Group */
.settings-group {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-md);
}

/* Setting Item */
.setting-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--spacing-md);
  background: var(--bg-tertiary);
  border-radius: var(--radius-md);
  transition: background var(--transition-fast);
}

.setting-item:hover {
  background: var(--bg-hover);
}

.setting-content {
  flex: 1;
  min-width: 0;
}

.setting-label {
  display: block;
  font-weight: 500;
  color: var(--text-primary);
  margin-bottom: var(--spacing-xs);
}

.setting-description {
  margin: 0;
  font-size: 0.8rem;
  color: var(--text-muted);
  line-height: 1.4;
}

/* Toggle Switch */
.toggle-switch {
  position: relative;
  display: inline-block;
  width: 48px;
  height: 26px;
  flex-shrink: 0;
  margin-left: var(--spacing-md);
}

.toggle-switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.toggle-slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: var(--bg-tertiary);
  border: 1px solid var(--border-light);
  border-radius: 26px;
  transition: all var(--transition-normal);
}

.toggle-slider:before {
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 3px;
  bottom: 3px;
  background-color: var(--text-muted);
  border-radius: 50%;
  transition: all var(--transition-normal);
}

.toggle-switch input:checked + .toggle-slider {
  background-color: rgba(0, 255, 65, 0.2);
  border-color: var(--color-primary);
}

.toggle-switch input:checked + .toggle-slider:before {
  background-color: var(--color-primary);
  transform: translateX(22px);
  box-shadow: 0 0 10px var(--color-primary);
}

.toggle-switch input:focus + .toggle-slider {
  box-shadow: 0 0 0 3px var(--color-primary-glow);
}

/* Status Message */
.status-message {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-md);
  border-radius: var(--radius-md);
  font-size: 0.9rem;
  margin-top: var(--spacing-lg);
}

.status-message.success {
  background: rgba(0, 255, 65, 0.1);
  color: var(--color-success);
  border: 1px solid rgba(0, 255, 65, 0.3);
}

.status-message.error {
  background: rgba(255, 61, 61, 0.1);
  color: var(--color-error);
  border: 1px solid rgba(255, 61, 61, 0.3);
}

.status-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
}

.status-icon svg {
  width: 18px;
  height: 18px;
}

/* Fade transition */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Responsive */
@media (max-width: 600px) {
  .setting-item {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-md);
  }
  
  .toggle-switch {
    margin-left: 0;
  }
}
</style>

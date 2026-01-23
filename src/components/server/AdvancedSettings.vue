<template>
  <div class="advanced-toggle">
    <button @click="showAdvancedSettings = !showAdvancedSettings" class="text-button">
      {{ showAdvancedSettings ? '⬆️ Hide Advanced Settings' : '⬇️ Show Advanced Settings' }}
    </button>
  </div>
  
  <div v-if="showAdvancedSettings" class="advanced-settings" :class="{ disabled: disabled }">
    <div class="setting-group">
      <h4>VNC Audio Settings</h4>
      <label>
        <input type="checkbox" v-model="settings.enableAudio" :disabled="disabled" @change="$emit('settings-changed')" />
        Enable Audio Streaming (via RTSP on port {{ settings.audioPort }})
      </label>
      
      <div v-if="settings.enableAudio" class="slider-group">
        <label for="audio-port">Audio Port:</label>
        <input type="number" id="audio-port" v-model.number="settings.audioPort"
               min="1024" max="65535" :disabled="disabled" @change="$emit('settings-changed')" />
      </div>
    </div>
    
    <div class="help-text">
      <p><strong>Note:</strong> VNC protocol (RFB 3.8) is used for video streaming. Audio is streamed separately via RTSP.</p>
      <p>Connect using any VNC client such as TigerVNC, RealVNC, or VNC Viewer.</p>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue';

defineProps({
  settings: Object,
  disabled: {
    type: Boolean,
    default: false
  }
});

defineEmits(['settings-changed']);

const showAdvancedSettings = ref(false);
</script>

<style scoped>
.advanced-toggle {
  margin: 1rem 0;
  display: flex;
  align-items: center;
}

.text-button {
  background: none;
  border: none;
  color: #3498db;
  cursor: pointer;
  font-size: 0.9rem;
  padding: 0;
  text-decoration: underline;
}

.advanced-settings {
  background-color: #f8f9fa;
  border-radius: 4px;
  padding: 1rem;
  margin-bottom: 1rem;
}

.setting-group {
  margin-bottom: 0.75rem;
}

.setting-group label {
  display: flex;
  align-items: center;
  cursor: pointer;
}

.setting-group input[type="checkbox"] {
  margin-right: 0.5rem;
}

.slider-group {
  margin-bottom: 12px;
}

.slider-group label {
  display: block;
  margin-bottom: 4px;
}

.slider-group input[type="range"] {
  width: 100%;
}

.slider-group input[type="number"] {
  width: 120px;
  padding: 0.4rem;
  border: 1px solid #ddd;
  border-radius: 4px;
}

.help-text {
  margin-top: 1rem;
  padding: 1rem;
  background-color: #e7f3ff;
  border-left: 3px solid #0066cc;
  border-radius: 4px;
}

.help-text p {
  margin: 0.5rem 0;
  font-size: 0.9rem;
  color: #495057;
}

.help-text strong {
  color: #0066cc;
}

h4 {
  color: #2c3e50;
  font-size: 1rem;
  margin-top: 1rem;
  margin-bottom: 0.5rem;
}

.advanced-settings.disabled {
  opacity: 0.6;
  pointer-events: none;
}

input:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
</style>

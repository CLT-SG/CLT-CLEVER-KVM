<template>
  <div class="advanced-toggle">
    <button @click="showAdvancedSettings = !showAdvancedSettings" class="text-button">
      {{ showAdvancedSettings ? '⬆️ Hide Advanced Settings' : '⬇️ Show Advanced Settings' }}
    </button>
  </div>
  
  <div v-if="showAdvancedSettings" class="advanced-settings" :class="{ disabled: disabled }">
    <div class="setting-group">
      <h4>Video Codec</h4>
      <div class="codec-info">
        <span class="codec-badge">VP9</span>
        <span class="codec-description">Software encoding via libvpx (VP8/VP9)</span>
      </div>
    </div>
    
    <div class="setting-group">
      <h4>Bitrate &amp; Quality</h4>
      <div class="slider-group">
        <label for="video-bitrate">Video Bitrate: {{ settings.bitrate }} kbps</label>
        <input type="range" id="video-bitrate" v-model.number="settings.bitrate"
               min="1000" max="12000" step="500" :disabled="disabled" />
      </div>
      
      <div class="slider-group">
        <label for="framerate">Framerate: {{ settings.fps }} FPS</label>
        <input type="range" id="framerate" v-model.number="settings.fps"
               min="15" max="60" step="5" :disabled="disabled" />
      </div>
    </div>

    <div class="setting-group">
      <h4>Features</h4>
      <label>
        <input type="checkbox" v-model="settings.allowRemoteInput" :disabled="disabled" />
        Allow Remote Input (keyboard &amp; mouse)
      </label>
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

.codec-info {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.5rem;
  background-color: #e8f4fd;
  border-radius: 4px;
  border-left: 3px solid #3498db;
}

.codec-badge {
  background-color: #3498db;
  color: white;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  font-weight: bold;
  font-size: 0.85rem;
}

.codec-description {
  color: #555;
  font-size: 0.85rem;
}
</style>

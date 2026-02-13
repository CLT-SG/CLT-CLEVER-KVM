<template>
  <div class="advanced-settings-container">
    <button @click="showAdvancedSettings = !showAdvancedSettings" class="toggle-btn">
      <svg :class="{ rotated: showAdvancedSettings }" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
        <polyline points="6 9 12 15 18 9" />
      </svg>
      {{ showAdvancedSettings ? 'Hide Advanced Settings' : 'Show Advanced Settings' }}
    </button>
    
    <transition name="slide">
      <div v-if="showAdvancedSettings" class="advanced-settings" :class="{ disabled: disabled }">
        <!-- Video Codec -->
        <div class="setting-section">
          <h4 class="section-title">Video Codec</h4>
          <div class="codec-info">
            <span class="codec-badge">VP9</span>
            <span class="codec-description">Software encoding via libvpx (VP8/VP9)</span>
          </div>
        </div>
        
        <!-- Bitrate & Quality -->
        <div class="setting-section">
          <h4 class="section-title">Bitrate & Quality</h4>
          
          <div class="slider-control">
            <div class="slider-header">
              <label for="video-bitrate">Video Bitrate</label>
              <span class="slider-value">{{ settings.bitrate }} kbps</span>
            </div>
            <input 
              type="range" 
              id="video-bitrate" 
              v-model.number="settings.bitrate"
              min="1000" 
              max="12000" 
              step="500" 
              :disabled="disabled"
              class="slider"
            />
            <div class="slider-labels">
              <span>1000</span>
              <span>12000</span>
            </div>
          </div>
          
          <div class="slider-control">
            <div class="slider-header">
              <label for="framerate">Framerate</label>
              <span class="slider-value">{{ settings.fps }} FPS</span>
            </div>
            <input 
              type="range" 
              id="framerate" 
              v-model.number="settings.fps"
              min="15" 
              max="60" 
              step="5" 
              :disabled="disabled"
              class="slider"
            />
            <div class="slider-labels">
              <span>15</span>
              <span>60</span>
            </div>
          </div>
        </div>

        <!-- Features -->
        <div class="setting-section">
          <h4 class="section-title">Features</h4>
          <label class="checkbox-control">
            <input 
              type="checkbox" 
              v-model="settings.allowRemoteInput" 
              :disabled="disabled"
            />
            <span class="checkbox-custom"></span>
            <span class="checkbox-label">Allow Remote Input (keyboard & mouse)</span>
          </label>
        </div>
      </div>
    </transition>
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
.advanced-settings-container {
  margin-top: var(--spacing-md);
}

.toggle-btn {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-sm) var(--spacing-md);
  background: transparent;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  font-size: 0.9rem;
  font-family: inherit;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.toggle-btn:hover {
  color: var(--color-primary);
  border-color: var(--color-primary);
  background: rgba(0, 255, 65, 0.05);
}

.toggle-btn svg {
  transition: transform var(--transition-fast);
}

.toggle-btn svg.rotated {
  transform: rotate(180deg);
}

.advanced-settings {
  margin-top: var(--spacing-lg);
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--spacing-lg);
}

.advanced-settings.disabled {
  opacity: 0.6;
  pointer-events: none;
}

.setting-section {
  margin-bottom: var(--spacing-xl);
}

.setting-section:last-child {
  margin-bottom: 0;
}

.section-title {
  color: var(--text-primary);
  font-size: 0.95rem;
  font-weight: 600;
  margin: 0 0 var(--spacing-md) 0;
  padding-bottom: var(--spacing-sm);
  border-bottom: 1px solid var(--border-color);
}

/* Codec Info */
.codec-info {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  padding: var(--spacing-md);
  background: var(--bg-primary);
  border-radius: var(--radius-md);
  border-left: 3px solid var(--color-primary);
}

.codec-badge {
  background: var(--color-primary);
  color: var(--bg-primary);
  padding: 0.25rem 0.6rem;
  border-radius: var(--radius-sm);
  font-weight: 600;
  font-size: 0.8rem;
}

.codec-description {
  color: var(--text-secondary);
  font-size: 0.85rem;
}

/* Slider Controls */
.slider-control {
  margin-bottom: var(--spacing-lg);
}

.slider-control:last-child {
  margin-bottom: 0;
}

.slider-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-sm);
}

.slider-header label {
  color: var(--text-secondary);
  font-size: 0.9rem;
}

.slider-value {
  color: var(--color-primary);
  font-family: var(--font-mono);
  font-size: 0.9rem;
  font-weight: 500;
}

.slider {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 6px;
  background: var(--bg-primary);
  border-radius: var(--radius-sm);
  outline: none;
  cursor: pointer;
}

.slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: var(--color-primary);
  cursor: pointer;
  transition: box-shadow var(--transition-fast);
}

.slider::-webkit-slider-thumb:hover {
  box-shadow: 0 0 12px var(--color-primary);
}

.slider::-moz-range-thumb {
  width: 18px;
  height: 18px;
  border: none;
  border-radius: 50%;
  background: var(--color-primary);
  cursor: pointer;
}

.slider:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.slider-labels {
  display: flex;
  justify-content: space-between;
  margin-top: var(--spacing-xs);
  font-size: 0.75rem;
  color: var(--text-muted);
}

/* Checkbox Control */
.checkbox-control {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  cursor: pointer;
  user-select: none;
  padding: var(--spacing-sm);
  border-radius: var(--radius-md);
  transition: background var(--transition-fast);
}

.checkbox-control:hover {
  background: var(--bg-hover);
}

.checkbox-control input {
  position: absolute;
  opacity: 0;
  width: 0;
  height: 0;
}

.checkbox-custom {
  width: 20px;
  height: 20px;
  border: 2px solid var(--border-light);
  border-radius: var(--radius-sm);
  background: var(--bg-primary);
  transition: all var(--transition-fast);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.checkbox-control input:checked + .checkbox-custom {
  background: var(--color-primary);
  border-color: var(--color-primary);
}

.checkbox-control input:checked + .checkbox-custom::after {
  content: '';
  width: 6px;
  height: 10px;
  border: solid var(--bg-primary);
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
  margin-bottom: 2px;
}

.checkbox-label {
  color: var(--text-secondary);
  font-size: 0.9rem;
}

/* Slide transition */
.slide-enter-active,
.slide-leave-active {
  transition: all 0.3s ease;
  overflow: hidden;
}

.slide-enter-from,
.slide-leave-to {
  opacity: 0;
  max-height: 0;
  margin-top: 0;
}

.slide-enter-to,
.slide-leave-from {
  opacity: 1;
  max-height: 500px;
}
</style>

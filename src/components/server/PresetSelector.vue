<template>
  <div class="presets-section">
    <button @click="showPresets = !showPresets" class="text-button">
      {{ showPresets ? '⬆️ Hide Presets' : '⬇️ Show Presets' }}
    </button>
    
    <div v-if="showPresets" class="presets-container">
      <button 
        v-for="(preset, key) in presetOptions"
        :key="key"
        @click="!disabled && $emit('apply-preset', key)" 
        class="preset-button"
        :class="{ 'active-preset': selectedPreset === key, 'disabled': disabled }"
        :disabled="disabled">
        {{ preset.name }}
      </button>
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

defineEmits(['apply-preset']);

const showPresets = ref(false);
const selectedPreset = ref('desktop');

const presetOptions = {
  gaming: { name: 'Gaming (60 FPS)' },
  desktop: { name: 'Desktop (Balanced)' },
  lowBandwidth: { name: 'Low Bandwidth' }
};
</script>

<style scoped>
.presets-section {
  margin: 1rem 0;
}

.presets-container {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 8px;
}

.preset-button {
  background-color: #f8f9fa;
  border: 1px solid #ddd;
  border-radius: 4px;
  padding: 8px 12px;
  cursor: pointer;
  transition: all 0.2s;
}

.preset-button:hover {
  background-color: #e9ecef;
}

.preset-button.disabled,
.preset-button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
  pointer-events: none;
}

.active-preset {
  background-color: #3498db;
  color: white;
  border-color: #2980b9;
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
</style>

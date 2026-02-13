<template>
  <div class="server-config" :class="{ disabled: disabled }">
    <!-- Port Configuration -->
    <div class="config-card">
      <div class="config-row">
        <div class="config-label">
          <label for="port">Server Port</label>
          <span class="label-hint">Range: 1024 - 65535</span>
        </div>
        <input 
          id="port" 
          :value="serverPort" 
          @input="$emit('update:server-port', parseInt($event.target.value))"
          type="number" 
          min="1024" 
          max="65535"
          :disabled="disabled"
          class="config-input"
        />
      </div>
    </div>
    
    <!-- Display Selection -->
    <div class="config-card" v-if="displays.length > 0">
      <div class="config-row">
        <div class="config-label">
          <label for="display">Display Monitor</label>
          <span class="label-hint">Select which screen to share</span>
        </div>
        <select 
          id="display" 
          :value="settings.selectedWebrtcDisplay"
          @change="$emit('update:selected-display', parseInt($event.target.value))"
          :disabled="disabled"
          class="config-select"
        >
          <option v-for="(display, index) in displays" :key="index" :value="index">
            {{ display.name }} {{ display.is_primary ? '(Primary)' : '' }} - {{ display.width }}x{{ display.height }}
          </option>
        </select>
      </div>
    </div>
    
    <!-- Preset Selector -->
    <PresetSelector 
      :settings="settings"
      :disabled="disabled"
      @apply-preset="$emit('apply-preset', $event)"
    />
    
    <!-- Advanced Settings -->
    <AdvancedSettings 
      :settings="settings" 
      :disabled="disabled"
    />
  </div>
</template>

<script setup>
import PresetSelector from './PresetSelector.vue';
import AdvancedSettings from './AdvancedSettings.vue';

defineProps({
  serverPort: Number,
  settings: Object,
  displays: {
    type: Array,
    default: () => []
  },
  disabled: {
    type: Boolean,
    default: false
  }
});

defineEmits(['apply-preset', 'update:server-port', 'update:selected-display']);
</script>

<style scoped>
.server-config {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-lg);
}

.server-config.disabled {
  opacity: 0.6;
  pointer-events: none;
}

.config-card {
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--spacing-lg);
  transition: border-color var(--transition-fast);
}

.config-card:hover {
  border-color: var(--border-light);
}

.config-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--spacing-lg);
}

.config-label {
  flex: 1;
}

.config-label label {
  display: block;
  font-weight: 500;
  color: var(--text-primary);
  margin-bottom: var(--spacing-xs);
}

.label-hint {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.config-input,
.config-select {
  padding: 0.65rem 1rem;
  font-size: 0.9rem;
  font-family: var(--font-mono);
  color: var(--text-primary);
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  min-width: 200px;
}

.config-input:focus,
.config-select:focus {
  outline: none;
  border-color: var(--color-primary);
  box-shadow: 0 0 0 3px var(--color-primary-glow);
}

.config-input:disabled,
.config-select:disabled {
  background: var(--bg-secondary);
  color: var(--text-muted);
  cursor: not-allowed;
}

.config-select {
  cursor: pointer;
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%2300ff41' d='M6 8L1 3h10z'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 1rem center;
  padding-right: 2.5rem;
}

.config-select option {
  background: var(--bg-card);
  color: var(--text-primary);
}

/* Responsive */
@media (max-width: 600px) {
  .config-row {
    flex-direction: column;
    align-items: stretch;
    gap: var(--spacing-md);
  }
  
  .config-input,
  .config-select {
    min-width: 100%;
  }
}
</style>

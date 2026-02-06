<template>
  <div class="server-config" :class="{ disabled: disabled }">
    <div class="form-group">
      <label for="port">Port:</label>
      <input 
        id="port" 
        :value="serverPort" 
        @input="$emit('update:server-port', parseInt($event.target.value))"
        type="number" 
        min="1024" 
        max="65535"
        :disabled="disabled"
      />
    </div>
    
    <div class="form-group" v-if="displays.length > 0">
      <label for="display">Display:</label>
      <select 
        id="display" 
        :value="settings.selectedWebrtcDisplay"
        @change="$emit('update:selected-display', parseInt($event.target.value))"
        :disabled="disabled"
      >
        <option v-for="(display, index) in displays" :key="index" :value="index">
          {{ display.name }} {{ display.is_primary ? '(Primary)' : '' }} - {{ display.width }}x{{ display.height }}
        </option>
      </select>
    </div>
    
    <PresetSelector 
      :settings="settings"
      :disabled="disabled"
      @apply-preset="$emit('apply-preset', $event)"
    />
    
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
  displays: Array,
  disabled: {
    type: Boolean,
    default: false
  }
});

defineEmits(['apply-preset', 'update:server-port', 'update:selected-display']);
</script>

<style scoped>
.server-config {
  margin-top: 1rem;
}

.form-group {
  margin-bottom: 1rem;
  display: flex;
  align-items: center;
}

.form-group label {
  margin-right: 1rem;
  min-width: 60px;
}

input[type="number"], select {
  padding: 0.5rem;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 1rem;
  min-width: 200px;
}

.server-config.disabled {
  opacity: 0.6;
  pointer-events: none;
}

input:disabled, select:disabled {
  background-color: #f5f5f5;
  cursor: not-allowed;
}
</style>

<template>
  <div class="server-config" :class="{ disabled: disabled }">
    <div class="form-group checkbox-group">
      <label class="checkbox-label">
        <input 
          type="checkbox" 
          v-model="settings.autoStart"
          @change="$emit('settings-changed')"
          :disabled="disabled"
        />
        <span>Auto-start VNC server on application launch</span>
      </label>
    </div>
    
    <div class="form-group">
      <label for="port">VNC Port:</label>
      <input 
        id="port" 
        :value="serverPort" 
        @input="$emit('update:server-port', parseInt($event.target.value))"
        type="number" 
        min="1024" 
        max="65535"
        :disabled="disabled"
      />
      <span class="help-text">Default: 5900</span>
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
    
    <AdvancedSettings 
      :settings="settings" 
      :disabled="disabled"
      :scanningMediaMtx="scanningMediaMtx"
      :mediamtxServers="mediamtxServers"
      @settings-changed="$emit('settings-changed')"
      @scan-mediamtx="$emit('scan-mediamtx')"
    />
  </div>
</template>

<script setup>
import AdvancedSettings from './AdvancedSettings.vue';

defineProps({
  serverPort: Number,
  settings: Object,
  displays: Array,
  disabled: {
    type: Boolean,
    default: false
  },
  scanningMediaMtx: {
    type: Boolean,
    default: false
  },
  mediamtxServers: {
    type: Array,
    default: () => []
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
  min-width: 100px;
  font-weight: 500;
}

.checkbox-group {
  margin-bottom: 1.5rem;
  padding: 1rem;
  background-color: #e7f3ff;
  border-left: 3px solid #0066cc;
  border-radius: 4px;
}

.checkbox-label {
  display: flex;
  align-items: center;
  cursor: pointer;
  user-select: none;
  margin: 0;
}

.checkbox-label input[type="checkbox"] {
  width: 18px;
  height: 18px;
  margin-right: 0.75rem;
  cursor: pointer;
}

.checkbox-label span {
  font-weight: 500;
  color: #0066cc;
}

.help-text {
  margin-left: 0.5rem;
  font-size: 0.85rem;
  color: #6c757d;
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

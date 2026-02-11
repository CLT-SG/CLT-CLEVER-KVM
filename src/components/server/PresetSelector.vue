<template>
  <div class="preset-selector">
    <div class="preset-header">
      <h2 class="section-title">Quality Presets</h2>
      <p class="section-description">Choose a preset for optimal streaming performance</p>
    </div>
    
    <div class="presets-grid">
      <div
        v-for="preset in presets"
        :key="preset.id"
        class="preset-card"
        :class="{ active: selectedPreset === preset.id }"
        @click="selectPreset(preset)"
      >
        <div class="preset-icon">
          <svg v-if="preset.id === 'low'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24">
            <path d="M22 12h-4l-3 9L9 3l-3 9H2" />
          </svg>
          <svg v-else-if="preset.id === 'medium'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24">
            <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
            <path d="M7 11V7a5 5 0 0 1 10 0v4" />
          </svg>
          <svg v-else-if="preset.id === 'high'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24">
            <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
          </svg>
          <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="24" height="24">
            <circle cx="12" cy="12" r="10" />
            <polygon points="10 8 16 12 10 16 10 8" />
          </svg>
        </div>
        <div class="preset-info">
          <h3 class="preset-name">{{ preset.name }}</h3>
          <p class="preset-description">{{ preset.description }}</p>
        </div>
        <div class="preset-specs">
          <div class="spec">
            <span class="spec-label">Resolution</span>
            <span class="spec-value">{{ preset.resolution }}</span>
          </div>
          <div class="spec">
            <span class="spec-label">FPS</span>
            <span class="spec-value">{{ preset.fps }}</span>
          </div>
          <div class="spec">
            <span class="spec-label">Bitrate</span>
            <span class="spec-value">{{ preset.bitrate }}</span>
          </div>
        </div>
        <div class="preset-check" v-if="selectedPreset === preset.id">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" width="16" height="16">
            <polyline points="20 6 9 17 4 12" />
          </svg>
        </div>
      </div>
    </div>
    
    <div class="preset-actions">
      <button @click="applyPreset" class="btn-apply" :disabled="!selectedPreset">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
          <polyline points="20 6 9 17 4 12" />
        </svg>
        Apply Preset
      </button>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue';

const emit = defineEmits(['preset-applied']);

const selectedPreset = ref(null);

const presets = [
  {
    id: 'low',
    name: 'Low Bandwidth',
    description: 'Optimized for slower connections',
    resolution: '1280x720',
    fps: '30',
    bitrate: '2 Mbps'
  },
  {
    id: 'medium',
    name: 'Balanced',
    description: 'Good balance of quality and performance',
    resolution: '1920x1080',
    fps: '30',
    bitrate: '5 Mbps'
  },
  {
    id: 'high',
    name: 'High Quality',
    description: 'Best quality for fast connections',
    resolution: '1920x1080',
    fps: '60',
    bitrate: '10 Mbps'
  },
  {
    id: 'ultra',
    name: 'Ultra',
    description: 'Maximum quality, requires excellent connection',
    resolution: '2560x1440',
    fps: '60',
    bitrate: '20 Mbps'
  }
];

function selectPreset(preset) {
  selectedPreset.value = preset.id;
}

function applyPreset() {
  const preset = presets.find(p => p.id === selectedPreset.value);
  if (preset) {
    emit('preset-applied', preset);
  }
}
</script>

<style scoped>
.preset-selector {
  max-width: 100%;
}

.preset-header {
  margin-bottom: var(--spacing-lg);
}

.section-title {
  margin: 0 0 var(--spacing-xs) 0;
  font-size: 1.35rem;
  color: var(--text-primary);
}

.section-description {
  margin: 0;
  color: var(--text-muted);
  font-size: 0.9rem;
}

.presets-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-lg);
}

.preset-card {
  position: relative;
  background: var(--bg-tertiary);
  border: 2px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--spacing-lg);
  cursor: pointer;
  transition: all var(--transition-normal);
}

.preset-card:hover {
  border-color: var(--border-light);
  transform: translateY(-2px);
}

.preset-card.active {
  border-color: var(--color-primary);
  background: rgba(0, 255, 65, 0.05);
}

.preset-icon {
  width: 48px;
  height: 48px;
  background: var(--bg-secondary);
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: var(--spacing-md);
  color: var(--text-muted);
  transition: all var(--transition-fast);
}

.preset-card:hover .preset-icon,
.preset-card.active .preset-icon {
  color: var(--color-primary);
  background: rgba(0, 255, 65, 0.1);
}

.preset-info {
  margin-bottom: var(--spacing-md);
}

.preset-name {
  margin: 0 0 var(--spacing-xs) 0;
  font-size: 1.1rem;
  font-weight: 600;
  color: var(--text-primary);
}

.preset-description {
  margin: 0;
  font-size: 0.85rem;
  color: var(--text-muted);
  line-height: 1.4;
}

.preset-specs {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-xs);
  padding-top: var(--spacing-md);
  border-top: 1px solid var(--border-color);
}

.spec {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.spec-label {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.spec-value {
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--text-secondary);
  font-family: var(--font-mono);
}

.preset-check {
  position: absolute;
  top: var(--spacing-md);
  right: var(--spacing-md);
  width: 24px;
  height: 24px;
  background: var(--color-primary);
  border-radius: var(--radius-full);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--bg-primary);
}

.preset-actions {
  display: flex;
  justify-content: flex-end;
}

.btn-apply {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: 0.75rem 1.5rem;
  background: var(--color-primary);
  border: none;
  border-radius: var(--radius-md);
  color: var(--bg-primary);
  font-size: 0.9rem;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.btn-apply:hover:not(:disabled) {
  background: var(--color-primary-hover);
  transform: translateY(-1px);
}

.btn-apply:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Responsive */
@media (max-width: 480px) {
  .presets-grid {
    grid-template-columns: 1fr;
  }
  
  .preset-actions {
    justify-content: stretch;
  }
  
  .btn-apply {
    width: 100%;
    justify-content: center;
  }
}
</style>

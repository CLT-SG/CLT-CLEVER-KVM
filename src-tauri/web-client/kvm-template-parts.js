// Template initialization and utility functions
class TemplateInitializer {
    static initializeToggles(config) {
        document.querySelectorAll('.toggle-switch').forEach(toggle => {
            const setting = toggle.dataset.setting;
            const checkbox = toggle.querySelector('input');
            
            // Set initial state based on config
            switch (setting) {
                case 'stretch':
                    checkbox.checked = config.stretch;
                    break;
                case 'audio':
                    checkbox.checked = config.audio;
                    break;
                case 'mute':
                    checkbox.checked = config.mute;
                    break;
                case 'stats':
                    checkbox.checked = false; // Default to false
                    break;
            }
            
            // Add click handler
            toggle.addEventListener('click', () => {
                checkbox.checked = !checkbox.checked;
                toggle.classList.toggle('active', checkbox.checked);
                
                // Trigger change event for any listeners
                checkbox.dispatchEvent(new Event('change'));
            });
            
            // Initialize visual state
            toggle.classList.toggle('active', checkbox.checked);
        });
    }
    
    static initializeDropdowns(config) {
        // Initialize monitor dropdown (will be populated by server data)
        const monitorDropdown = document.getElementById('monitor-dropdown');
        if (monitorDropdown) {
            // This will be updated when server info is received
            monitorDropdown.innerHTML = '<option value="0">Primary Monitor</option>';
        }
        
        // Initialize codec dropdown with correct value
        const codecDropdown = document.getElementById('codec-dropdown');
        if (codecDropdown && config.codec) {
            // H.264 is the only supported codec
            codecDropdown.value = 'h264';
        }
    }
    
    static initializeQualitySlider() {
        const qualitySlider = document.getElementById('quality-slider');
        const qualityValue = document.getElementById('quality-value');
        
        if (qualitySlider && qualityValue) {
            qualitySlider.addEventListener('input', (e) => {
                qualityValue.textContent = e.target.value;
            });
            
            // Set initial value
            qualityValue.textContent = qualitySlider.value;
        }
    }
    
    static initializeKeyboardShortcuts() {
        document.addEventListener('keydown', (e) => {
            // Global keyboard shortcuts
            if (e.key === 'F11') {
                e.preventDefault();
                // Trigger fullscreen toggle
                const fullscreenBtn = document.getElementById('fullscreen-btn');
                if (fullscreenBtn) {
                    fullscreenBtn.click();
                }
            } else if (e.key === 'Escape') {
                // Close settings panel if open
                const settingsPanel = document.querySelector('.settings-panel');
                if (settingsPanel && settingsPanel.classList.contains('visible')) {
                    settingsPanel.classList.remove('visible');
                }
            }
        });
    }
    
    static initialize(config) {
        this.initializeToggles(config);
        this.initializeDropdowns(config);
        this.initializeQualitySlider();
        this.initializeKeyboardShortcuts();
    }
}

// Export for use in main script
if (typeof module !== 'undefined' && module.exports) {
    module.exports = TemplateInitializer;
} else {
    window.TemplateInitializer = TemplateInitializer;
}

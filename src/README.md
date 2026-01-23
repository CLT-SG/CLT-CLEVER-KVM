# Source Code Structure

This directory contains the main Vue.js application source code, organized in a professional and maintainable structure.

## Directory Structure

```
src/
├── App.vue                    # Main application component
├── main.js                    # Application entry point
├── index.js                   # Main exports for the src module
├── components/                # Vue components organized by feature
│   ├── index.js              # Main components export
│   ├── server/               # Server-related components
│   │   ├── index.js          # Server components export
│   │   ├── ServerStatus.vue      # Server status display
│   │   ├── ServerConfiguration.vue # Server configuration form
│   │   ├── ServerControls.vue    # VNC server controls
│   │   ├── ConnectionOptions.vue  # Connection settings
│   │   ├── AdvancedSettings.vue   # Advanced server settings
│   │   ├── PresetSelector.vue     # Configuration presets
│   │   └── LogViewer.vue         # Server logs display
│   ├── ui/                   # Reusable UI components
│   │   ├── index.js          # UI components export
│   │   └── TabContainer.vue      # Tab navigation container
│   └── update/               # Application update components
│       ├── index.js          # Update components export
│       ├── UpdateChecker.vue     # Update status checker
│       └── UpdaterDialog.vue     # Update installation dialog
├── composables/              # Vue 3 composition functions
│   ├── index.js              # Composables export
│   └── useServer.js          # Server state and operations
└── constants/                # Application constants
    ├── index.js              # Constants export
    └── presets.js            # Server configuration presets
```

## Organization Principles

### Components
- **server/**: Components specifically related to server management and configuration
- **ui/**: Generic, reusable UI components that could be used across the application
- **update/**: Components related to application updates and version management

### Composables
- Reusable Vue 3 composition functions that encapsulate reactive state and logic
- Follow the `use*` naming convention

### Constants
- Application-wide constants, configurations, and static data

## Import Patterns

Thanks to the index.js files, you can use clean imports:

```javascript
// Instead of:
import ServerStatus from './components/server/ServerStatus.vue'
import TabContainer from './components/ui/TabContainer.vue'

// You can use:
import { ServerStatus, TabContainer } from './components'

// Or category-specific imports:
import { ServerStatus } from './components/server'
import { TabContainer } from './components/ui'
```

## Benefits of This Structure

1. **Logical Grouping**: Related components are grouped together by functionality
2. **Scalability**: Easy to add new components in appropriate categories
3. **Clean Imports**: Index files provide clean import paths
4. **Maintainability**: Clear separation of concerns
5. **Discoverability**: Easy to find components based on their purpose

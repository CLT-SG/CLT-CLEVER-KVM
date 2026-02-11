# Clever KVM Theme Documentation

This document describes the Matrix Dark Theme UI implementation for Clever KVM.

## Overview

The Matrix Dark Theme is a modern, professional dark interface inspired by The Matrix aesthetic. It features a distinctive green accent color (#00ff41) on a deep black background, creating a visually striking and easy-on-the-eyes experience.

## Theme Architecture

### CSS Custom Properties

The theme is implemented using CSS custom properties (variables) defined in `src/assets/styles/theme.css`. This allows for:
- Consistent styling across all components
- Easy theme customization
- Centralized color management

### Color Palette

| Variable | Value | Usage |
|----------|-------|-------|
| `--color-primary` | `#00ff41` | Primary accent color (Matrix green) |
| `--color-primary-dim` | `#00cc33` | Dimmed primary for hover states |
| `--color-primary-dark` | `#00aa2a` | Darker primary for active states |
| `--color-primary-glow` | `rgba(0, 255, 65, 0.3)` | Glow effect for focus states |
| `--bg-primary` | `#0a0a0a` | Main background |
| `--bg-secondary` | `#121212` | Secondary background |
| `--bg-tertiary` | `#1a1a1a` | Tertiary background (cards) |
| `--bg-card` | `#151515` | Card background |
| `--bg-hover` | `#252525` | Hover state background |
| `--text-primary` | `#e0e0e0` | Primary text color |
| `--text-secondary` | `#a0a0a0` | Secondary text color |
| `--text-muted` | `#666666` | Muted/disabled text |

### Status Colors

| Variable | Value | Usage |
|----------|-------|-------|
| `--color-success` | `#00ff41` | Success states (same as primary) |
| `--color-warning` | `#ffae00` | Warning states |
| `--color-error` | `#ff3d3d` | Error states |
| `--color-info` | `#00b4ff` | Info states |

### Spacing System

| Variable | Value | Usage |
|----------|-------|-------|
| `--spacing-xs` | `0.25rem` | Extra small spacing (4px) |
| `--spacing-sm` | `0.5rem` | Small spacing (8px) |
| `--spacing-md` | `1rem` | Medium spacing (16px) |
| `--spacing-lg` | `1.5rem` | Large spacing (24px) |
| `--spacing-xl` | `2rem` | Extra large spacing (32px) |

### Border Radius

| Variable | Value | Usage |
|----------|-------|-------|
| `--radius-sm` | `4px` | Small radius (buttons, inputs) |
| `--radius-md` | `8px` | Medium radius (cards) |
| `--radius-lg` | `12px` | Large radius (panels) |
| `--radius-xl` | `16px` | Extra large radius (dialogs) |
| `--radius-full` | `50%` | Circular elements |

### Transitions

| Variable | Value | Usage |
|----------|-------|-------|
| `--transition-fast` | `0.15s ease` | Quick interactions |
| `--transition-normal` | `0.25s ease` | Standard transitions |
| `--transition-slow` | `0.4s ease` | Slow, smooth transitions |

## Component Styling

### Interactive Status Ring

The server status indicator is a clickable ring that toggles server start/stop:

```css
.status-ring {
  width: 160px;
  height: 160px;
  border-radius: 50%;
  background: var(--bg-tertiary);
  cursor: pointer;
}

.status-ring.active::before {
  border-color: var(--color-primary);
  box-shadow: 0 0 20px var(--color-primary);
  animation: pulseRing 2s ease-in-out infinite;
}
```

**States:**
- **Stopped**: Gray border, play icon, "Click to start" hint on hover
- **Running**: Green glowing border, stop icon on hover, "Click to stop" hint
- **Loading**: Pulsing yellow border, spinner icon

### Tab Navigation

Tabs feature icons and a glowing underline indicator:

```css
.tab-button.active {
  color: var(--color-primary);
  background: var(--bg-card);
  box-shadow: 0 0 15px rgba(0, 255, 65, 0.1);
}

.tab-button.active::after {
  background: var(--color-primary);
  box-shadow: 0 0 8px var(--color-primary);
}
```

**Available Tabs:**
| Tab | Icon | Description |
|-----|------|-------------|
| Status | Circle with dot | Server status and URL |
| Configuration | Gear | Port, display, presets |
| Options | Sliders | Connection parameters |
| Logs | Document | Error and debug logs |
| Settings | Wrench | App settings & updates |

### Settings Panel

Settings are organized in sections with colored icons:

| Section | Icon Color | Purpose |
|---------|------------|---------|
| Application Updates | Purple (#8847ff) | Check for updates |
| Startup | Green (#00ff41) | Auto-start, minimized start |
| System Tray | Orange (#ffae00) | Tray icon, notifications |

### Toggle Switches

Custom toggle switches with glow effect when active:

```css
.toggle-switch input:checked + .toggle-slider {
  background-color: rgba(0, 255, 65, 0.2);
  border-color: var(--color-primary);
}

.toggle-switch input:checked + .toggle-slider:before {
  background-color: var(--color-primary);
  box-shadow: 0 0 10px var(--color-primary);
}
```

## SVG Icon System

All icons are inline SVGs using Feather/Material Design style:
- **Stroke-based**: 2px stroke width
- **Consistent sizing**: 16-24px depending on context
- **Current color**: Icons inherit text color via `currentColor`

### Icon Examples

**Play Icon (Server Start):**
```html
<svg viewBox="0 0 24 24" fill="currentColor">
  <polygon points="5 3 19 12 5 21 5 3" />
</svg>
```

**Stop Icon (Server Stop):**
```html
<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <rect x="6" y="6" width="12" height="12" rx="2" />
</svg>
```

**Gear Icon (Settings):**
```html
<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <circle cx="12" cy="12" r="3" />
  <path d="M19.4 15a1.65..." />
</svg>
```

## File Structure

```
src/
├── assets/
│   └── styles/
│       └── theme.css          # Global theme variables and base styles
├── components/
│   ├── ui/
│   │   ├── TabContainer.vue   # Tabbed navigation with icons
│   │   └── SettingsPanel.vue  # Settings with toggle switches
│   ├── server/
│   │   ├── ServerStatus.vue   # Interactive status ring
│   │   ├── ConnectionOptions.vue  # Options with SVG icons
│   │   └── ...
│   └── update/
│       ├── UpdateChecker.vue  # Update card with icons
│       └── UpdaterDialog.vue  # Modal with transitions
└── main.js                    # Theme CSS import
```

## Responsive Design

The theme includes responsive breakpoints:

| Breakpoint | Width | Changes |
|------------|-------|---------|
| Mobile | < 480px | Single column, stacked controls |
| Tablet | < 600px | Reduced padding, hidden tab labels |
| Desktop | < 768px | Flexible grids, full features |

```css
@media (max-width: 600px) {
  .status-ring {
    width: 140px;
    height: 140px;
  }
  
  .tab-label {
    display: none; /* Show icons only */
  }
}
```

## Animations

### Pulse Ring Animation
```css
@keyframes pulseRing {
  0%, 100% {
    box-shadow: 0 0 20px var(--color-primary);
  }
  50% {
    box-shadow: 0 0 35px var(--color-primary);
  }
}
```

### Loading Pulse Animation
```css
@keyframes loadingPulse {
  0%, 100% {
    border-color: var(--border-color);
  }
  50% {
    border-color: var(--color-warning);
    box-shadow: 0 0 15px rgba(255, 193, 7, 0.3);
  }
}
```

### Fade In Animation
```css
@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
```

## Customization

To customize the theme, modify the CSS variables in `theme.css`:

```css
:root {
  /* Change primary color to blue */
  --color-primary: #00b4ff;
  --color-primary-dim: #0090cc;
  --color-primary-glow: rgba(0, 180, 255, 0.3);
  
  /* Lighter background */
  --bg-primary: #1a1a1a;
  --bg-secondary: #222222;
}
```

## Browser Support

The theme uses modern CSS features supported in:
- Chrome 80+
- Firefox 75+
- Safari 13.1+
- Edge 80+

Required features:
- CSS Custom Properties
- CSS Grid
- Flexbox
- CSS Transitions
- CSS Animations

## Related Documentation

- [CHANGELOG.md](CHANGELOG.md) - Version history with UI changes
- [BUILD.md](BUILD.md) - Build instructions
- [RDENGINE_STREAMING_IMPLEMENTATION.md](RDENGINE_STREAMING_IMPLEMENTATION.md) - Streaming architecture

# Plugin System Architecture

A comprehensive, modular plugin system for audio processing UI components based on the design specifications in `design-eq.md`.

## Overview

The plugin system provides a flexible architecture for creating and managing audio processing UI components. Plugins can be used standalone or embedded in a host container.

## Architecture

### Core Components

#### 1. **Type System** (`plugin-types.ts`)

Defines interfaces and types for the entire plugin system:

- `IPlugin`: Base interface all plugins implement
- `PluginMetadata`: Plugin identification and info
- `PluginState`: Plugin state management
- `PluginConfig`: Configuration options
- `LevelMeterData`, `LUFSMeterData`: Metering data structures

#### 2. **Base Plugin** (`plugin-base.ts`)

Abstract base class providing common functionality:

- State management
- Event system (on/off/emit)
- Parameter handling
- Bypass/enable control

Plugins extend `BasePlugin` and implement:

```typescript
render(standalone: boolean): void
```

#### 3. **Reusable Components**

**LevelMeter** (`level-meter.ts`)

- Vertical level meter with peak hold
- Configurable channels, labels, dB range
- Gradient coloring (green → yellow → red)
- Used by Host and Upmixer

**PluginMenubar** (`plugin-menubar.ts`)

- Shared menubar across all plugins
- Features:
  - Name display
  - Preset dropdown (load/save)
  - Matrix routing button
  - Mute/Solo buttons
  - Bypass button
  - Custom buttons support

### Plugin Host System

#### **PluginHost** (`host.ts`)

Container for managing multiple plugins:

**Structure:**

```
┌─────────────────────────────────────────────┐
│ Menubar (Name, Presets, Matrix, M/S)       │
├─────────────────────────────────────────────┤
│ Hosting Bar: [P1] [P2] [+]                 │
├──────────────────────────────┬──────────────┤
│                              │ LUFS  M / S  │
│                              ├──────────────┤
│        Active Plugin         │ Level Meters │
│                              │              │
│                              ├──────────────┤
│                              │   Volume     │
└──────────────────────────────┴──────────────┘
```

**Features:**

- Plugin slot management (add/remove)
- Active plugin selection and display
- LUFS metering (Momentary, Short-term, Integrated)
- Multi-channel level meters
- Volume control
- Preset system integration

**Usage:**

```typescript
import { PluginHost } from './plugins';

const host = new PluginHost(container, {
  name: 'My Host',
  allowedPlugins: ['eq', 'dynamics', 'spatial'],
  maxPlugins: 10,
  showLevelMeters: true,
  showLUFS: true,
  showVolumeControl: true,
});

// Add plugins
host.addPlugin(eqPlugin);
host.addPlugin(upmixerPlugin);

// Select plugin
host.selectPlugin(eqPlugin);

// Update meters
host.updateLevelMeters({ channels: [...], peaks: [...], clipping: [...] });
host.updateLUFS({ momentary: -12.3, shortTerm: -14.5, integrated: -16.2 });
```

#### **ChannelStrip** (`host-channel-strip.ts`)

Preconfigured host with fixed plugin chain:

- EQ → Compressor → Limiter
- Fixed 3-plugin configuration
- Optimized for channel processing workflow

**Usage:**

```typescript
import { ChannelStrip } from './plugins';

const strip = new ChannelStrip(container, {
  onPluginSelect: (plugin) => console.log('Selected:', plugin),
});

const eqPlugin = strip.getEQ();
eqPlugin.setFilters([...]);
```

### Available Plugins

#### 1. **EQ Plugin** (`plugin-eq.ts`)

Visual parametric equalizer with interactive frequency response.

**Features:**

- Visual frequency response graph (20Hz - 20kHz)
- Interactive filter handles (drag to adjust)
- Filter table for precise editing
- Popup editor for detailed parameters
- Multiple filter types:
  - Peak (PK)
  - Low Shelf (LS)
  - High Shelf (HS)
  - Low Pass (LP)
  - High Pass (HP)
  - Band Pass (BP)
  - Notch (NO)
- Dynamic Y-axis scaling
- Real-time response computation via Tauri backend

**Usage:**

```typescript
import { EQPlugin } from "./plugins";

const eq = new EQPlugin();
const container = document.getElementById("eq-container")!;

// Standalone mode (with menubar)
eq.initialize(container, { standalone: true });

// Embedded mode (no menubar)
eq.initialize(container, { standalone: false });

// Set filters
eq.setFilters([
  { filter_type: "Peak", frequency: 100, q: 1.0, gain: 3.0, enabled: true },
  {
    filter_type: "Highshelf",
    frequency: 10000,
    q: 0.7,
    gain: -2.0,
    enabled: true,
  },
]);

// Get filters
const filters = eq.getFilters();

// Listen to changes
eq.on("parameterChanged", (changes) => {
  console.log("Parameters changed:", changes);
});
```

**Graph Interaction:**

- **Click & drag handles**: Adjust frequency (horizontal) and gain (vertical)
- **Q bar width**: Visual indication of filter bandwidth
- **Selected filter**: Highlighted in green with selection ring
- **Table editing**: Precise numeric input for all parameters

#### 2. **Upmixer Plugin** (`plugin-upmixer.ts`)

Stereo (2ch) to 5.0 surround upmixer with level metering.

**Layout:**

```
┌──────┬────────────────────┬────────────────────────┐
│ L  R │                    │ L  R  C  LFE  SL  SR  │
│      │   Parameters       │                        │
│ [==] │                    │ [==] [==] [==] [==]   │
│ [==] │ - Center Level     │ [==] [==] [==] [==]   │
│      │ - Surround Level   │                        │
│      │ - LFE Level        │  [M]  [M] [M]  [M]    │
│      │ - Crossfeed        │  [S]  [S] [S]  [S]    │
└──────┴────────────────────┴────────────────────────┘
```

**Features:**

- Input meters: L, R (stereo input)
- Output meters: L, R, C, LFE, SL, SR (5.1 output)
- Channel groups with mute/solo:
  - L+R (front pair)
  - C (center)
  - LFE (subwoofer)
  - SL+SR (surround pair)
- Parameters:
  - Center Level (-12 to 0 dB)
  - Surround Level (-12 to 0 dB)
  - LFE Level (-12 to 0 dB)
  - Crossfeed Amount (0-100%)

**Usage:**

```typescript
import { UpmixerPlugin } from './plugins';

const upmixer = new UpmixerPlugin();
upmixer.initialize(container, { standalone: true });

// Update meters
upmixer.updateInputMeters({ channels: [-12, -14], peaks: [-8, -10], clipping: [false, false] });
upmixer.updateOutputMeters({ channels: [-15, -16, -18, -25, -20, -20], ... });

// Set parameters
upmixer.setParameters({
  centerLevel: -3.0,
  surroundLevel: -3.0,
  lfeLevel: 0.0,
  crossfeedAmount: 0.5,
});

// Get parameters
const params = upmixer.getParameters();

// Listen to mute/solo
upmixer.on('groupMuteChange', ({ group, muted }) => {
  console.log(`${group} muted:`, muted);
});
```

**Mute/Solo Behavior:**

- **Mute**: Silences the channel group
- **Solo**: Only this group plays (others are implicitly muted)
- Solo takes precedence over mute
- Visual feedback: Active buttons highlighted in blue

## File Structure

```
src-ui-frontend/modules/plugins/
├── plugin-types.ts           # Type definitions
├── plugin-base.ts            # Base plugin class
├── level-meter.ts            # Reusable level meter component
├── plugin-menubar.ts         # Shared menubar component
├── host.ts                   # Plugin host container
├── host-channel-strip.ts     # Preconfigured host
├── plugin-eq.ts              # EQ plugin
├── plugin-upmixer.ts         # Upmixer plugin
├── index.ts                  # Public exports
└── README.md                 # This file

src-ui-frontend/styles/
└── plugins.css               # Complete styling for plugin system
```

## Styling

All plugin components are styled in `styles/plugins.css` with:

- Dark theme optimized (respects CSS variables)
- Consistent spacing and typography
- Responsive design
- Hover/active states
- Smooth transitions

Import in your main CSS or component:

```css
@import "./plugins.css";
```

## Future Extensions

### Planned Plugins

1. **Compressor Plugin**
   - Threshold, ratio, attack, release
   - Gain reduction meter
   - Knee control

2. **Limiter Plugin**
   - Ceiling, release
   - True peak detection
   - Lookahead

3. **Spectrum Analyzer Plugin**
   - Wrap existing `spectrum-analyzer.ts`
   - FFT-based visualization
   - Configurable resolution

### Planned Features

1. **Plugin Factory**
   - Dynamic plugin instantiation from type string
   - Plugin registration system

2. **Preset Management**
   - Save/load presets
   - Preset browser
   - Factory presets

3. **Routing Matrix**
   - Visual routing editor
   - Multi-bus support
   - Send/return configuration

## Development Guidelines

### Creating a New Plugin

1. **Define Plugin Class**

```typescript
import { BasePlugin } from "./plugin-base";
import type { PluginMetadata } from "./plugin-types";

export class MyPlugin extends BasePlugin {
  public readonly metadata: PluginMetadata = {
    id: "my-plugin",
    name: "My Plugin",
    category: "utility",
    version: "1.0.0",
  };

  render(standalone: boolean): void {
    // Render UI
  }
}
```

2. **Implement Render Method**

```typescript
render(standalone: boolean): void {
  if (!this.container) return;

  this.container.innerHTML = `
    <div class="my-plugin ${standalone ? 'standalone' : 'embedded'}">
      ${standalone ? '<div class="menubar-container"></div>' : ''}
      <div class="plugin-content">
        <!-- Plugin UI -->
      </div>
    </div>
  `;

  // Initialize components
  if (standalone) {
    const menubar = new PluginMenubar(...);
  }

  this.attachEventListeners();
}
```

3. **Handle Parameters**

```typescript
// Update parameter
this.updateParameter("paramName", value);

// Listen to changes
this.on("parameterChanged", (changes) => {
  // React to changes
});
```

4. **Export in index.ts**

```typescript
export { MyPlugin } from "./plugin-my";
```

5. **Add Styling in plugins.css**

```css
.my-plugin {
  /* Plugin styles */
}
```

## Integration Examples

### Standalone Plugin

```typescript
import { EQPlugin } from './plugins';

const eq = new EQPlugin();
const container = document.getElementById('standalone-eq')!;

eq.initialize(container, {
  standalone: true,
  initialState: {
    enabled: true,
    bypassed: false,
    parameters: { filters: [...] },
  },
  onStateChange: (state) => console.log('State:', state),
});
```

### Plugin in Host

```typescript
import { PluginHost, EQPlugin, UpmixerPlugin } from "./plugins";

const host = new PluginHost(container, {
  name: "Master Bus",
  showLevelMeters: true,
  showLUFS: true,
});

const eq = new EQPlugin();
const upmixer = new UpmixerPlugin();

host.addPlugin(eq);
host.addPlugin(upmixer);
host.selectPlugin(eq);
```

### Channel Strip (Preconfigured)

```typescript
import { ChannelStrip } from './plugins';

const strip = new ChannelStrip(container, {
  onPluginSelect: (plugin) => {
    console.log('Active plugin:', plugin?.metadata.name);
  },
  onVolumeChange: (volume) => {
    console.log('Volume:', volume);
  },
});

// Access specific plugins
const eq = strip.getEQ();
eq.setFilters([...]);
```

## Testing

See the test plan in the main TODO list for integration testing procedures.

## License

Part of the AutoEQ project.

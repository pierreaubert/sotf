# Audio Capture Module

This module provides a complete audio capture interface for measuring frequency and phase response using microphones and test signals.

## Architecture

### Web Component

The capture interface is now implemented as a **Web Component** (`capture-panel`) for better modularity and reusability.

**File:** `capture-panel.ts`

```typescript
import "@audio-capture/capture-panel";

// Then use in HTML:
<capture-panel></capture-panel>
```

### Key Features

- **Self-contained**: All HTML template is encapsulated in the web component
- **Auto-registration**: Component registers itself with `customElements.define()`
- **Event-driven**: Dispatches `capturePanelRendered` event when ready
- **No dependencies on templates.ts**: Template is managed within the component

### Usage

#### 1. Inline Usage (Data Acquisition Step)

The capture panel is embedded directly in the Data Acquisition step:

```typescript
// data-acquisition-step.ts
import "@audio-capture/capture-panel";

// Render the component
const capturePanel = document.createElement("capture-panel");
container.appendChild(capturePanel);
```

#### 2. Modal Usage (Legacy)

The capture modal still uses the web component internally:

```typescript
// templates.ts
export function generateCaptureModal(): string {
  return `<div id="capture_modal" class="modal">
    <div class="modal-content">
      <capture-panel></capture-panel>
    </div>
  </div>`;
}
```

## Components

### capture-panel.ts

Custom element that renders the complete capture interface including:

- Input/output device selectors
- Volume controls
- Calibration file loader
- Signal type and duration selectors
- Frequency/phase response graph
- Saved records sidebar
- Control buttons (Start, Stop, Export)

### capture-modal-manager.ts

Manages the capture panel functionality:

- Device enumeration
- Audio recording
- Signal playback
- Graph rendering
- Record management

**Note:** Despite the name "modal-manager", it now works for both modal and inline panel usage.

## Migration Notes

### Before (String Templates)

```typescript
// templates.ts
export function generateCapturePanel(): string {
  return `<div>...200+ lines of HTML...</div>`;
}

// Usage
container.innerHTML = generateCapturePanel();
```

### After (Web Component)

```typescript
// capture-panel.ts
export class CapturePanel extends HTMLElement {
  render() {
    /* HTML template */
  }
}

// Usage
import "@audio-capture/capture-panel";
const panel = document.createElement("capture-panel");
container.appendChild(panel);
```

## Benefits

1. **Separation of Concerns**: Template is in the component, not in a utility file
2. **Reusability**: Can be used anywhere (inline or in modal)
3. **Maintainability**: All capture UI code is in one module
4. **Type Safety**: Component is a TypeScript class with proper types
5. **Standards-Based**: Uses Web Components standard
6. **No Build Step**: Native browser feature

## Event Flow

1. Component is created: `document.createElement('capture-panel')`
2. Component is added to DOM: `container.appendChild(capturePanel)`
3. `connectedCallback()` fires
4. Component dispatches `capturePanelRendered` event
5. `CaptureModalManager.initialize()` is called
6. UI is ready for user interaction

## Styling

All styles are defined in `styles.css` and `audio-capture/styles.css`:

- `.capture-panel` - Main container
- `.capture-panel-body` - Content area
- `.capture-controls-block` - Control groups
- `.capture-main-area` - Graph and sidebar
- `.capture-bottom-controls` - Action buttons

## Future Improvements

- [ ] Use Shadow DOM for style encapsulation
- [ ] Add TypeScript decorators for property binding
- [ ] Extract sub-components (graph, controls, records)
- [ ] Add unit tests for the web component
- [ ] Support multiple instances on the same page

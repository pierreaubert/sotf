# Guide: Adding data-testid Attributes for E2E Tests

This guide shows where and how to add `data-testid` attributes to make E2E tests more reliable and maintainable.

## Why data-testid?

- **Stable selectors:** Don't break when CSS classes change
- **Clear intent:** Shows which elements are used in tests
- **Better performance:** Faster element lookup than complex CSS selectors

## Naming Convention

```
data-testid="feature-element-type"
```

Examples:
- `data-testid="speaker-select"`
- `data-testid="optimize-button"`
- `data-testid="step3-results"`
- `data-testid="eq-toggle"`

## Where to Add

### 1. Use Case Cards (src-ui-frontend/modules/use-case-selector.ts)

```typescript
<div class="use-case-card ${selected}"
     data-use-case="${useCase}"
     data-testid="use-case-${useCase}">  // ADD THIS
```

### 2. Step Navigation (src-ui-frontend/modules/step-navigator.ts)

```typescript
<div class="step-nav-item ${activeClass} ${disabledClass}"
     data-step-id="${step.id}"
     data-testid="step-nav-${step.id}">  // ADD THIS
```

### 3. Form Inputs (src-ui-frontend/modules/templates.ts)

Already have IDs, but add data-testid for consistency:

```typescript
<input
  type="number"
  id="num_filters"
  data-testid="num-filters-input"  // ADD THIS
  name="num_filters"
  value="5">

<select
  id="algo"
  data-testid="algorithm-select"  // ADD THIS
  name="algo">
```

### 4. Buttons (src-ui-frontend/main.ts)

```typescript
<button
  type="submit"
  id="optimize_btn"
  data-testid="optimize-button"  // ADD THIS
  class="btn btn-primary btn-large">

<button
  type="button"
  id="step3_continue_btn"
  data-testid="step3-continue-button"  // ADD THIS
  class="btn btn-primary btn-large">
```

### 5. Results Sections (src-ui-frontend/main.ts)

```typescript
<div
  id="step3-results"
  data-testid="optimization-results"  // ADD THIS
  class="step3-results">

<span
  id="step3_score_before"
  data-testid="score-before">  // ADD THIS
  -
</span>
```

### 6. Audio Player (src-ui-frontend/modules/audio-player/audio-player.ts)

```typescript
<button
  class="player-button"
  data-testid="play-button">  // ADD THIS
  ▶ Play
</button>

<button
  class="player-button"
  data-testid="eq-toggle-button">  // ADD THIS
  EQ: ON
</button>

<canvas
  class="spectrum-canvas"
  data-testid="spectrum-canvas">  // ADD THIS
</canvas>
```

### 7. Modals (src-ui-frontend/modules/templates.ts)

```typescript
<div
  id="optimization_modal"
  data-testid="optimization-modal"  // ADD THIS
  class="modal">

<button
  id="cancel_optimization_btn"
  data-testid="cancel-button"  // ADD THIS
  class="btn btn-secondary">
```

## Priority List

### High Priority (Core Workflows)

1. ✅ Use case cards: `data-testid="use-case-speaker"`, etc.
2. ✅ Step navigation: `data-testid="step-nav-1"`, etc.
3. ✅ Main action buttons: `optimize-button`, `continue-button`
4. ✅ Form inputs: `num-filters-input`, `algorithm-select`
5. ✅ Results displays: `optimization-results`, `score-before`

### Medium Priority (Audio Features)

6. Audio player controls: `play-button`, `stop-button`, `eq-toggle`
7. Volume/progress controls: `volume-slider`, `progress-bar`
8. Spectrum canvas: `spectrum-canvas`

### Low Priority (Nice to Have)

9. Individual filter controls
10. Advanced settings
11. Status indicators

## Example: Update Use Case Selector

**File:** `src-ui-frontend/modules/use-case-selector.ts`

**Before:**
```typescript
<div class="use-case-card" data-use-case="speaker">
  <h3>🔊 Speaker Optimization</h3>
</div>
```

**After:**
```typescript
<div
  class="use-case-card"
  data-use-case="speaker"
  data-testid="use-case-speaker">
  <h3>🔊 Speaker Optimization</h3>
</div>
```

## Example: Update Main Buttons

**File:** `src-ui-frontend/main.ts`

**Before:**
```typescript
<button type="submit" id="optimize_btn" class="btn btn-primary btn-large">
  Run Optimization
</button>
```

**After:**
```typescript
<button
  type="submit"
  id="optimize_btn"
  data-testid="optimize-button"
  class="btn btn-primary btn-large">
  Run Optimization
</button>
```

## Using in Tests

### Before (fragile):
```typescript
const button = await $(".btn.btn-primary.btn-large");
```

### After (stable):
```typescript
const button = await $('[data-testid="optimize-button"]');
```

## Verification Checklist

After adding data-testid attributes, verify they work:

1. **Run tests:**
   ```bash
   npm run test:e2e-ui
   ```

2. **Check in DevTools:**
   - Open app in dev mode: `npm run tauri dev`
   - Inspect elements in browser DevTools
   - Verify data-testid attributes are present

3. **Update test selectors:**
   - Replace CSS selectors with `data-testid` where applicable
   - Re-run tests to confirm they pass

## Migration Strategy

### Phase 1: Core Elements (Week 1)
- Use case cards
- Step navigation
- Main buttons (optimize, continue, reset)
- Form inputs (num_filters, algo, etc.)

### Phase 2: Results & Modals (Week 2)
- Optimization modal
- Results displays
- Score indicators
- Export controls

### Phase 3: Audio Player (Week 3)
- Player buttons
- EQ controls
- Volume/progress
- Spectrum canvas

### Phase 4: Advanced Features (Week 4)
- Filter controls
- Settings panels
- Status indicators
- Error displays

## Automated Script (Future)

Consider creating a script to add data-testid automatically:

```bash
# scripts/add-testids.sh
# Adds data-testid based on element IDs
```

This would parse HTML/TypeScript files and add `data-testid="${id}"` where `id="${id}"` exists.

## Resources

- [Testing Library Best Practices](https://testing-library.com/docs/queries/bytestid/)
- [WebDriver Element Selectors](https://webdriver.io/docs/selectors/)

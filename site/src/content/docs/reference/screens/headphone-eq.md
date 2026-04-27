---
title: Headphone EQ Screen
description: Optimize headphone frequency response with scientifically-derived target curves.
---

The Headphone EQ screen guides you through the process of generating a parametric EQ
correction for your headphones. It uses the same optimization engine as the `autoeq` CLI.

## Workflow

### 1. Select Your Headphone

Choose your headphone model from the built-in database (sourced from community
measurement repositories) or import a custom frequency response CSV file.

The CSV format is: `frequency,spl` (one row per measurement point).

### 2. Choose a Target Curve

| Target | Description |
|--------|-------------|
| **Harman Over-Ear 2018** | Widely accepted research-based target for over-ear headphones |
| **Harman In-Ear 2019** | Research-based target for in-ear monitors |
| **Diffuse Field** | Flat perceived response in a diffuse sound field |
| **Free Field** | Flat perceived response from a frontal sound source |
| **Custom** | Import your own target curve CSV |

The Harman targets are a good starting point for most listeners.

### 3. Configure the Optimizer

| Setting | Description |
|---------|-------------|
| **Filters** | Number of parametric EQ bands (5–10 recommended) |
| **Max gain** | Maximum boost per band (keep ≤ 6 dB to avoid driver stress) |
| **Frequency range** | Limit optimization to a specific range (e.g., 20–10000 Hz) |
| **Algorithm** | COBYLA (fast) or Differential Evolution (more thorough) |

### 4. Optimize

Press **Optimize** (or `Enter`). The optimizer runs and displays the result curve
overlaid on your headphone's measured response.

### 5. Review and Apply

The result screen shows:
- Before/after frequency response overlay
- Generated PEQ filter list
- Predicted error (deviation from target)

If the result looks good, press **Apply** to add the filters as an EQ plugin in your
chain. If not, adjust settings and run the optimizer again.

### 6. A/B Compare

Use the A/B Compare plugin (or toggle the EQ on/off with `e` in the Plugins screen)
to switch between corrected and uncorrected sound in real time.

## Tips

- **Bass boost preference** — The Harman target includes a bass shelf. If you find it
  too bass-heavy, use the `headphone-flat` loss instead of `headphone-score`.
- **Measurement quality** — Community measurements vary; try multiple measurement
  sources for your model if available.
- **Preamp** — Boost EQ bands can increase peak level above 0 dBFS. Add a Gain plugin
  before the EQ with a negative value equal to the largest boost in your filter set.

## See Also

- [Better Headphones Quick Start](/quick-start/headphone-quick/)
- [Headphone EQ Guide](/guides/headphone-eq/)

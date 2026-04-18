# 0.6.3

## Features

- Stroke dash array support (`.dash_array(StrokeDashArray::Dashed)`) for line charts
- Migrated showcase to design system / builder pattern
- Re-exported `StrokeDashArray` from `gpui_px`

## Fixes

- Clippy and metadata cleanup
- Showcase dash pattern demo (Solid, Dashed, Dotted, Dash-Dot, Custom)

# 0.6.2

## Features

- Interactive chart pan/zoom (`InteractiveChart` with drag and scroll)
- Heatmap rendering improvements and log scale support
- Bar chart negative value support

## Fixes

- Treemap and boxplot rendering fixes
- Animation crash fix

# 0.6.0

- Initial release after crate reorganization (renamed to `gpui-px`)
- Plotly Express-style API: `scatter()`, `line()`, `bar()`, `heatmap()`,
  `contour()`, `isoline()`, `treemap()`, `boxplot()`, `pie()`, `area()`
- Multi-series line charts with legend and secondary Y-axis
- Logarithmic scale support for all chart types
- Color scales (Viridis, Plasma, Inferno, Magma, Heat, Coolwarm, Greys)
- Showcase binary with interactive examples

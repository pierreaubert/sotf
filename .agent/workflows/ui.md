---
description: UI features
---

UI features will use
- gpui-ui-kit
- gpui-px for simple plots
UI features will not use
- gpui-d3rs
- complicated logic that should be implemented in a component in ui-kit

All UI components must:
- support theming
- support i18n
- support mouse events
- support keyboard events
- be tested with at least one builder test and one integration test.

All changes must check, test, and build.
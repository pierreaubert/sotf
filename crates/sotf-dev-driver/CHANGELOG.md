# [0.8.4] - 2026-07-08

- QA review: documented dev-driver workflows (scenario/suite DSL, GPUI/TUI/CLI
  targets, query paths, tracked elements) in `README.md`.
- Clarified that `sotf-dev-driver` is a development / CI-only tool; it is not
  built by `just dist` and is intended for use against debug builds with the
  `dev-api` feature.

# 0.6.2

- Added support for testing rack and plugins

# 0.6.1

## New

- Added some basic scenario to test the app-gpui
- Added a dev-api that allows to control the GPUI app from the outside

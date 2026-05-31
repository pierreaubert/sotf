# sotf-docs-gen

Generate documentation markdown from plugin `ParamSpec` definitions.

## Overview

Reads the static `ParamSpec` arrays compiled into every SOTF plugin and generates markdown reference pages for the documentation site. Each plugin gets its own page with a full parameter table (name, type, range, default, unit, description), and an index page links them all together.

## Features

- Generates per-plugin reference pages from compiled `ParamSpec` data
- Renders parameter tables grouped by parameter group
- Handles flat params, global + per-band params (EQ, multiband compressor/expander)
- Marks structural parameters that require plugin rebuild
- Idempotent output (only writes files that changed)
- Outputs Astro/Starlight-compatible markdown with YAML frontmatter

## Usage

```bash
# From the project root (directory with Cargo.toml + site/)
cargo run -p sotf-docs-gen

# CI/idempotence check: fail if generated files would change.
cargo run -p sotf-docs-gen -- --check
```

Output goes to `site/src/content/docs/reference/plugins/`:
- `index.md` — Plugin index with descriptions
- `{slug}.md` — Individual plugin reference (e.g., `eq.md`, `compressor.md`)

## Architecture

Single-file binary crate (`src/main.rs`). No library component.

1. `plugin_registry()` defines all plugins with their `ParamSpec` references
2. `generate_plugin_page()` renders one plugin to markdown
3. `generate_plugin_index()` renders the index page
4. `write_if_changed()` writes only modified files

## Testing

```bash
cargo check -p sotf-docs-gen
cargo test -p sotf-docs-gen
```

## License

See the root workspace `LICENSE` file.

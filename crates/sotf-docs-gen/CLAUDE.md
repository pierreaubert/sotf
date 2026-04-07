# sotf-docs-gen

Generate documentation markdown from plugin ParamSpec definitions, keybindings, and screen guides.

## Architecture

Binary-only crate (no lib.rs). Single file: `src/main.rs`.

```
main.rs
  PluginEntry        -- Registry entry: slug, name, description, params, global_params, band_template
  plugin_registry()  -- Returns Vec<PluginEntry> with all 36 plugins and their ParamSpec arrays
  generate_plugin_page()   -- Renders one plugin's markdown page with frontmatter + parameter tables
  generate_plugin_index()  -- Renders the plugin index page linking to all individual pages
  generate_params_table()  -- Formats ParamSpec array as markdown table (grouped by group name)
  write_if_changed()       -- Only writes files that actually changed (idempotent)
  find_project_root()      -- Walks up to find directory with both Cargo.toml and site/
```

Output goes to `site/src/content/docs/reference/plugins/` (Astro/Starlight docs site).

## Key Public API

Binary only -- no library API. Run with:

```bash
cargo run -p sotf-docs-gen
```

## Testing

```bash
cargo check -p sotf-docs-gen
```

No tests (binary-only crate). Verify by running the binary and checking generated markdown.

## Important Notes

- Reads `ParamSpec` arrays directly from `sotf_plugins::param_specs::*` modules at compile time
- Supports three parameter layouts: flat params, global_params + band_template (EQ, multiband), or both
- Handles `ParamType` variants: Float, Int, Bool, Choice, FilePath
- Marks `UpdateMode::Structural` parameters with a special info box (require plugin rebuild)
- Output uses Astro/Starlight frontmatter format (`---` YAML blocks with title/description)
- `write_if_changed()` is idempotent -- safe to run in CI without creating spurious diffs
- Depends on `sotf-host` (for ParamSpec types) and `sotf-plugins` (for actual param definitions)

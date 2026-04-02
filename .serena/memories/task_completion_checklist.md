# Task Completion Checklist

After completing any code change, run these steps in order:

1. **Format**: `cargo fmt --all`
2. **Check**: `cargo check` on affected crate(s)
   - For plugins/engine crates: use `--no-default-features` (hdf5-metno-sys build issue)
3. **Lint**: `cargo clippy` on affected crate(s)
4. **Test**: `cargo test -p <crate>` for each modified crate
5. **Verify**: Re-read modified files to confirm edits applied correctly

## Notes
- `RUST_MIN_STACK=16777216` is needed for full workspace builds (deeply nested GPUI macros)
- Pre-existing clippy warnings exist in some crates — focus on warnings from your changes
- Pre-existing compilation errors exist in `plugins/plugin_expander.rs`

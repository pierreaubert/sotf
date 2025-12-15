//! Legacy file for result graphs
//!
//! Content has been moved to:
//! - `headphone_graphs.rs`: Headphone optimization results
//! - `speaker_graphs.rs`: Speaker optimization results
//! - `common.rs`: Shared utilities

// This file is kept to avoid breaking existing imports of `result_graphs`,
// but the functionality is now provided by the new modules.
// Since the functionality was implemented as methods on `PlayerView`,
// importing the new modules in `mod.rs` is sufficient to make them available.
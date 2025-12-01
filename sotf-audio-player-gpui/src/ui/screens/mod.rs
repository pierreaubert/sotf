// Screen rendering modules
//
// Note: plugins and spectrum screens have been moved to ui/components:
// - plugins screen: ui/components/host/rack.rs
// - spectrum screen: ui/components/plugins/spectrum.rs
pub mod library;
pub mod queue;
pub mod settings;

// Re-export directory from settings module for backward compatibility
pub use settings::directory;

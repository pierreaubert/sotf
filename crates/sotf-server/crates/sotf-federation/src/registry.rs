// ============================================================================
// Source Registry
// ============================================================================
//
// Manages configured library providers, their priorities, and availability.

use crate::provider::*;
use std::collections::HashMap;

/// Stored source configuration (mirrors the `library_sources` database table).
#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub source_id: SourceId,
    pub source_type: SourceType,
    pub display_name: String,
    pub priority: i32,
    pub is_enabled: bool,
    pub config_json: Option<String>,
    pub last_sync_at: Option<u64>,
}

/// Manages registered library providers and their configuration.
pub struct SourceRegistry {
    /// Registered providers, keyed by source_id.
    providers: HashMap<String, Box<dyn LibraryProvider>>,
    /// Configuration for each source (persisted in database).
    configs: HashMap<String, SourceConfig>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            configs: HashMap::new(),
        }
    }

    /// Register a provider with its configuration.
    pub fn register(&mut self, provider: Box<dyn LibraryProvider>, config: SourceConfig) {
        let key = provider.source_id().0.clone();
        self.configs.insert(key.clone(), config);
        self.providers.insert(key, provider);
    }

    /// Unregister a provider by source_id.
    pub fn unregister(&mut self, source_id: &str) -> Option<Box<dyn LibraryProvider>> {
        self.configs.remove(source_id);
        self.providers.remove(source_id)
    }

    /// Get a provider by source_id.
    pub fn get(&self, source_id: &str) -> Option<&dyn LibraryProvider> {
        self.providers.get(source_id).map(|p| p.as_ref())
    }

    /// Get configuration for a source.
    pub fn get_config(&self, source_id: &str) -> Option<&SourceConfig> {
        self.configs.get(source_id)
    }

    /// Update the last_sync_at timestamp for a source.
    pub fn update_last_sync(&mut self, source_id: &str, timestamp: u64) {
        if let Some(config) = self.configs.get_mut(source_id) {
            config.last_sync_at = Some(timestamp);
        }
    }

    /// Get all enabled providers, sorted by priority (highest first).
    pub fn enabled_providers(&self) -> Vec<(&str, &dyn LibraryProvider, i32)> {
        let mut result: Vec<_> = self
            .providers
            .iter()
            .filter_map(|(key, provider)| {
                self.configs.get(key).and_then(|config| {
                    if config.is_enabled {
                        Some((key.as_str(), provider.as_ref(), config.priority))
                    } else {
                        None
                    }
                })
            })
            .collect();
        result.sort_by_key(|b| std::cmp::Reverse(b.2)); // highest priority first
        result
    }

    /// Get all registered source IDs.
    pub fn source_ids(&self) -> Vec<&str> {
        self.providers.keys().map(|k| k.as_str()).collect()
    }

    /// Check if a source is registered.
    pub fn contains(&self, source_id: &str) -> bool {
        self.providers.contains_key(source_id)
    }

    /// Get the priority for a source (higher = preferred for metadata conflicts).
    pub fn priority(&self, source_id: &str) -> Option<i32> {
        self.configs.get(source_id).map(|c| c.priority)
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_provider::{LocalFilesProvider, LocalProviderConfig};
    use std::path::PathBuf;

    #[test]
    fn test_register_and_get() {
        let mut registry = SourceRegistry::new();

        let provider = LocalFilesProvider::new(LocalProviderConfig {
            directories: vec![PathBuf::from("/music")],
        });

        registry.register(
            Box::new(provider),
            SourceConfig {
                source_id: SourceId("local".to_string()),
                source_type: SourceType::Local,
                display_name: "Local Files".to_string(),
                priority: 100,
                is_enabled: true,
                config_json: None,
                last_sync_at: None,
            },
        );

        assert_eq!(registry.len(), 1);
        assert!(registry.contains("local"));
        assert!(registry.get("local").is_some());
        assert_eq!(registry.priority("local"), Some(100));
    }

    #[test]
    fn test_enabled_providers_sorted_by_priority() {
        let mut registry = SourceRegistry::new();

        // Register two providers with different priorities
        let local = LocalFilesProvider::new(LocalProviderConfig {
            directories: vec![],
        });
        registry.register(
            Box::new(local),
            SourceConfig {
                source_id: SourceId("local".to_string()),
                source_type: SourceType::Local,
                display_name: "Local".to_string(),
                priority: 100,
                is_enabled: true,
                config_json: None,
                last_sync_at: None,
            },
        );

        let remote = LocalFilesProvider::new(LocalProviderConfig {
            directories: vec![],
        });
        // Using LocalFilesProvider as a stand-in; in practice this would be SubsonicProvider etc.
        // We override the source_id via the config key.
        registry
            .providers
            .insert("subsonic:test".to_string(), Box::new(remote));
        registry.configs.insert(
            "subsonic:test".to_string(),
            SourceConfig {
                source_id: SourceId("subsonic:test".to_string()),
                source_type: SourceType::Subsonic,
                display_name: "Subsonic".to_string(),
                priority: 50,
                is_enabled: true,
                config_json: None,
                last_sync_at: None,
            },
        );

        let enabled = registry.enabled_providers();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0].2, 100); // local first (higher priority)
        assert_eq!(enabled[1].2, 50); // subsonic second
    }

    #[test]
    fn test_disabled_providers_excluded() {
        let mut registry = SourceRegistry::new();

        let provider = LocalFilesProvider::new(LocalProviderConfig {
            directories: vec![],
        });
        registry.register(
            Box::new(provider),
            SourceConfig {
                source_id: SourceId("local".to_string()),
                source_type: SourceType::Local,
                display_name: "Local".to_string(),
                priority: 100,
                is_enabled: false,
                config_json: None,
                last_sync_at: None,
            },
        );

        assert_eq!(registry.enabled_providers().len(), 0);
        assert_eq!(registry.len(), 1); // still registered, just disabled
    }

    #[test]
    fn test_unregister() {
        let mut registry = SourceRegistry::new();

        let provider = LocalFilesProvider::new(LocalProviderConfig {
            directories: vec![],
        });
        registry.register(
            Box::new(provider),
            SourceConfig {
                source_id: SourceId("local".to_string()),
                source_type: SourceType::Local,
                display_name: "Local".to_string(),
                priority: 100,
                is_enabled: true,
                config_json: None,
                last_sync_at: None,
            },
        );

        assert!(registry.unregister("local").is_some());
        assert_eq!(registry.len(), 0);
        assert!(!registry.contains("local"));
    }

    #[test]
    fn test_update_last_sync() {
        let mut registry = SourceRegistry::new();

        let provider = LocalFilesProvider::new(LocalProviderConfig {
            directories: vec![],
        });
        registry.register(
            Box::new(provider),
            SourceConfig {
                source_id: SourceId("local".to_string()),
                source_type: SourceType::Local,
                display_name: "Local".to_string(),
                priority: 100,
                is_enabled: true,
                config_json: None,
                last_sync_at: None,
            },
        );

        assert_eq!(registry.get_config("local").unwrap().last_sync_at, None);
        registry.update_last_sync("local", 1234567890);
        assert_eq!(
            registry.get_config("local").unwrap().last_sync_at,
            Some(1234567890)
        );
    }
}

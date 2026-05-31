//! Curated MIDI mapping templates for common controller + plugin combinations
//!
//! Templates provide hand-tuned mappings that are better than auto-map defaults
//! for specific controller/plugin pairs.

use crate::mapping::{ControlBinding, MidiMapping, ValueScaling};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A curated mapping template for a specific controller + plugin combination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingTemplate {
    /// Controller layout name
    pub controller_name: String,
    /// Plugin type (e.g., "Compressor", "EQ")
    pub plugin_type: String,
    /// Pre-configured bindings (plugin_index will be filled in at runtime)
    pub bindings: Vec<TemplateBinding>,
}

/// A binding within a template (plugin_index is determined at runtime)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateBinding {
    pub control_id: String,
    pub param_index: usize,
    pub page: usize,
    pub scaling: ValueScaling,
}

impl MappingTemplate {
    /// Convert this template into a concrete MidiMapping for a specific plugin instance
    pub fn to_mapping(&self, plugin_index: usize) -> MidiMapping {
        let bindings = self
            .bindings
            .iter()
            .map(|tb| ControlBinding {
                control_id: tb.control_id.clone(),
                plugin_index,
                param_index: tb.param_index,
                page: tb.page,
                scaling: tb.scaling,
            })
            .collect::<Vec<_>>();

        let total_pages = bindings.iter().map(|b| b.page).max().map_or(1, |p| p + 1);

        MidiMapping {
            controller_name: self.controller_name.clone(),
            plugin_type: self.plugin_type.clone(),
            bindings,
            current_page: 0,
            total_pages,
            manual_overrides: HashMap::new(),
        }
    }

    /// Validate this template against the currently focused plugin's parameter count.
    pub fn validate_for_param_count(&self, param_count: usize) -> Result<(), String> {
        for binding in &self.bindings {
            if binding.param_index >= param_count {
                return Err(format!(
                    "template {} / {} binding for control '{}' references param_index {}, but plugin exposes {} params",
                    self.controller_name,
                    self.plugin_type,
                    binding.control_id,
                    binding.param_index,
                    param_count
                ));
            }
        }
        Ok(())
    }

    /// Convert this template after checking all parameter indices are in bounds.
    pub fn to_mapping_checked(
        &self,
        plugin_index: usize,
        param_count: usize,
    ) -> Result<MidiMapping, String> {
        self.validate_for_param_count(param_count)?;
        Ok(self.to_mapping(plugin_index))
    }
}

/// Registry of mapping templates, loaded from disk or built-in defaults
#[derive(Debug, Clone)]
pub struct TemplateRegistry {
    templates: Vec<MappingTemplate>,
}

impl TemplateRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
        }
    }

    /// Load templates from a directory (JSON files)
    pub fn load_from_dir<P: AsRef<Path>>(&mut self, dir: P) -> Result<usize, std::io::Error> {
        let dir = dir.as_ref();
        if !dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                match fs::read_to_string(&path) {
                    Ok(contents) => match serde_json::from_str::<MappingTemplate>(&contents) {
                        Ok(template) => {
                            self.templates.push(template);
                            count += 1;
                        }
                        Err(e) => {
                            log::warn!("Failed to parse template {:?}: {}", path, e);
                        }
                    },
                    Err(e) => {
                        log::warn!("Failed to read template {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(count)
    }

    /// Add a template programmatically
    pub fn add(&mut self, template: MappingTemplate) {
        self.templates.push(template);
    }

    /// Find a template for a specific controller + plugin combination
    pub fn find(&self, controller_name: &str, plugin_type: &str) -> Option<&MappingTemplate> {
        self.templates
            .iter()
            .find(|t| t.controller_name == controller_name && t.plugin_type == plugin_type)
    }

    /// Get the default templates directory path
    pub fn default_dir() -> Option<PathBuf> {
        directories::BaseDirs::new()
            .map(|b| b.config_dir().join("sotf").join("midi").join("templates"))
    }

    /// Save a template to the templates directory
    pub fn save_template(&self, template: &MappingTemplate) -> Result<(), std::io::Error> {
        if let Some(dir) = Self::default_dir() {
            fs::create_dir_all(&dir)?;
            let filename = format!(
                "{}_{}.json",
                template.controller_name.replace(' ', "_").to_lowercase(),
                template.plugin_type.replace(' ', "_").to_lowercase()
            );
            let path = dir.join(filename);
            let json = serde_json::to_string_pretty(template).map_err(std::io::Error::other)?;
            fs::write(path, json)?;
        }
        Ok(())
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_to_mapping() {
        let template = MappingTemplate {
            controller_name: "Test Controller".to_string(),
            plugin_type: "Compressor".to_string(),
            bindings: vec![
                TemplateBinding {
                    control_id: "pot_1".to_string(),
                    param_index: 0,
                    page: 0,
                    scaling: ValueScaling::Linear,
                },
                TemplateBinding {
                    control_id: "fader_1".to_string(),
                    param_index: 1,
                    page: 0,
                    scaling: ValueScaling::Linear,
                },
                TemplateBinding {
                    control_id: "pot_1".to_string(),
                    param_index: 5,
                    page: 1,
                    scaling: ValueScaling::Logarithmic,
                },
            ],
        };

        let mapping = template.to_mapping(2);
        assert_eq!(mapping.plugin_type, "Compressor");
        assert_eq!(mapping.total_pages, 2);
        assert_eq!(mapping.bindings.len(), 3);
        assert!(mapping.bindings.iter().all(|b| b.plugin_index == 2));
    }

    #[test]
    fn test_template_rejects_stale_param_indices() {
        let template = MappingTemplate {
            controller_name: "Test Controller".to_string(),
            plugin_type: "Compressor".to_string(),
            bindings: vec![TemplateBinding {
                control_id: "pot_1".to_string(),
                param_index: 99,
                page: 0,
                scaling: ValueScaling::Linear,
            }],
        };

        let err = template.to_mapping_checked(0, 4).unwrap_err();
        assert!(err.contains("param_index 99"), "{err}");
    }

    #[test]
    fn test_registry_find() {
        let mut registry = TemplateRegistry::new();
        registry.add(MappingTemplate {
            controller_name: "Xone K2".to_string(),
            plugin_type: "Compressor".to_string(),
            bindings: vec![],
        });
        registry.add(MappingTemplate {
            controller_name: "LCXL".to_string(),
            plugin_type: "EQ".to_string(),
            bindings: vec![],
        });

        assert!(registry.find("Xone K2", "Compressor").is_some());
        assert!(registry.find("LCXL", "EQ").is_some());
        assert!(registry.find("Xone K2", "EQ").is_none());
    }
}

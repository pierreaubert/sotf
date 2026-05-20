// ============================================================================
// Plugin Serialization
// ============================================================================

use crate::error::PluginError;
use crate::parameters::ParameterValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trait for plugins that support preset serialization
///
/// This trait enables plugins to save and load their state as presets.
/// Plugins implement this trait to support:
/// - Preset file save/load
/// - Parameter automation
/// - Plugin state snapshots
///
/// # Example
/// ```rust,ignore
/// use sotf_plugins::{SerializablePlugin, PluginPreset};
///
/// impl SerializablePlugin for EqPlugin {
///     fn serialize(&self) -> Result<PluginPreset, PluginError> {
///         Ok(PluginPreset {
///             name: "My EQ".to_string(),
///             plugin_id: "sotf-eq".to_string(),
///             version: env!("CARGO_PKG_VERSION").to_string(),
///             parameters: self.parameters_to_map(),
///             data: HashMap::new(),
///             metadata: PresetMetadata::default(),
///         })
///     }
///
///     fn deserialize(&mut self, preset: &PluginPreset) -> Result<(), PluginError> {
///         self.parameters_from_map(&preset.parameters)
///     }
/// }
/// ```
pub trait SerializablePlugin {
    /// Serialize plugin state to a preset
    fn serialize(&self) -> Result<PluginPreset, PluginError>;

    /// Deserialize plugin state from a preset
    fn deserialize(&mut self, preset: &PluginPreset) -> Result<(), PluginError>;

    /// Get all parameter values as a map
    fn parameters_to_map(&self) -> HashMap<String, ParameterValue>;

    /// Set parameters from a map
    fn parameters_from_map(
        &mut self,
        params: &HashMap<String, ParameterValue>,
    ) -> Result<(), PluginError>;
}

/// A serializable plugin preset
///
/// Presets contain all the state needed to recreate a plugin's configuration.
/// They can be saved to files and loaded later.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginPreset {
    /// Preset name
    pub name: String,

    /// Plugin identifier (unique ID, e.g., "sotf-eq", "sotf-compressor")
    pub plugin_id: String,

    /// Plugin version when preset was created
    pub version: String,

    /// Parameter values
    pub parameters: HashMap<String, ParameterValue>,

    /// Extended data (plugin-specific, serialized as JSON)
    pub data: HashMap<String, serde_json::Value>,

    /// User metadata
    #[serde(default)]
    pub metadata: PresetMetadata,
}

impl PluginPreset {
    /// Create a new preset with basic fields
    pub fn new(name: String, plugin_id: String, version: String) -> Self {
        Self {
            name,
            plugin_id,
            version,
            parameters: HashMap::new(),
            data: HashMap::new(),
            metadata: PresetMetadata::default(),
        }
    }

    /// Check if this preset is compatible with the given plugin ID.
    ///
    /// Compatibility is based on the plugin id alone — version is reported
    /// separately via [`Self::is_version_compatible`] so callers can decide
    /// whether to attempt a [`SerializablePlugin::deserialize`] or trigger a
    /// migration path.
    pub fn is_compatible(&self, plugin_id: &str) -> bool {
        self.plugin_id == plugin_id
    }

    /// Returns true when this preset can be loaded by this host for the current
    /// plugin version.
    ///
    /// Compatibility requires both a matching plugin identifier and a compatible
    /// major-version check.
    pub fn is_loadable_for(&self, plugin_id: &str, current_version: &str) -> bool {
        self.is_compatible(plugin_id) && self.is_version_compatible(current_version)
    }

    /// Returns true when the preset was saved by a plugin version whose major
    /// component matches `current_version`. Semantic-versioning convention:
    /// only major-version bumps break preset format compatibility.
    ///
    /// Both `self.version` and `current_version` should be valid `semver`
    /// strings (`MAJOR.MINOR.PATCH`); leading non-numeric prefixes are
    /// tolerated by parsing the first `.`-separated component as an unsigned
    /// integer. Unparseable versions fall back to strict string equality, so
    /// the function never accepts a clearly unknown format.
    pub fn is_version_compatible(&self, current_version: &str) -> bool {
        match (
            major_component(&self.version),
            major_component(current_version),
        ) {
            (Some(a), Some(b)) => a == b,
            _ => self.version == current_version,
        }
    }

    /// Add a tag to the preset
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        self.metadata.tags.push(tag.into());
    }

    /// Set the author
    pub fn set_author(&mut self, author: impl Into<String>) {
        self.metadata.author = Some(author.into());
    }

    /// Add a comment
    pub fn set_comment(&mut self, comment: impl Into<String>) {
        self.metadata.comment = Some(comment.into());
    }
}

/// Error classification when resolving a preset for version-aware loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetLoadError {
    /// Preset name does not exist in the bank.
    MissingPreset,
    /// Preset exists but is for a different plugin.
    PluginMismatch,
    /// Preset exists for plugin, but major version is incompatible.
    VersionMismatch,
}

/// Extract the major-version component (`<MAJOR>` in `MAJOR.MINOR.PATCH`)
/// as a `u32`. Returns `None` if the string has no leading numeric segment.
fn major_component(version: &str) -> Option<u32> {
    version.split('.').next()?.parse::<u32>().ok()
}

impl Default for PluginPreset {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            plugin_id: "unknown".to_string(),
            version: "0.0.0".to_string(),
            parameters: HashMap::new(),
            data: HashMap::new(),
            metadata: PresetMetadata::default(),
        }
    }
}

/// User metadata for a preset
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PresetMetadata {
    /// Author name
    #[serde(default)]
    pub author: Option<String>,

    /// Creation timestamp
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// User comment/description
    #[serde(default)]
    pub comment: Option<String>,
}

impl PresetMetadata {
    /// Create empty metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the creation timestamp to now
    pub fn set_created_now(&mut self) {
        self.created_at = Some(Utc::now());
    }
}

/// Built-in preset bank
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetBank {
    /// Bank name
    pub name: String,

    /// Presets in the bank
    pub presets: Vec<PluginPreset>,
}

/// Field that contributed to a preset search match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresetSearchField {
    /// Preset display name.
    Name,
    /// Plugin identifier.
    PluginId,
    /// Metadata author.
    Author,
    /// Metadata tag.
    Tag,
    /// Metadata comment.
    Comment,
}

/// Ranked preset search result.
#[derive(Debug, Clone)]
pub struct PresetSearchResult<'a> {
    /// Matched preset.
    pub preset: &'a PluginPreset,
    /// Higher scores are stronger matches.
    pub score: u32,
    /// Fields that matched at least one query term.
    pub matched_fields: Vec<PresetSearchField>,
}

impl PresetBank {
    /// Create a new empty bank
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            presets: Vec::new(),
        }
    }

    /// Add a preset to the bank
    pub fn add_preset(&mut self, preset: PluginPreset) {
        self.presets.push(preset);
    }

    /// Find a preset by name
    pub fn find_preset(&self, name: &str) -> Option<&PluginPreset> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// Find all presets that match both plugin and major-version compatibility.
    pub fn presets_for_plugin_version<'a>(
        &'a self,
        plugin_id: &str,
        current_version: &str,
    ) -> Vec<&'a PluginPreset> {
        self.presets
            .iter()
            .filter(|p| p.is_loadable_for(plugin_id, current_version))
            .collect()
    }

    /// Find a preset by name while keeping plugin/version compatibility explicit.
    pub fn find_preset_for_load(
        &self,
        name: &str,
        plugin_id: &str,
        current_version: &str,
    ) -> Result<&PluginPreset, PresetLoadError> {
        let preset = self
            .find_preset(name)
            .ok_or(PresetLoadError::MissingPreset)?;
        if !preset.is_compatible(plugin_id) {
            return Err(PresetLoadError::PluginMismatch);
        }
        if !preset.is_version_compatible(current_version) {
            return Err(PresetLoadError::VersionMismatch);
        }
        Ok(preset)
    }

    /// Get presets by tag
    pub fn presets_with_tag(&self, tag: &str) -> Vec<&PluginPreset> {
        self.presets
            .iter()
            .filter(|p| p.metadata.tags.contains(&tag.to_string()))
            .collect()
    }

    /// Search presets by name, plugin id, author, tags, and comment.
    ///
    /// The query is split into whitespace-separated terms. All terms must
    /// match at least one searchable field. Results are ranked by match
    /// strength, then by preset name for deterministic ordering.
    pub fn search(&self, query: &str) -> Vec<PresetSearchResult<'_>> {
        let terms = normalize_terms(query);
        if terms.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<PresetSearchResult<'_>> = self
            .presets
            .iter()
            .filter_map(|preset| score_preset(preset, &terms))
            .collect();

        results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.preset.name.cmp(&b.preset.name))
                .then_with(|| a.preset.plugin_id.cmp(&b.preset.plugin_id))
        });

        results
    }
}

fn normalize_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(normalize_search_text)
        .filter(|term| !term.is_empty())
        .collect()
}

fn normalize_search_text(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn score_preset<'a>(preset: &'a PluginPreset, terms: &[String]) -> Option<PresetSearchResult<'a>> {
    let fields = searchable_fields(preset);
    let mut score = 0u32;
    let mut matched_fields = Vec::new();

    for term in terms {
        let mut term_matched = false;
        for (field, value, exact_weight, contains_weight, fuzzy_weight) in &fields {
            let Some(field_score) =
                score_field(value, term, *exact_weight, *contains_weight, *fuzzy_weight)
            else {
                continue;
            };
            score = score.saturating_add(field_score);
            push_unique_field(&mut matched_fields, *field);
            term_matched = true;
        }
        if !term_matched {
            return None;
        }
    }

    Some(PresetSearchResult {
        preset,
        score,
        matched_fields,
    })
}

fn searchable_fields(preset: &PluginPreset) -> Vec<(PresetSearchField, String, u32, u32, u32)> {
    let mut fields = vec![
        (
            PresetSearchField::Name,
            normalize_search_text(&preset.name),
            100,
            60,
            20,
        ),
        (
            PresetSearchField::PluginId,
            normalize_search_text(&preset.plugin_id),
            45,
            35,
            8,
        ),
    ];

    if let Some(author) = preset.metadata.author.as_ref() {
        fields.push((
            PresetSearchField::Author,
            normalize_search_text(author),
            40,
            25,
            8,
        ));
    }
    if let Some(comment) = preset.metadata.comment.as_ref() {
        fields.push((
            PresetSearchField::Comment,
            normalize_search_text(comment),
            25,
            15,
            5,
        ));
    }
    for tag in &preset.metadata.tags {
        fields.push((
            PresetSearchField::Tag,
            normalize_search_text(tag),
            70,
            50,
            12,
        ));
    }

    fields
}

fn score_field(
    field_value: &str,
    term: &str,
    exact_weight: u32,
    contains_weight: u32,
    fuzzy_weight: u32,
) -> Option<u32> {
    if field_value.is_empty() || term.is_empty() {
        return None;
    }
    if field_value == term {
        return Some(exact_weight);
    }
    if field_value.split_whitespace().any(|word| word == term) {
        return Some(exact_weight.saturating_sub(5));
    }
    if field_value.contains(term) {
        return Some(contains_weight);
    }
    if is_subsequence(term, field_value) {
        return Some(fuzzy_weight);
    }
    None
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = needle.chars();
    let Some(mut wanted) = chars.next() else {
        return true;
    };
    for ch in haystack.chars() {
        if ch == wanted {
            match chars.next() {
                Some(next) => wanted = next,
                None => return true,
            }
        }
    }
    false
}

fn push_unique_field(fields: &mut Vec<PresetSearchField>, field: PresetSearchField) {
    if !fields.contains(&field) {
        fields.push(field);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(name: &str, plugin_id: &str, tags: &[&str]) -> PluginPreset {
        let mut preset = PluginPreset::new(name.into(), plugin_id.into(), "1.2.3".into());
        for tag in tags {
            preset.add_tag(*tag);
        }
        preset
    }

    #[test]
    fn presets_with_tag_remains_exact() {
        let mut bank = PresetBank::new("Factory");
        bank.add_preset(preset("Warm Bus", "sotf-eq", &["Bus"]));

        assert_eq!(bank.presets_with_tag("Bus").len(), 1);
        assert!(bank.presets_with_tag("bus").is_empty());
    }

    #[test]
    fn search_matches_name_case_insensitively() {
        let mut bank = PresetBank::new("Factory");
        bank.add_preset(preset("Warm Analog Bus", "sotf-eq", &["mix"]));
        bank.add_preset(preset("Clean Vocal", "sotf-compressor", &["voice"]));

        let results = bank.search("analog");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].preset.name, "Warm Analog Bus");
        assert!(results[0].matched_fields.contains(&PresetSearchField::Name));
    }

    #[test]
    fn search_matches_author_comment_and_tags() {
        let mut bank = PresetBank::new("Factory");
        let mut vocal = preset("Smooth Lead", "sotf-compressor", &["vocal"]);
        vocal.set_author("Ada");
        vocal.set_comment("Gentle leveling for spoken voice");
        bank.add_preset(vocal);

        assert_eq!(bank.search("ada")[0].preset.name, "Smooth Lead");
        assert_eq!(bank.search("spoken")[0].preset.name, "Smooth Lead");
        assert_eq!(bank.search("vocal")[0].preset.name, "Smooth Lead");
    }

    #[test]
    fn search_requires_all_terms_but_allows_partial_terms() {
        let mut bank = PresetBank::new("Factory");
        bank.add_preset(preset("Warm Analog Bus", "sotf-eq", &["mixbus"]));
        bank.add_preset(preset("Warm Vocal", "sotf-compressor", &["voice"]));

        let results = bank.search("warm ana");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].preset.name, "Warm Analog Bus");
    }

    #[test]
    fn search_ranks_exact_name_above_tag_match() {
        let mut bank = PresetBank::new("Factory");
        bank.add_preset(preset("Glue", "sotf-compressor", &["master"]));
        bank.add_preset(preset("Master", "sotf-eq", &["utility"]));

        let results = bank.search("master");

        assert_eq!(results[0].preset.name, "Master");
        assert_eq!(results[1].preset.name, "Glue");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn empty_search_returns_no_results() {
        let mut bank = PresetBank::new("Factory");
        bank.add_preset(preset("Warm Analog Bus", "sotf-eq", &["mix"]));

        assert!(bank.search("  ").is_empty());
    }

    #[test]
    fn is_version_compatible_major_only() {
        let mut bank = PresetBank::new("Factory");
        bank.add_preset(PluginPreset::new(
            "Legacy".into(),
            "sotf-eq".into(),
            "2.4.0".into(),
        ));
        bank.add_preset(PluginPreset::new(
            "Next".into(),
            "sotf-eq".into(),
            "3.0.0".into(),
        ));

        let presets = bank.presets_for_plugin_version("sotf-eq", "2.9.7");
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].name, "Legacy");
    }

    #[test]
    fn find_preset_for_load_reports_errors() {
        let mut bank = PresetBank::new("Factory");
        let mut mismatch = preset("EQPreset", "sotf-eq", &[]);
        mismatch.version = "1.2.9".into();
        bank.add_preset(mismatch);
        bank.add_preset(preset("Comp", "sotf-compressor", &[]));

        assert_eq!(
            bank.find_preset_for_load("Nope", "sotf-eq", "1.2.3"),
            Err(PresetLoadError::MissingPreset)
        );
        assert_eq!(
            bank.find_preset_for_load("EQPreset", "sotf-compressor", "1.2.3"),
            Err(PresetLoadError::PluginMismatch)
        );
        assert_eq!(
            bank.find_preset_for_load("EQPreset", "sotf-eq", "2.0.0"),
            Err(PresetLoadError::VersionMismatch)
        );
        assert!(
            bank.find_preset_for_load("EQPreset", "sotf-eq", "1.2.9")
                .is_ok()
        );
    }

    #[test]
    fn presets_for_plugin_version_only_matches_plugin_and_major() {
        let mut bank = PresetBank::new("Factory");
        bank.add_preset(preset("Good", "sotf-eq", &[]));
        bank.add_preset(preset("BadPlugin", "sotf-compressor", &[]));
        let mut incompatible = preset("OtherMajor", "sotf-eq", &[]);
        incompatible.version = "9.8.7".into();
        bank.add_preset(incompatible);

        let mut good = preset("Another", "sotf-eq", &[]);
        good.version = "1.0.0".into();
        bank.add_preset(good);

        let mut names: Vec<_> = bank
            .presets_for_plugin_version("sotf-eq", "1.2.3")
            .into_iter()
            .map(|p| p.name.as_str())
            .collect();
        names.sort_unstable();

        assert_eq!(names, vec!["Another", "Good"]);
    }
}

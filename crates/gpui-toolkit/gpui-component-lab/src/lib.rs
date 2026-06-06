//! Prop-driven component lab data model for gpui-toolkit.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// Editable story property value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum StoryPropValue {
    Bool(bool),
    Number(f64),
    Text(String),
    Choice(String),
    Color(String),
}

/// Editable story property definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoryProp {
    pub name: String,
    pub label: String,
    pub value: StoryPropValue,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

impl StoryProp {
    pub fn new(name: impl Into<String>, label: impl Into<String>, value: StoryPropValue) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            value,
            options: Vec::new(),
        }
    }

    pub fn options(mut self, options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.options = options.into_iter().map(Into::into).collect();
        self
    }
}

/// Named viewport used by responsive previews.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportPreset {
    pub id: String,
    pub label: String,
    pub width: f32,
    pub height: f32,
}

impl ViewportPreset {
    pub fn new(id: impl Into<String>, label: impl Into<String>, width: f32, height: f32) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            width,
            height,
        }
    }
}

/// Theme/design selector used by stories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePreset {
    pub id: String,
    pub label: String,
    pub design: String,
    pub reduced_motion: bool,
}

impl ThemePreset {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        design: impl Into<String>,
        reduced_motion: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            design: design.into(),
            reduced_motion,
        }
    }
}

/// Prop-driven component story.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentStory {
    pub id: String,
    pub crate_name: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub props: Vec<StoryProp>,
    #[serde(default)]
    pub viewports: Vec<ViewportPreset>,
    #[serde(default)]
    pub themes: Vec<ThemePreset>,
}

impl ComponentStory {
    pub fn new(
        id: impl Into<String>,
        crate_name: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            crate_name: crate_name.into(),
            title: title.into(),
            description: description.into(),
            props: Vec::new(),
            viewports: default_viewports(),
            themes: default_theme_presets(),
        }
    }

    pub fn props(mut self, props: impl IntoIterator<Item = StoryProp>) -> Self {
        self.props = props.into_iter().collect();
        self
    }
}

/// Registry of all stories shown by the lab.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StoryRegistry {
    stories: BTreeMap<String, ComponentStory>,
}

impl StoryRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, story: ComponentStory) -> Result<()> {
        if self.stories.contains_key(&story.id) {
            bail!("duplicate component story id '{}'", story.id);
        }
        self.stories.insert(story.id.clone(), story);
        Ok(())
    }

    pub fn story(&self, id: &str) -> Option<&ComponentStory> {
        self.stories.get(id)
    }

    pub fn stories(&self) -> impl Iterator<Item = &ComponentStory> {
        self.stories.values()
    }

    pub fn len(&self) -> usize {
        self.stories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stories.is_empty()
    }
}

/// One responsive preview cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponsivePreviewCell {
    pub story_id: String,
    pub viewport: ViewportPreset,
    pub theme: ThemePreset,
}

/// Matrix of story previews across viewport/theme combinations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponsivePreviewMatrix {
    pub cells: Vec<ResponsivePreviewCell>,
}

impl ResponsivePreviewMatrix {
    pub fn for_story(story: &ComponentStory) -> Self {
        let mut cells = Vec::new();
        for viewport in &story.viewports {
            for theme in &story.themes {
                cells.push(ResponsivePreviewCell {
                    story_id: story.id.clone(),
                    viewport: viewport.clone(),
                    theme: theme.clone(),
                });
            }
        }
        Self { cells }
    }
}

/// Persisted designer/story state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoryDocument {
    pub story: ComponentStory,
    #[serde(default)]
    pub layout: Value,
}

impl StoryDocument {
    pub fn new(story: ComponentStory) -> Self {
        Self {
            story,
            layout: Value::Object(Default::default()),
        }
    }

    pub fn set_prop_value(&mut self, name: &str, value: StoryPropValue) -> Result<()> {
        let Some(prop) = self.story.props.iter_mut().find(|prop| prop.name == name) else {
            bail!("story '{}' has no prop '{name}'", self.story.id);
        };
        prop.value = value;
        Ok(())
    }

    pub fn save_story_json(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self).context("serialize story document")?;
        std::fs::write(path, json).with_context(|| format!("write {}", path.display()))
    }

    pub fn load_story_json(path: &Path) -> Result<Self> {
        let input =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&input).with_context(|| format!("parse {}", path.display()))
    }
}

/// Build the default lab registry.
pub fn builtin_story_registry() -> Result<StoryRegistry> {
    let mut registry = StoryRegistry::new();
    register_ui_kit_stories(&mut registry)?;
    register_px_stories(&mut registry)?;
    register_audio_kit_stories(&mut registry)?;
    Ok(registry)
}

pub fn register_ui_kit_stories(registry: &mut StoryRegistry) -> Result<()> {
    registry.register(
        ComponentStory::new(
            "ui-kit.button",
            "gpui-ui-kit",
            "Button",
            "Primary action button",
        )
        .props([
            StoryProp::new("label", "Label", StoryPropValue::Text("Save".into())),
            StoryProp::new(
                "variant",
                "Variant",
                StoryPropValue::Choice("primary".into()),
            )
            .options(["primary", "secondary", "destructive", "ghost", "outline"]),
            StoryProp::new("disabled", "Disabled", StoryPropValue::Bool(false)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "ui-kit.form",
            "gpui-ui-kit",
            "Form Controls",
            "Inputs, toggles, selects, and sliders",
        )
        .props([
            StoryProp::new("label", "Label", StoryPropValue::Text("Gain".into())),
            StoryProp::new("value", "Value", StoryPropValue::Number(0.5)),
        ]),
    )
}

pub fn register_px_stories(registry: &mut StoryRegistry) -> Result<()> {
    registry.register(
        ComponentStory::new("px.line", "gpui-px", "Line Chart", "Responsive line chart").props([
            StoryProp::new("series", "Series", StoryPropValue::Choice("sine".into()))
                .options(["sine", "sweep", "flat"]),
            StoryProp::new("fill", "Fill", StoryPropValue::Bool(true)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "px.bar",
            "gpui-px",
            "Bar Chart",
            "Responsive categorical bars",
        )
        .props([StoryProp::new("bars", "Bars", StoryPropValue::Number(8.0))]),
    )
}

pub fn register_audio_kit_stories(registry: &mut StoryRegistry) -> Result<()> {
    registry.register(
        ComponentStory::new(
            "audio-kit.potentiometer",
            "gpui-audio-kit",
            "Potentiometer",
            "Rotary audio parameter control",
        )
        .props([
            StoryProp::new("label", "Label", StoryPropValue::Text("Frequency".into())),
            StoryProp::new("value", "Value", StoryPropValue::Number(1000.0)),
            StoryProp::new(
                "scale",
                "Scale",
                StoryPropValue::Choice("logarithmic".into()),
            )
            .options(["linear", "logarithmic"]),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "audio-kit.meter",
            "gpui-audio-kit",
            "Level Meter",
            "Peak and level metering",
        )
        .props([
            StoryProp::new("level_db", "Level", StoryPropValue::Number(-12.0)),
            StoryProp::new("peak_db", "Peak", StoryPropValue::Number(-3.0)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "audio-kit.spectrum",
            "gpui-audio-kit",
            "Spectrum",
            "Spectrum analyzer element",
        )
        .props([StoryProp::new("bins", "Bins", StoryPropValue::Number(64.0))]),
    )
}

pub fn default_viewports() -> Vec<ViewportPreset> {
    vec![
        ViewportPreset::new("mobile", "Mobile", 390.0, 844.0),
        ViewportPreset::new("tablet", "Tablet", 834.0, 1112.0),
        ViewportPreset::new("desktop", "Desktop", 1280.0, 800.0),
        ViewportPreset::new("wide", "Wide", 1728.0, 1117.0),
    ]
}

pub fn default_theme_presets() -> Vec<ThemePreset> {
    vec![
        ThemePreset::new("neutral", "Neutral", "neutral", false),
        ThemePreset::new("apple-hig", "Apple HIG", "apple_hig", false),
        ThemePreset::new("reduced-motion", "Reduced Motion", "neutral", true),
    ]
}

/// Load all story documents from a directory.
pub fn load_story_documents(dir: &Path) -> Result<Vec<StoryDocument>> {
    let mut docs = Vec::new();
    if !dir.exists() {
        return Ok(docs);
    }
    for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json")
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".story.json"))
        {
            docs.push(StoryDocument::load_story_json(&path)?);
        }
    }
    Ok(docs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_covers_requested_crates() {
        let registry = builtin_story_registry().unwrap();
        assert!(registry.story("ui-kit.button").is_some());
        assert!(registry.story("px.line").is_some());
        assert!(registry.story("audio-kit.potentiometer").is_some());
    }

    #[test]
    fn responsive_matrix_crosses_viewports_and_themes() {
        let registry = builtin_story_registry().unwrap();
        let story = registry.story("audio-kit.meter").unwrap();
        let matrix = ResponsivePreviewMatrix::for_story(story);
        assert_eq!(
            matrix.cells.len(),
            story.viewports.len() * story.themes.len()
        );
    }

    #[test]
    fn designer_story_json_round_trips() {
        let story = builtin_story_registry()
            .unwrap()
            .story("ui-kit.button")
            .unwrap()
            .clone();
        let mut doc = StoryDocument::new(story);
        doc.set_prop_value("label", StoryPropValue::Text("Apply".into()))
            .unwrap();
        let json = serde_json::to_string(&doc).unwrap();
        let parsed: StoryDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.story.props[0].value,
            StoryPropValue::Text("Apply".into())
        );
    }
}

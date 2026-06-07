//! Prop-driven component lab data model for gpui-toolkit.

pub mod lab_ui;

use anyhow::{Context, Result, bail};
use gpui_design_tools::{
    DesignTokenFormat, DesignTokenValidationReport, validate_design_tokens_from_path,
};
use gpui_ui_kit::DesignSystem;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

/// Expected rendered preview behavior used by the conformance gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderedPreviewConformance {
    #[serde(default = "default_preview_min_width")]
    pub min_width: f32,
    #[serde(default = "default_preview_min_height")]
    pub min_height: f32,
    #[serde(default)]
    pub allow_scroll: bool,
}

impl RenderedPreviewConformance {
    pub fn new(min_width: f32, min_height: f32) -> Self {
        Self {
            min_width,
            min_height,
            allow_scroll: false,
        }
    }

    pub fn scrollable(mut self, allow_scroll: bool) -> Self {
        self.allow_scroll = allow_scroll;
        self
    }
}

impl Default for RenderedPreviewConformance {
    fn default() -> Self {
        Self::new(160.0, 120.0)
    }
}

/// Machine-readable conformance expectations for a story.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoryConformance {
    #[serde(default = "default_true")]
    pub responsive: bool,
    #[serde(default)]
    pub rendered: RenderedPreviewConformance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_touch_target: Option<f32>,
    #[serde(default)]
    pub focusable_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_labels: Vec<String>,
    #[serde(default = "default_true")]
    pub builder_layout: bool,
}

impl StoryConformance {
    pub fn display(min_width: f32, min_height: f32) -> Self {
        Self {
            responsive: true,
            rendered: RenderedPreviewConformance::new(min_width, min_height),
            min_touch_target: None,
            focusable_count: 0,
            focus_labels: Vec::new(),
            builder_layout: true,
        }
    }

    pub fn interactive(
        min_width: f32,
        min_height: f32,
        min_touch_target: f32,
        focus_labels: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let focus_labels = focus_labels.into_iter().map(Into::into).collect::<Vec<_>>();
        Self {
            responsive: true,
            rendered: RenderedPreviewConformance::new(min_width, min_height),
            min_touch_target: Some(min_touch_target),
            focusable_count: focus_labels.len().max(1),
            focus_labels,
            builder_layout: true,
        }
    }

    pub fn scrollable_showcase(focus_labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut conformance = Self::interactive(320.0, 220.0, 48.0, focus_labels);
        conformance.rendered = conformance.rendered.scrollable(true);
        conformance
    }

    pub fn px_chart() -> Self {
        Self::display(220.0, 145.0)
    }

    pub fn for_story(story_id: &str, crate_name: &str) -> Self {
        match crate_name {
            "gpui-px" => Self::px_chart(),
            "gpui-audio-kit" => match story_id {
                "audio-kit.potentiometer"
                | "audio-kit.vertical-slider"
                | "audio-kit.volume-knob" => {
                    Self::interactive(140.0, 170.0, 48.0, ["Audio control"])
                }
                "audio-kit.horizontal-meter" => Self::display(320.0, 120.0),
                "audio-kit.spectrum-axis" => Self::display(360.0, 180.0),
                _ => Self::display(140.0, 170.0),
            },
            "gpui-ui-kit" => match story_id {
                "ui-kit.button" => Self::interactive(180.0, 120.0, 48.0, ["Primary action"]),
                "ui-kit.form" => Self::interactive(320.0, 260.0, 48.0, ["Form input"]),
                "ui-kit.status" => Self::display(320.0, 160.0),
                "ui-kit.navigation" => Self::interactive(320.0, 140.0, 48.0, ["Tabs"]),
                "ui-kit.feedback" => Self::display(320.0, 120.0),
                "ui-kit.card" => Self::display(320.0, 180.0),
                _ => Self::scrollable_showcase([story_id.replace("ui-kit.", "")]),
            },
            _ => Self::default(),
        }
    }
}

impl Default for StoryConformance {
    fn default() -> Self {
        Self::display(160.0, 120.0)
    }
}

fn default_true() -> bool {
    true
}

fn default_preview_min_width() -> f32 {
    RenderedPreviewConformance::default().min_width
}

fn default_preview_min_height() -> f32 {
    RenderedPreviewConformance::default().min_height
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

/// Motion preset used by the designer preview controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotionPreset {
    pub id: String,
    pub label: String,
    pub reduced_motion: bool,
}

impl MotionPreset {
    pub fn new(id: impl Into<String>, label: impl Into<String>, reduced_motion: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
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
    #[serde(default)]
    pub motions: Vec<MotionPreset>,
    #[serde(default)]
    pub conformance: StoryConformance,
}

impl ComponentStory {
    pub fn new(
        id: impl Into<String>,
        crate_name: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let crate_name = crate_name.into();
        let conformance = StoryConformance::for_story(&id, &crate_name);
        Self {
            id,
            crate_name,
            title: title.into(),
            description: description.into(),
            props: Vec::new(),
            viewports: default_viewports(),
            themes: default_theme_presets(),
            motions: default_motion_presets(),
            conformance,
        }
    }

    pub fn props(mut self, props: impl IntoIterator<Item = StoryProp>) -> Self {
        self.props = props.into_iter().collect();
        self
    }

    pub fn conformance(mut self, conformance: StoryConformance) -> Self {
        self.conformance = conformance;
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

/// Story ids that have first-party GPUI renderers in the interactive lab.
pub const BUILTIN_RENDERER_STORY_IDS: &[&str] = &[
    "ui-kit.button",
    "ui-kit.form",
    "ui-kit.status",
    "ui-kit.navigation",
    "ui-kit.feedback",
    "ui-kit.card",
    "ui-kit.buttons",
    "ui-kit.text",
    "ui-kit.badges",
    "ui-kit.avatars",
    "ui-kit.form-controls",
    "ui-kit.progress",
    "ui-kit.alerts",
    "ui-kit.tabs",
    "ui-kit.cards",
    "ui-kit.breadcrumbs",
    "ui-kit.spinners",
    "ui-kit.layout",
    "ui-kit.icon-buttons",
    "ui-kit.toasts",
    "ui-kit.dialog",
    "ui-kit.menu",
    "ui-kit.table",
    "ui-kit.tooltips",
    "ui-kit.accordion",
    "ui-kit.wizard",
    "ui-kit.workflow",
    "ui-kit.qr-code",
    "ui-kit.context-menu",
    "ui-kit.popover",
    "ui-kit.sidebar",
    "ui-kit.status-bar",
    "ui-kit.search-bar",
    "ui-kit.keyboard-shortcut",
    "ui-kit.empty-state",
    "ui-kit.confirm-dialog",
    "ui-kit.split-pane",
    "ui-kit.image-view",
    "ui-kit.settings-form",
    "ui-kit.step-indicator",
    "ui-kit.loading-overlay",
    "ui-kit.tag",
    "ui-kit.toolbar",
    "ui-kit.notification",
    "ui-kit.tree-view",
    "ui-kit.drag-list",
    "ui-kit.command-palette",
    "ui-kit.accessibility",
    "audio-kit.potentiometer",
    "audio-kit.vertical-slider",
    "audio-kit.volume-knob",
    "audio-kit.meter",
    "audio-kit.horizontal-meter",
    "audio-kit.spectrum",
    "audio-kit.spectrum-axis",
    "px.line",
    "px.bar",
    "px.scatter",
    "px.area",
    "px.heatmap",
    "px.contour",
    "px.isoline",
    "px.pie",
    "px.boxplot",
    "px.treemap",
    "px.surface3d",
];

pub fn builtin_story_has_renderer(story_id: &str) -> bool {
    BUILTIN_RENDERER_STORY_IDS.contains(&story_id)
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

/// Token file validation result produced by live preview reloads.
#[derive(Debug, Clone, PartialEq)]
pub struct LivePreviewTokenReload {
    pub path: PathBuf,
    pub report: DesignTokenValidationReport,
}

/// Changed story/token state loaded by the component lab live preview loop.
#[derive(Debug, Clone, PartialEq)]
pub struct LivePreviewReload {
    pub latest_modified: SystemTime,
    pub story_documents: Vec<StoryDocument>,
    pub token_reports: Vec<LivePreviewTokenReload>,
}

/// Reload story JSON and token JSON when either has changed since `last_seen`.
pub fn reload_live_preview_state(
    stories_dir: &Path,
    tokens: &[PathBuf],
    last_seen: SystemTime,
) -> Result<Option<LivePreviewReload>> {
    let latest_modified = latest_story_or_token_modified(stories_dir, tokens)?;
    if latest_modified <= last_seen {
        return Ok(None);
    }

    let story_documents = load_story_documents(stories_dir)?;
    let mut token_reports = Vec::new();
    for token in tokens {
        let report =
            validate_design_tokens_from_path(token, DesignTokenFormat::StyleDictionaryJson)
                .with_context(|| format!("validate {}", token.display()))?;
        token_reports.push(LivePreviewTokenReload {
            path: token.clone(),
            report,
        });
    }

    Ok(Some(LivePreviewReload {
        latest_modified,
        story_documents,
        token_reports,
    }))
}

/// Latest modified time among `*.story.json` documents and watched token files.
pub fn latest_story_or_token_modified(dir: &Path, tokens: &[PathBuf]) -> Result<SystemTime> {
    let mut latest = latest_story_modified(dir)?;
    for token in tokens {
        if token.exists() {
            latest = latest.max(token.metadata()?.modified()?);
        }
    }
    Ok(latest)
}

/// Latest modified time for story documents in a directory.
pub fn latest_story_modified(dir: &Path) -> Result<SystemTime> {
    let mut latest = SystemTime::UNIX_EPOCH;
    if !dir.exists() {
        return Ok(latest);
    }
    for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".story.json"))
        {
            latest = latest.max(entry.metadata()?.modified()?);
        }
    }
    Ok(latest)
}

/// Latest modified time for Rust/TOML sources under a root, ignoring target dirs.
pub fn latest_rust_source_modified(root: &Path) -> Result<SystemTime> {
    let mut latest = SystemTime::UNIX_EPOCH;
    if !root.exists() {
        return Ok(latest);
    }
    for entry in std::fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "target")
            {
                continue;
            }
            latest = latest.max(latest_rust_source_modified(&path)?);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == "rs" || ext == "toml")
        {
            latest = latest.max(entry.metadata()?.modified()?);
        }
    }
    Ok(latest)
}

/// One actionable component-lab conformance finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComponentLabConformanceFinding {
    pub id: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story_id: Option<String>,
    pub message: String,
}

impl ComponentLabConformanceFinding {
    fn new(
        category: impl Into<String>,
        id: impl Into<String>,
        story_id: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category: category.into(),
            id: id.into(),
            story_id: story_id.map(str::to_string),
            message: message.into(),
        }
    }
}

/// CI-facing component lab conformance report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComponentLabConformanceReport {
    pub passed: bool,
    pub story_count: usize,
    pub token_preset_count: usize,
    pub token_count: usize,
    pub findings: Vec<ComponentLabConformanceFinding>,
    pub design_conformance_markdown: String,
}

impl ComponentLabConformanceReport {
    fn new(
        story_count: usize,
        token_report: &DesignTokenValidationReport,
        findings: Vec<ComponentLabConformanceFinding>,
    ) -> Self {
        Self {
            passed: findings.is_empty(),
            story_count,
            token_preset_count: token_report.preset_count,
            token_count: token_report.token_count,
            findings,
            design_conformance_markdown: token_report.conformance_markdown.clone(),
        }
    }

    pub fn passed(&self) -> bool {
        self.passed
    }

    pub fn to_markdown(&self) -> String {
        let mut output = String::from("# GPUI Component Lab Conformance\n\n");
        output.push_str(&format!(
            "- stories: {}\n- token presets: {}\n- tokens: {}\n- status: {}\n\n",
            self.story_count,
            self.token_preset_count,
            self.token_count,
            if self.passed { "pass" } else { "fail" }
        ));
        output.push_str("## Design Tokens\n\n");
        output.push_str(&self.design_conformance_markdown);
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("\n## Component Findings\n\n");
        if self.findings.is_empty() {
            output.push_str("No component-lab findings.\n");
        } else {
            output.push_str("| category | id | story | message |\n");
            output.push_str("| --- | --- | --- | --- |\n");
            for finding in &self.findings {
                output.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    finding.category,
                    finding.id,
                    finding.story_id.as_deref().unwrap_or("-"),
                    finding.message.replace('|', "\\|")
                ));
            }
        }
        output
    }
}

/// Validate story metadata, persisted designer state, responsive constraints,
/// reduced-motion coverage, and the DesignSystem token report used by the lab.
pub fn validate_component_lab_conformance(
    registry: &StoryRegistry,
    documents: &[StoryDocument],
    token_report: &DesignTokenValidationReport,
) -> ComponentLabConformanceReport {
    let mut findings = Vec::new();

    if registry.is_empty() {
        findings.push(ComponentLabConformanceFinding::new(
            "registry",
            "registry.empty",
            None,
            "component lab must register at least one story",
        ));
    }

    for token_finding in &token_report.findings {
        findings.push(ComponentLabConformanceFinding::new(
            "tokens",
            "tokens.validation",
            None,
            token_finding.clone(),
        ));
    }

    for story in registry.stories() {
        validate_story_conformance(story, &mut findings);
    }

    for document in documents {
        validate_document_conformance(registry, document, &mut findings);
    }

    ComponentLabConformanceReport::new(registry.len(), token_report, findings)
}

pub fn ensure_component_lab_conformance_passed(
    report: &ComponentLabConformanceReport,
) -> Result<()> {
    if report.passed() {
        Ok(())
    } else {
        bail!(
            "component lab conformance failed: {}",
            report
                .findings
                .iter()
                .map(|finding| format!("{}:{}", finding.category, finding.id))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn validate_story_conformance(
    story: &ComponentStory,
    findings: &mut Vec<ComponentLabConformanceFinding>,
) {
    if story.id.trim().is_empty() {
        findings.push(ComponentLabConformanceFinding::new(
            "registry",
            "story.id",
            None,
            "story id must not be empty",
        ));
    }
    if story.title.trim().is_empty() || story.description.trim().is_empty() {
        findings.push(ComponentLabConformanceFinding::new(
            "accessibility",
            "story.label",
            Some(&story.id),
            "story title and description are used as preview/accessibility metadata",
        ));
    }
    if story.crate_name.trim().is_empty() {
        findings.push(ComponentLabConformanceFinding::new(
            "registry",
            "story.crate",
            Some(&story.id),
            "story crate_name must not be empty",
        ));
    }

    validate_viewports(story, findings);
    validate_theme_presets(story, findings);
    validate_motion_presets(story, findings);
    validate_props(story, findings);
    validate_preview_conformance(story, findings);
    validate_touch_target_conformance(story, findings);
    validate_focus_metadata_conformance(story, findings);
    validate_renderer_coverage(story, findings);
}

fn validate_preview_conformance(
    story: &ComponentStory,
    findings: &mut Vec<ComponentLabConformanceFinding>,
) {
    let rendered = &story.conformance.rendered;
    if !rendered.min_width.is_finite()
        || !rendered.min_height.is_finite()
        || rendered.min_width <= 0.0
        || rendered.min_height <= 0.0
    {
        findings.push(ComponentLabConformanceFinding::new(
            "rendered",
            "rendered.bounds",
            Some(&story.id),
            "rendered preview minimum size must be finite and positive",
        ));
    }

    if !story.conformance.responsive {
        findings.push(ComponentLabConformanceFinding::new(
            "responsive",
            "responsive.disabled",
            Some(&story.id),
            "first-party stories must opt into responsive preview conformance",
        ));
    }

    if !rendered.allow_scroll {
        for viewport in &story.viewports {
            if rendered.min_width > viewport.width || rendered.min_height > viewport.height {
                findings.push(ComponentLabConformanceFinding::new(
                    "rendered",
                    "rendered.overflow",
                    Some(&story.id),
                    format!(
                        "rendered minimum {}x{} exceeds viewport '{}' {}x{} without scroll",
                        rendered.min_width,
                        rendered.min_height,
                        viewport.id,
                        viewport.width,
                        viewport.height
                    ),
                ));
            }
        }
    }
}

fn validate_touch_target_conformance(
    story: &ComponentStory,
    findings: &mut Vec<ComponentLabConformanceFinding>,
) {
    let Some(min_touch_target) = story.conformance.min_touch_target else {
        return;
    };
    if !min_touch_target.is_finite() || min_touch_target <= 0.0 {
        findings.push(ComponentLabConformanceFinding::new(
            "accessibility",
            "touch.target",
            Some(&story.id),
            "minimum touch target metadata must be finite and positive",
        ));
        return;
    }

    for theme in &story.themes {
        let Some(design) = DesignSystem::from_language_id(&theme.design) else {
            continue;
        };
        let required = design.interaction.min_touch_target;
        if min_touch_target + f32::EPSILON < required {
            findings.push(ComponentLabConformanceFinding::new(
                "accessibility",
                "touch.target",
                Some(&story.id),
                format!(
                    "minimum touch target {min_touch_target}px is smaller than '{}' design requirement {required}px",
                    theme.id
                ),
            ));
        }
    }
}

fn validate_focus_metadata_conformance(
    story: &ComponentStory,
    findings: &mut Vec<ComponentLabConformanceFinding>,
) {
    if story.conformance.focusable_count == 0 {
        return;
    }

    if story.conformance.min_touch_target.is_none() {
        findings.push(ComponentLabConformanceFinding::new(
            "accessibility",
            "focus.touch_target",
            Some(&story.id),
            "focusable stories must declare minimum touch target metadata",
        ));
    }

    if story.conformance.focus_labels.len() < story.conformance.focusable_count {
        findings.push(ComponentLabConformanceFinding::new(
            "accessibility",
            "focus.labels",
            Some(&story.id),
            format!(
                "focusable story declares {} focus target(s) but only {} label(s)",
                story.conformance.focusable_count,
                story.conformance.focus_labels.len()
            ),
        ));
    }

    if story
        .conformance
        .focus_labels
        .iter()
        .any(|label| label.trim().is_empty())
    {
        findings.push(ComponentLabConformanceFinding::new(
            "accessibility",
            "focus.label",
            Some(&story.id),
            "focus labels must not be empty",
        ));
    }
}

fn validate_renderer_coverage(
    story: &ComponentStory,
    findings: &mut Vec<ComponentLabConformanceFinding>,
) {
    if matches!(
        story.crate_name.as_str(),
        "gpui-ui-kit" | "gpui-px" | "gpui-audio-kit"
    ) && !builtin_story_has_renderer(&story.id)
    {
        findings.push(ComponentLabConformanceFinding::new(
            "registry",
            "renderer.coverage",
            Some(&story.id),
            "first-party toolkit stories must have an interactive lab renderer",
        ));
    }
}

fn validate_viewports(story: &ComponentStory, findings: &mut Vec<ComponentLabConformanceFinding>) {
    if story.viewports.is_empty() {
        findings.push(ComponentLabConformanceFinding::new(
            "responsive",
            "viewports.empty",
            Some(&story.id),
            "stories must provide responsive viewport presets",
        ));
        return;
    }

    let mut ids = BTreeSet::new();
    for viewport in &story.viewports {
        if !ids.insert(viewport.id.as_str()) {
            findings.push(ComponentLabConformanceFinding::new(
                "responsive",
                "viewports.duplicate",
                Some(&story.id),
                format!("duplicate viewport id '{}'", viewport.id),
            ));
        }
        if viewport.id.trim().is_empty() || viewport.label.trim().is_empty() {
            findings.push(ComponentLabConformanceFinding::new(
                "responsive",
                "viewports.metadata",
                Some(&story.id),
                "viewport id and label must not be empty",
            ));
        }
        if !viewport.width.is_finite()
            || !viewport.height.is_finite()
            || viewport.width < 320.0
            || viewport.height < 240.0
        {
            findings.push(ComponentLabConformanceFinding::new(
                "responsive",
                "viewports.bounds",
                Some(&story.id),
                format!(
                    "viewport '{}' must be finite and at least 320x240",
                    viewport.id
                ),
            ));
        }
    }

    for required in ["mobile", "tablet", "desktop", "wide"] {
        if !story
            .viewports
            .iter()
            .any(|viewport| viewport.id == required)
        {
            findings.push(ComponentLabConformanceFinding::new(
                "responsive",
                "viewports.coverage",
                Some(&story.id),
                format!("missing '{required}' viewport preset"),
            ));
        }
    }
}

fn validate_theme_presets(
    story: &ComponentStory,
    findings: &mut Vec<ComponentLabConformanceFinding>,
) {
    if story.themes.is_empty() {
        findings.push(ComponentLabConformanceFinding::new(
            "design",
            "themes.empty",
            Some(&story.id),
            "stories must provide design/theme presets",
        ));
        return;
    }

    let mut ids = BTreeSet::new();
    for theme in &story.themes {
        if !ids.insert(theme.id.as_str()) {
            findings.push(ComponentLabConformanceFinding::new(
                "design",
                "themes.duplicate",
                Some(&story.id),
                format!("duplicate theme id '{}'", theme.id),
            ));
        }
        if theme.id.trim().is_empty()
            || theme.label.trim().is_empty()
            || theme.design.trim().is_empty()
        {
            findings.push(ComponentLabConformanceFinding::new(
                "design",
                "themes.metadata",
                Some(&story.id),
                "theme id, label, and design must not be empty",
            ));
        }
        if DesignSystem::from_language_id(&theme.design).is_none() {
            findings.push(ComponentLabConformanceFinding::new(
                "design",
                "themes.design",
                Some(&story.id),
                format!(
                    "theme '{}' uses unknown design '{}'",
                    theme.id, theme.design
                ),
            ));
        }
    }

    if !story.themes.iter().any(|theme| theme.id == "neutral") {
        findings.push(ComponentLabConformanceFinding::new(
            "design",
            "themes.neutral",
            Some(&story.id),
            "stories must include the neutral DesignSystem preset",
        ));
    }
}

fn validate_motion_presets(
    story: &ComponentStory,
    findings: &mut Vec<ComponentLabConformanceFinding>,
) {
    if story.motions.is_empty() {
        findings.push(ComponentLabConformanceFinding::new(
            "motion",
            "motion.empty",
            Some(&story.id),
            "stories must provide motion presets",
        ));
        return;
    }

    let mut ids = BTreeSet::new();
    for motion in &story.motions {
        if !ids.insert(motion.id.as_str()) {
            findings.push(ComponentLabConformanceFinding::new(
                "motion",
                "motion.duplicate",
                Some(&story.id),
                format!("duplicate motion id '{}'", motion.id),
            ));
        }
        if motion.id.trim().is_empty() || motion.label.trim().is_empty() {
            findings.push(ComponentLabConformanceFinding::new(
                "motion",
                "motion.metadata",
                Some(&story.id),
                "motion id and label must not be empty",
            ));
        }
    }

    if !story.motions.iter().any(|motion| !motion.reduced_motion) {
        findings.push(ComponentLabConformanceFinding::new(
            "motion",
            "motion.standard",
            Some(&story.id),
            "stories must include a standard-motion preset",
        ));
    }
    if !story.motions.iter().any(|motion| motion.reduced_motion) {
        findings.push(ComponentLabConformanceFinding::new(
            "motion",
            "motion.reduced",
            Some(&story.id),
            "stories must include a reduced-motion preset",
        ));
    }
}

fn validate_props(story: &ComponentStory, findings: &mut Vec<ComponentLabConformanceFinding>) {
    let mut names = BTreeSet::new();
    for prop in &story.props {
        if prop.name.trim().is_empty() {
            findings.push(ComponentLabConformanceFinding::new(
                "props",
                "props.name",
                Some(&story.id),
                "prop names must not be empty",
            ));
        }
        if !names.insert(prop.name.as_str()) {
            findings.push(ComponentLabConformanceFinding::new(
                "props",
                "props.duplicate",
                Some(&story.id),
                format!("duplicate prop '{}'", prop.name),
            ));
        }
        if prop.label.trim().is_empty() {
            findings.push(ComponentLabConformanceFinding::new(
                "accessibility",
                "props.label",
                Some(&story.id),
                format!("prop '{}' must have a visible editor label", prop.name),
            ));
        }
        if let StoryPropValue::Choice(value) = &prop.value {
            if prop.options.is_empty() {
                findings.push(ComponentLabConformanceFinding::new(
                    "props",
                    "choice.options",
                    Some(&story.id),
                    format!("choice prop '{}' must provide options", prop.name),
                ));
            }
            if !prop.options.iter().any(|option| option == value) {
                findings.push(ComponentLabConformanceFinding::new(
                    "props",
                    "choice.value",
                    Some(&story.id),
                    format!(
                        "choice prop '{}' current value '{}' is not in options",
                        prop.name, value
                    ),
                ));
            }
            if prop.options.iter().any(|option| option.trim().is_empty()) {
                findings.push(ComponentLabConformanceFinding::new(
                    "accessibility",
                    "choice.option_label",
                    Some(&story.id),
                    format!("choice prop '{}' has an empty option label", prop.name),
                ));
            }
        }
    }
}

fn validate_document_conformance(
    registry: &StoryRegistry,
    document: &StoryDocument,
    findings: &mut Vec<ComponentLabConformanceFinding>,
) {
    let story_id = document.story.id.as_str();
    let Some(story) = registry.story(story_id) else {
        findings.push(ComponentLabConformanceFinding::new(
            "registry",
            "story.unknown",
            Some(story_id),
            "story document is not registered in the lab registry",
        ));
        return;
    };

    let selected_viewport = layout_string(&document.layout, "viewport");
    let selected_theme = layout_string(&document.layout, "theme");
    let selected_motion = layout_string(&document.layout, "motion");

    if let Some(viewport_id) = selected_viewport.as_deref()
        && !story
            .viewports
            .iter()
            .any(|viewport| viewport.id.as_str() == viewport_id)
    {
        findings.push(ComponentLabConformanceFinding::new(
            "responsive",
            "layout.viewport",
            Some(story_id),
            format!("saved viewport '{viewport_id}' is not defined by the story"),
        ));
    }
    if let Some(theme_id) = selected_theme.as_deref()
        && !story
            .themes
            .iter()
            .any(|theme| theme.id.as_str() == theme_id)
    {
        findings.push(ComponentLabConformanceFinding::new(
            "design",
            "layout.theme",
            Some(story_id),
            format!("saved theme '{theme_id}' is not defined by the story"),
        ));
    }
    if let Some(motion_id) = selected_motion.as_deref()
        && !story
            .motions
            .iter()
            .any(|motion| motion.id.as_str() == motion_id)
    {
        findings.push(ComponentLabConformanceFinding::new(
            "motion",
            "layout.motion",
            Some(story_id),
            format!("saved motion '{motion_id}' is not defined by the story"),
        ));
    }

    validate_layout_constraints(story, document, selected_viewport.as_deref(), findings);
    validate_builder_layout(story, document, findings);
}

fn validate_layout_constraints(
    story: &ComponentStory,
    document: &StoryDocument,
    selected_viewport: Option<&str>,
    findings: &mut Vec<ComponentLabConformanceFinding>,
) {
    let Some(raw) = document.layout.get("constraints") else {
        return;
    };
    let story_id = story.id.as_str();
    let Some(raw) = raw.as_object() else {
        findings.push(ComponentLabConformanceFinding::new(
            "responsive",
            "layout.constraints",
            Some(story_id),
            "layout constraints must be a JSON object",
        ));
        return;
    };

    let sizing = raw.get("sizing").and_then(Value::as_str).unwrap_or("fill");
    if !matches!(sizing, "fill" | "fit" | "fixed") {
        findings.push(ComponentLabConformanceFinding::new(
            "responsive",
            "layout.sizing",
            Some(story_id),
            "layout sizing must be one of fill, fit, or fixed",
        ));
    }

    let min_width = number_field(raw, "min_width");
    let min_height = number_field(raw, "min_height");
    let aspect_ratio = number_field(raw, "aspect_ratio");
    let padding = number_field(raw, "padding");

    if min_width.is_some_and(|value| !(160.0..=1600.0).contains(&value)) {
        findings.push(ComponentLabConformanceFinding::new(
            "responsive",
            "layout.min_width",
            Some(story_id),
            "layout min_width must be in [160, 1600]",
        ));
    }
    if min_height.is_some_and(|value| !(120.0..=1200.0).contains(&value)) {
        findings.push(ComponentLabConformanceFinding::new(
            "responsive",
            "layout.min_height",
            Some(story_id),
            "layout min_height must be in [120, 1200]",
        ));
    }
    if aspect_ratio.is_some_and(|value| !(0.5..=3.0).contains(&value)) {
        findings.push(ComponentLabConformanceFinding::new(
            "responsive",
            "layout.aspect_ratio",
            Some(story_id),
            "layout aspect_ratio must be in [0.5, 3.0]",
        ));
    }
    if padding.is_some_and(|value| !(0.0..=80.0).contains(&value)) {
        findings.push(ComponentLabConformanceFinding::new(
            "responsive",
            "layout.padding",
            Some(story_id),
            "layout padding must be in [0, 80]",
        ));
    }

    if sizing == "fixed"
        && let Some(viewport_id) = selected_viewport
        && let Some(viewport) = story
            .viewports
            .iter()
            .find(|viewport| viewport.id.as_str() == viewport_id)
    {
        if min_width.is_some_and(|value| value > viewport.width as f64)
            || min_height.is_some_and(|value| value > viewport.height as f64)
        {
            findings.push(ComponentLabConformanceFinding::new(
                "responsive",
                "layout.overflow",
                Some(story_id),
                format!("fixed layout overflows selected viewport '{viewport_id}'"),
            ));
        }
    }
}

fn validate_builder_layout(
    story: &ComponentStory,
    document: &StoryDocument,
    findings: &mut Vec<ComponentLabConformanceFinding>,
) {
    if !story.conformance.builder_layout {
        return;
    }
    let Some(raw) = document.layout.get("builder") else {
        return;
    };
    let story_id = story.id.as_str();
    let Some(raw) = raw.as_object() else {
        findings.push(ComponentLabConformanceFinding::new(
            "builder",
            "builder.layout",
            Some(story_id),
            "builder layout state must be a JSON object",
        ));
        return;
    };

    validate_enum_field(
        story_id,
        raw,
        "horizontal_align",
        &["start", "center", "end", "stretch"],
        "builder.horizontal_align",
        findings,
    );
    validate_enum_field(
        story_id,
        raw,
        "vertical_align",
        &["start", "center", "end", "stretch"],
        "builder.vertical_align",
        findings,
    );
    validate_enum_field(
        story_id,
        raw,
        "overflow",
        &["hidden", "scroll", "visible"],
        "builder.overflow",
        findings,
    );
    validate_enum_field(
        story_id,
        raw,
        "surface",
        &["background", "surface", "transparent"],
        "builder.surface",
        findings,
    );

    if raw
        .get("gap")
        .and_then(Value::as_f64)
        .is_some_and(|value| !(0.0..=80.0).contains(&value))
    {
        findings.push(ComponentLabConformanceFinding::new(
            "builder",
            "builder.gap",
            Some(story_id),
            "builder gap must be in [0, 80]",
        ));
    }
    if raw.get("border").is_some_and(|value| !value.is_boolean()) {
        findings.push(ComponentLabConformanceFinding::new(
            "builder",
            "builder.border",
            Some(story_id),
            "builder border must be a boolean",
        ));
    }
}

fn validate_enum_field(
    story_id: &str,
    raw: &serde_json::Map<String, Value>,
    field: &str,
    allowed: &[&str],
    id: &str,
    findings: &mut Vec<ComponentLabConformanceFinding>,
) {
    let Some(value) = raw.get(field) else {
        return;
    };
    let Some(value) = value.as_str() else {
        findings.push(ComponentLabConformanceFinding::new(
            "builder",
            id,
            Some(story_id),
            format!("builder field '{field}' must be a string"),
        ));
        return;
    };
    if !allowed.contains(&value) {
        findings.push(ComponentLabConformanceFinding::new(
            "builder",
            id,
            Some(story_id),
            format!(
                "builder field '{field}' must be one of {}",
                allowed.join(", ")
            ),
        ));
    }
}

fn layout_string(layout: &Value, key: &str) -> Option<String> {
    layout.get(key).and_then(Value::as_str).map(str::to_string)
}

fn number_field(raw: &serde_json::Map<String, Value>, key: &str) -> Option<f64> {
    raw.get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
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
    )?;
    registry.register(
        ComponentStory::new(
            "ui-kit.status",
            "gpui-ui-kit",
            "Status Indicators",
            "Badges and progress indicators",
        )
        .props([
            StoryProp::new("label", "Label", StoryPropValue::Text("Ready".into())),
            StoryProp::new(
                "variant",
                "Variant",
                StoryPropValue::Choice("success".into()),
            )
            .options(["default", "primary", "success", "warning", "error", "info"]),
            StoryProp::new("value", "Progress", StoryPropValue::Number(0.72)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "ui-kit.navigation",
            "gpui-ui-kit",
            "Tabs",
            "Segmented navigation tabs",
        )
        .props([
            StoryProp::new("variant", "Variant", StoryPropValue::Choice("pills".into())).options([
                "underline",
                "enclosed",
                "pills",
                "vertical_card",
            ]),
            StoryProp::new("selected", "Selected", StoryPropValue::Number(1.0)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "ui-kit.feedback",
            "gpui-ui-kit",
            "Feedback",
            "Alerts and inline feedback states",
        )
        .props([
            StoryProp::new("variant", "Variant", StoryPropValue::Choice("info".into()))
                .options(["info", "success", "warning", "error"]),
            StoryProp::new(
                "message",
                "Message",
                StoryPropValue::Text("Design tokens validated".into()),
            ),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "ui-kit.card",
            "gpui-ui-kit",
            "Card",
            "Header, content, and footer slots",
        )
        .props([
            StoryProp::new("title", "Title", StoryPropValue::Text("Preview".into())),
            StoryProp::new(
                "content",
                "Content",
                StoryPropValue::Text("Responsive component composition".into()),
            ),
        ]),
    )?;

    register_ui_kit_showcase_stories(registry)
}

const UI_KIT_SHOWCASE_STORIES: &[(&str, &str, &str)] = &[
    (
        "ui-kit.buttons",
        "Buttons",
        "Button variants, sizes, and states",
    ),
    ("ui-kit.text", "Text", "Typography, code, and link styles"),
    (
        "ui-kit.badges",
        "Badges",
        "Badge variants, dots, and sizing",
    ),
    (
        "ui-kit.avatars",
        "Avatars",
        "Avatar, group, shape, and status states",
    ),
    (
        "ui-kit.form-controls",
        "Form Controls",
        "Inputs, selects, sliders, toggles, and button sets",
    ),
    (
        "ui-kit.progress",
        "Progress",
        "Linear and circular progress indicators",
    ),
    ("ui-kit.alerts", "Alerts", "Alert and inline alert variants"),
    ("ui-kit.tabs", "Tabs", "Tab navigation variants and states"),
    ("ui-kit.cards", "Cards", "Card composition and slot styling"),
    ("ui-kit.breadcrumbs", "Breadcrumbs", "Breadcrumb navigation"),
    (
        "ui-kit.spinners",
        "Spinners",
        "Spinner and loading dot states",
    ),
    (
        "ui-kit.layout",
        "Layout",
        "Stack, spacer, and divider primitives",
    ),
    (
        "ui-kit.icon-buttons",
        "Icon Buttons",
        "Icon-only action buttons and states",
    ),
    (
        "ui-kit.toasts",
        "Toasts",
        "Toast container and notification states",
    ),
    ("ui-kit.dialog", "Dialog", "Modal dialog composition"),
    ("ui-kit.menu", "Menu", "Menu bar and menu item states"),
    (
        "ui-kit.table",
        "Table",
        "Table sorting, selection, and pagination",
    ),
    (
        "ui-kit.tooltips",
        "Tooltips",
        "Tooltip placements and triggers",
    ),
    (
        "ui-kit.accordion",
        "Accordion",
        "Accordion disclosure groups",
    ),
    ("ui-kit.wizard", "Wizard", "Step-based wizard navigation"),
    (
        "ui-kit.workflow",
        "Workflow",
        "Workflow canvas building blocks",
    ),
    ("ui-kit.qr-code", "QR Code", "QR code rendering"),
    (
        "ui-kit.context-menu",
        "Context Menu",
        "Context menu trigger patterns",
    ),
    ("ui-kit.popover", "Popover", "Popover placement and content"),
    ("ui-kit.sidebar", "Sidebar", "Sidebar layout variants"),
    (
        "ui-kit.status-bar",
        "Status Bar",
        "Status bar regions and items",
    ),
    ("ui-kit.search-bar", "Search Bar", "Search input states"),
    (
        "ui-kit.keyboard-shortcut",
        "Keyboard Shortcuts",
        "Keyboard shortcut label rendering",
    ),
    ("ui-kit.empty-state", "Empty State", "Empty state messaging"),
    (
        "ui-kit.confirm-dialog",
        "Confirm Dialog",
        "Confirmation dialog variants",
    ),
    (
        "ui-kit.split-pane",
        "Split Pane",
        "Resizable split pane behavior",
    ),
    (
        "ui-kit.image-view",
        "Image View",
        "Image fitting and preview states",
    ),
    (
        "ui-kit.settings-form",
        "Settings Form",
        "Settings form rows and grouping",
    ),
    (
        "ui-kit.step-indicator",
        "Step Indicator",
        "Horizontal and vertical step indicators",
    ),
    (
        "ui-kit.loading-overlay",
        "Loading Overlay",
        "Loading overlay states",
    ),
    ("ui-kit.tag", "Tag", "Tag states and removable tags"),
    ("ui-kit.toolbar", "Toolbar", "Toolbar action groups"),
    (
        "ui-kit.notification",
        "Notification",
        "Notification surface states",
    ),
    ("ui-kit.tree-view", "Tree View", "Tree hierarchy rendering"),
    ("ui-kit.drag-list", "Drag List", "Reorderable list preview"),
    (
        "ui-kit.command-palette",
        "Command Palette",
        "Command palette search and item states",
    ),
    (
        "ui-kit.accessibility",
        "Accessibility",
        "Accessible labeling and focus metadata examples",
    ),
];

fn register_ui_kit_showcase_stories(registry: &mut StoryRegistry) -> Result<()> {
    for (id, title, description) in UI_KIT_SHOWCASE_STORIES {
        registry.register(ComponentStory::new(
            *id,
            "gpui-ui-kit",
            *title,
            *description,
        ))?;
    }
    Ok(())
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
        .props([
            StoryProp::new("bars", "Bars", StoryPropValue::Number(8.0)),
            StoryProp::new("fill", "Fill", StoryPropValue::Bool(true)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "px.scatter",
            "gpui-px",
            "Scatter Chart",
            "Responsive point cloud chart",
        )
        .props([
            StoryProp::new("points", "Points", StoryPropValue::Number(48.0)),
            StoryProp::new("fill", "Fill", StoryPropValue::Bool(true)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "px.area",
            "gpui-px",
            "Area Chart",
            "Responsive filled area chart",
        )
        .props([
            StoryProp::new(
                "series",
                "Series",
                StoryPropValue::Choice("envelope".into()),
            )
            .options(["envelope", "decay", "baseline"]),
            StoryProp::new("fill", "Fill", StoryPropValue::Bool(true)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "px.heatmap",
            "gpui-px",
            "Heatmap",
            "Responsive scalar-field heatmap",
        )
        .props([
            StoryProp::new("size", "Grid Size", StoryPropValue::Number(18.0)),
            StoryProp::new(
                "scale",
                "Color Scale",
                StoryPropValue::Choice("viridis".into()),
            )
            .options(["viridis", "plasma", "inferno", "heat", "coolwarm", "greys"]),
            StoryProp::new("fill", "Fill", StoryPropValue::Bool(true)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "px.contour",
            "gpui-px",
            "Contour Chart",
            "Responsive filled contour bands",
        )
        .props([
            StoryProp::new("size", "Grid Size", StoryPropValue::Number(24.0)),
            StoryProp::new("fill", "Fill", StoryPropValue::Bool(true)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "px.isoline",
            "gpui-px",
            "Isoline Chart",
            "Responsive contour line chart",
        )
        .props([
            StoryProp::new("size", "Grid Size", StoryPropValue::Number(24.0)),
            StoryProp::new("fill", "Fill", StoryPropValue::Bool(true)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "px.pie",
            "gpui-px",
            "Pie Chart",
            "Responsive pie and donut chart",
        )
        .props([
            StoryProp::new("donut", "Donut", StoryPropValue::Bool(true)),
            StoryProp::new("slices", "Slices", StoryPropValue::Number(5.0)),
            StoryProp::new("fill", "Fill", StoryPropValue::Bool(true)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "px.boxplot",
            "gpui-px",
            "Box Plot",
            "Responsive grouped distribution chart",
        )
        .props([
            StoryProp::new("groups", "Groups", StoryPropValue::Number(5.0)),
            StoryProp::new("fill", "Fill", StoryPropValue::Bool(true)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "px.treemap",
            "gpui-px",
            "Treemap",
            "Responsive hierarchy chart",
        )
        .props([
            StoryProp::new(
                "tiling",
                "Tiling",
                StoryPropValue::Choice("squarify".into()),
            )
            .options(["squarify", "binary", "slice", "dice"]),
            StoryProp::new("fill", "Fill", StoryPropValue::Bool(true)),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "px.surface3d",
            "gpui-px",
            "3D Surface",
            "Responsive GPU-backed 3D surface chart",
        )
        .props([
            StoryProp::new("size", "Grid Size", StoryPropValue::Number(22.0)),
            StoryProp::new(
                "colormap",
                "Colormap",
                StoryPropValue::Choice("viridis".into()),
            )
            .options(["viridis", "plasma", "inferno", "turbo", "coolwarm"]),
            StoryProp::new("wireframe", "Wireframe", StoryPropValue::Bool(false)),
            StoryProp::new("fill", "Fill", StoryPropValue::Bool(true)),
        ]),
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
            "audio-kit.vertical-slider",
            "gpui-audio-kit",
            "Vertical Slider",
            "Vertical audio parameter fader",
        )
        .props([
            StoryProp::new("label", "Label", StoryPropValue::Text("Gain".into())),
            StoryProp::new("value", "Value", StoryPropValue::Number(-6.0)),
            StoryProp::new("min", "Min", StoryPropValue::Number(-60.0)),
            StoryProp::new("max", "Max", StoryPropValue::Number(6.0)),
            StoryProp::new("peak", "Peak", StoryPropValue::Number(-1.5)),
            StoryProp::new("ticks", "Ticks", StoryPropValue::Bool(true)),
            StoryProp::new("scale", "Scale", StoryPropValue::Choice("linear".into()))
                .options(["linear", "logarithmic"]),
        ]),
    )?;
    registry.register(
        ComponentStory::new(
            "audio-kit.volume-knob",
            "gpui-audio-kit",
            "Volume Knob",
            "Circular volume control with mute state",
        )
        .props([
            StoryProp::new("label", "Label", StoryPropValue::Text("Output".into())),
            StoryProp::new("value", "Value", StoryPropValue::Number(0.72)),
            StoryProp::new("muted", "Muted", StoryPropValue::Bool(false)),
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
            "audio-kit.horizontal-meter",
            "gpui-audio-kit",
            "Horizontal Meter",
            "Tick-aligned horizontal audio meter bar",
        )
        .props([
            StoryProp::new("label", "Label", StoryPropValue::Text("LUFS".into())),
            StoryProp::new("value", "Value", StoryPropValue::Number(-18.0)),
            StoryProp::new("gradient", "Gradient", StoryPropValue::Bool(true)),
            StoryProp::new("kind", "Scale", StoryPropValue::Choice("lufs".into())).options([
                "lufs",
                "stereo_width",
                "peak_spread",
            ]),
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
    )?;
    registry.register(
        ComponentStory::new(
            "audio-kit.spectrum-axis",
            "gpui-audio-kit",
            "Spectrum Axes",
            "Reusable logarithmic frequency and dB axes",
        )
        .props([
            StoryProp::new("min_freq", "Min Hz", StoryPropValue::Number(20.0)),
            StoryProp::new("max_freq", "Max Hz", StoryPropValue::Number(20_000.0)),
        ]),
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
        ThemePreset::new("material3", "Material 3", "material3", false),
        ThemePreset::new("fluent", "Fluent", "fluent", false),
        ThemePreset::new("reduced-motion", "Reduced Motion", "neutral", true),
    ]
}

pub fn default_motion_presets() -> Vec<MotionPreset> {
    vec![
        MotionPreset::new("system", "System", false),
        MotionPreset::new("reduced", "Reduced", true),
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
    use gpui_design_tools::{DesignTokenFormat, export_design_tokens};

    #[test]
    fn builtin_registry_covers_requested_crates() {
        let registry = builtin_story_registry().unwrap();
        assert!(registry.story("ui-kit.button").is_some());
        assert!(registry.story("ui-kit.status").is_some());
        assert!(registry.story("ui-kit.command-palette").is_some());
        assert!(registry.story("ui-kit.accessibility").is_some());
        assert!(registry.story("px.line").is_some());
        assert!(registry.story("px.heatmap").is_some());
        assert!(registry.story("px.treemap").is_some());
        assert!(registry.story("px.surface3d").is_some());
        assert!(registry.story("audio-kit.potentiometer").is_some());
        assert!(registry.story("audio-kit.vertical-slider").is_some());
        assert!(registry.story("audio-kit.volume-knob").is_some());
        assert!(registry.story("audio-kit.horizontal-meter").is_some());
        assert!(registry.story("audio-kit.spectrum-axis").is_some());
    }

    #[test]
    fn builtin_registry_has_renderer_coverage() {
        let registry = builtin_story_registry().unwrap();
        for story in registry.stories() {
            assert!(
                builtin_story_has_renderer(&story.id),
                "missing renderer coverage for {}",
                story.id
            );
        }
    }

    #[test]
    fn px_stories_expose_responsive_fill_prop() {
        let registry = builtin_story_registry().unwrap();
        for story in registry
            .stories()
            .filter(|story| story.crate_name == "gpui-px")
        {
            assert!(
                story.props.iter().any(|prop| prop.name == "fill"),
                "{} must expose the fill/fixed sizing toggle",
                story.id
            );
        }
    }

    #[test]
    fn px_stories_have_responsive_rendered_conformance() {
        let registry = builtin_story_registry().unwrap();
        let px_stories = registry
            .stories()
            .filter(|story| story.crate_name == "gpui-px")
            .collect::<Vec<_>>();
        assert!(px_stories.len() >= 11);
        for story in px_stories {
            assert!(story.conformance.responsive, "{}", story.id);
            assert!(!story.conformance.rendered.allow_scroll, "{}", story.id);
            assert!(
                story.conformance.rendered.min_width <= 390.0,
                "{} must fit mobile width",
                story.id
            );
            assert!(
                story.conformance.rendered.min_height <= 844.0,
                "{} must fit mobile height",
                story.id
            );
        }
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
    fn stories_include_motion_presets() {
        let registry = builtin_story_registry().unwrap();
        let story = registry.story("ui-kit.button").unwrap();
        assert!(story.motions.iter().any(|motion| motion.id == "system"));
        assert!(
            story
                .motions
                .iter()
                .any(|motion| motion.id == "reduced" && motion.reduced_motion)
        );
    }

    #[test]
    fn default_theme_presets_cover_design_languages() {
        let presets = default_theme_presets();
        for language in ["neutral", "apple_hig", "material3", "fluent"] {
            assert!(
                presets.iter().any(|preset| preset.design == language),
                "missing {language} design preset"
            );
        }
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

    fn passing_token_report() -> DesignTokenValidationReport {
        DesignTokenValidationReport {
            passed: true,
            findings: Vec::new(),
            preset_count: 4,
            token_count: 128,
            conformance_markdown: "| preset | motion | tokens | status | findings |\n\
                                   | --- | --- | ---: | --- | --- |\n"
                .to_string(),
        }
    }

    #[test]
    fn component_lab_conformance_passes_builtin_registry() {
        let registry = builtin_story_registry().unwrap();
        let report = validate_component_lab_conformance(&registry, &[], &passing_token_report());
        assert!(report.passed());
        assert!(report.to_markdown().contains("status: pass"));
    }

    #[test]
    fn component_lab_conformance_reports_choice_without_options() {
        let mut registry = StoryRegistry::new();
        registry
            .register(
                ComponentStory::new("test.choice", "test", "Choice", "Choice story").props([
                    StoryProp::new("mode", "Mode", StoryPropValue::Choice("a".into())),
                ]),
            )
            .unwrap();
        let report = validate_component_lab_conformance(&registry, &[], &passing_token_report());
        assert!(!report.passed());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id == "choice.options")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id == "choice.value")
        );
    }

    #[test]
    fn component_lab_conformance_reports_first_party_story_without_renderer() {
        let mut registry = StoryRegistry::new();
        registry
            .register(ComponentStory::new(
                "audio-kit.unrendered",
                "gpui-audio-kit",
                "Unrendered",
                "First-party story without a lab renderer",
            ))
            .unwrap();
        let report = validate_component_lab_conformance(&registry, &[], &passing_token_report());
        assert!(!report.passed());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id == "renderer.coverage")
        );
    }

    #[test]
    fn component_lab_conformance_reports_unknown_theme_design() {
        let mut registry = StoryRegistry::new();
        let mut story = ComponentStory::new("test.theme", "test", "Theme", "Theme story");
        story.themes = vec![ThemePreset::new("unknown", "Unknown", "bogus", false)];
        registry.register(story).unwrap();

        let report = validate_component_lab_conformance(&registry, &[], &passing_token_report());
        assert!(!report.passed());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id == "themes.design")
        );
    }

    #[test]
    fn component_lab_conformance_reports_rendered_overflow() {
        let mut registry = StoryRegistry::new();
        let story = ComponentStory::new("test.rendered", "test", "Rendered", "Rendered story")
            .conformance(StoryConformance::display(900.0, 120.0));
        registry.register(story).unwrap();

        let report = validate_component_lab_conformance(&registry, &[], &passing_token_report());
        assert!(!report.passed());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id == "rendered.overflow")
        );
    }

    #[test]
    fn component_lab_conformance_reports_touch_target_failure() {
        let mut registry = StoryRegistry::new();
        let story = ComponentStory::new("test.touch", "test", "Touch", "Touch story").conformance(
            StoryConformance::interactive(160.0, 120.0, 24.0, ["Action"]),
        );
        registry.register(story).unwrap();

        let report = validate_component_lab_conformance(&registry, &[], &passing_token_report());
        assert!(!report.passed());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id == "touch.target")
        );
    }

    #[test]
    fn component_lab_conformance_reports_focus_metadata_failure() {
        let mut registry = StoryRegistry::new();
        let mut story = ComponentStory::new("test.focus", "test", "Focus", "Focus story");
        story.conformance.focusable_count = 2;
        story.conformance.focus_labels = vec!["Primary".into()];
        registry.register(story).unwrap();

        let report = validate_component_lab_conformance(&registry, &[], &passing_token_report());
        assert!(!report.passed());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id == "focus.labels")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id == "focus.touch_target")
        );
    }

    #[test]
    fn component_lab_conformance_reports_fixed_layout_overflow() {
        let mut registry = StoryRegistry::new();
        let story = ComponentStory::new("test.fixed", "test", "Fixed", "Fixed story");
        registry.register(story.clone()).unwrap();
        let mut doc = StoryDocument::new(story);
        doc.layout = serde_json::json!({
            "viewport": "mobile",
            "constraints": {
                "sizing": "fixed",
                "min_width": 900.0,
                "min_height": 300.0,
                "aspect_ratio": 1.6,
                "padding": 16.0
            }
        });
        let report = validate_component_lab_conformance(&registry, &[doc], &passing_token_report());
        assert!(!report.passed());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id == "layout.overflow")
        );
    }

    #[test]
    fn component_lab_conformance_reports_bad_builder_layout() {
        let mut registry = StoryRegistry::new();
        let story = ComponentStory::new("test.builder", "test", "Builder", "Builder story");
        registry.register(story.clone()).unwrap();
        let mut doc = StoryDocument::new(story);
        doc.layout = serde_json::json!({
            "builder": {
                "horizontal_align": "middle",
                "vertical_align": "center",
                "overflow": "clip",
                "surface": "paper",
                "gap": 100.0,
                "border": "yes"
            }
        });

        let report = validate_component_lab_conformance(&registry, &[doc], &passing_token_report());
        assert!(!report.passed());
        for id in [
            "builder.horizontal_align",
            "builder.overflow",
            "builder.surface",
            "builder.gap",
            "builder.border",
        ] {
            assert!(
                report.findings.iter().any(|finding| finding.id == id),
                "missing {id}"
            );
        }
    }

    #[test]
    fn live_preview_reload_loads_story_documents_and_tokens() {
        let tmp = tempfile::tempdir().unwrap();
        let stories_dir = tmp.path().join("stories");
        std::fs::create_dir_all(&stories_dir).unwrap();

        let story = builtin_story_registry()
            .unwrap()
            .story("ui-kit.button")
            .unwrap()
            .clone();
        StoryDocument::new(story)
            .save_story_json(&stories_dir.join("button.story.json"))
            .unwrap();

        let token_path = tmp.path().join("tokens.json");
        std::fs::write(
            &token_path,
            export_design_tokens(DesignTokenFormat::StyleDictionaryJson).unwrap(),
        )
        .unwrap();

        let reload =
            reload_live_preview_state(&stories_dir, &[token_path.clone()], SystemTime::UNIX_EPOCH)
                .unwrap()
                .expect("first load should see files");
        assert_eq!(reload.story_documents.len(), 1);
        assert_eq!(reload.token_reports.len(), 1);
        assert!(reload.token_reports[0].report.passed);

        let unchanged =
            reload_live_preview_state(&stories_dir, &[token_path], reload.latest_modified).unwrap();
        assert!(unchanged.is_none());
    }

    #[test]
    fn latest_rust_source_modified_ignores_target_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let target_dir = tmp.path().join("target");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(target_dir.join("generated.rs"), "fn generated() {}").unwrap();

        assert_eq!(
            latest_rust_source_modified(tmp.path()).unwrap(),
            SystemTime::UNIX_EPOCH
        );

        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("lib.rs"), "pub fn real_source() {}").unwrap();

        assert!(latest_rust_source_modified(tmp.path()).unwrap() > SystemTime::UNIX_EPOCH);
    }
}

//! Interactive GPUI component lab.

use crate::{
    ComponentStory, LivePreviewReload, LivePreviewTokenReload, MotionPreset,
    ResponsivePreviewMatrix, StoryDocument, StoryProp, StoryPropValue, StoryRegistry,
    StoryRendererRegistry, ThemePreset, UI_KIT_EXPORTED_COMPONENT_STORY_IDS, ViewportPreset,
    builtin_story_registry, builtin_story_renderers, latest_story_or_token_modified,
    load_story_documents, reload_live_preview_state,
};
use anyhow::{Context as AnyhowContext, Result};
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Div, Entity, IntoElement, Render, SharedString, Stateful,
    StatefulInteractiveElement, WeakEntity, Window, div, px, relative,
};
use gpui_audio_kit::{
    AudioScale, HorizontalMeterTheme, LevelMeterElement, Potentiometer, PotentiometerSize,
    SpectrumAxisTheme, SpectrumElement, TickConfig, VerticalSlider, VerticalSliderSize, VolumeKnob,
    render_horizontal_meter_bar, render_spectrum_db_axis, render_spectrum_frequency_axis,
    render_tick_row,
};
use gpui_miniapp::{MiniApp, MiniAppConfig};
use gpui_px::{
    ColorScale, Colormap, LegendPosition, ScaleType, StrokeDashArray, TilingMethod, TreemapNode,
    area, bar, boxplot, contour, donut, heatmap, isoline, line, pie, scatter, surface3d, treemap,
};
use gpui_ui_kit::qr::AnimatedQrCode;
use gpui_ui_kit::showcase::{Showcase, ShowcaseSection};
use gpui_ui_kit::theme::ThemeExt;
use gpui_ui_kit::{
    Accordion, AccordionItem, Alert, AlertVariant, Avatar, AvatarGroup, AvatarShape, AvatarSize,
    AvatarStatus, Badge, BadgeDot, BadgeSize, BadgeVariant, BreadcrumbItem, Breadcrumbs, Button,
    ButtonSet, ButtonSetOption, ButtonSize, ButtonVariant, Card, Checkbox, CheckboxSize,
    CircularProgress, Code, Color, ColorPickerView, Column, CommandItem, CommandPalette,
    ConfirmDialog, ConfirmDialogVariant, ContextMenu, DesignSystem, Dialog, DialogSize, Divider,
    DragItem, DragList, EmptyState, FocusDirection, FocusGroup, HStack, Heading, IconButton,
    IconButtonSize, IconButtonVariant, ImageView, InlineAlert, Input, InputSize,
    KeyboardShortcutLabel, KeyboardShortcutSize, Link, LoadingDots, LoadingOverlay, Menu, MenuBar,
    MenuBarItem, MenuItem, Notification, NotificationVariant, NumberInput, NumberInputSize,
    PaneDivider, Popover, Port, PortDirection, Position, Progress, ProgressSize, ProgressVariant,
    QrCode, SearchBar, SearchBarSize, Select, SelectOption, SelectSize, SettingsForm, SettingsRow,
    Sidebar, Slider, Spacer, Spinner, SpinnerSize, SplitDirection, SplitPane, StatusBar,
    StepIndicator, StepIndicatorSize, StepItem, StepItemStatus, StepOrientation, StepStatus,
    TabItem, TabVariant, Table, Tabs, Tag, TagVariant, Text, TextSize, TextWeight, Toast,
    ToastContainer, ToastPosition, ToastVariant, Toggle, ToggleSize, ToggleStyle, Toolbar,
    ToolbarItem, Tooltip, TreeNode, TreeView, VStack, WithTooltip, Wizard, WizardHeader,
    WizardNavigation, WizardStep, WizardVariant, WorkflowCanvas, WorkflowGraph, WorkflowNode,
    WorkflowNodeData,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Configuration for the interactive component lab app.
#[derive(Debug, Clone)]
pub struct LabAppConfig {
    pub stories_dir: PathBuf,
    pub token_paths: Vec<PathBuf>,
    pub watch: bool,
}

impl LabAppConfig {
    pub fn new(stories_dir: PathBuf, token_paths: Vec<PathBuf>) -> Self {
        Self {
            stories_dir,
            token_paths,
            watch: false,
        }
    }

    pub fn with_watch(mut self, watch: bool) -> Self {
        self.watch = watch;
        self
    }
}

impl Default for LabAppConfig {
    fn default() -> Self {
        Self {
            stories_dir: PathBuf::from("crates/gpui-toolkit/stories"),
            token_paths: Vec::new(),
            watch: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewSizing {
    Fill,
    Fit,
    Fixed,
}

impl PreviewSizing {
    const ALL: [Self; 3] = [Self::Fill, Self::Fit, Self::Fixed];

    fn as_str(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::Fit => "fit",
            Self::Fixed => "fixed",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Fill => "Fill",
            Self::Fit => "Fit",
            Self::Fixed => "Fixed",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "fit" => Self::Fit,
            "fixed" => Self::Fixed,
            _ => Self::Fill,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewAlign {
    Start,
    Center,
    End,
    Stretch,
}

impl PreviewAlign {
    const ALL: [Self; 4] = [Self::Start, Self::Center, Self::End, Self::Stretch];

    fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::Stretch => "stretch",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Center => "Center",
            Self::End => "End",
            Self::Stretch => "Stretch",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "start" => Self::Start,
            "end" => Self::End,
            "stretch" => Self::Stretch,
            _ => Self::Center,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewOverflow {
    Hidden,
    Scroll,
    Visible,
}

impl PreviewOverflow {
    const ALL: [Self; 3] = [Self::Hidden, Self::Scroll, Self::Visible];

    fn as_str(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Scroll => "scroll",
            Self::Visible => "visible",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Hidden => "Hidden",
            Self::Scroll => "Scroll",
            Self::Visible => "Visible",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "scroll" => Self::Scroll,
            "visible" => Self::Visible,
            _ => Self::Hidden,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewSurface {
    Background,
    Surface,
    Transparent,
}

impl PreviewSurface {
    const ALL: [Self; 3] = [Self::Background, Self::Surface, Self::Transparent];

    fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Surface => "surface",
            Self::Transparent => "transparent",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Background => "Background",
            Self::Surface => "Surface",
            Self::Transparent => "Transparent",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "surface" => Self::Surface,
            "transparent" => Self::Transparent,
            _ => Self::Background,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreviewLayoutConstraints {
    sizing: PreviewSizing,
    min_width: f32,
    min_height: f32,
    aspect_ratio: f32,
    padding: f32,
    horizontal_align: PreviewAlign,
    vertical_align: PreviewAlign,
    overflow: PreviewOverflow,
    surface: PreviewSurface,
    gap: f32,
    border: bool,
}

impl Default for PreviewLayoutConstraints {
    fn default() -> Self {
        Self {
            sizing: PreviewSizing::Fill,
            min_width: 560.0,
            min_height: 340.0,
            aspect_ratio: 1.6,
            padding: 24.0,
            horizontal_align: PreviewAlign::Center,
            vertical_align: PreviewAlign::Center,
            overflow: PreviewOverflow::Hidden,
            surface: PreviewSurface::Background,
            gap: 0.0,
            border: true,
        }
    }
}

impl PreviewLayoutConstraints {
    fn from_layout(layout: &Value) -> Self {
        let mut constraints = Self::default();
        let Some(raw) = layout.get("constraints") else {
            return constraints;
        };

        if let Some(sizing) = raw.get("sizing").and_then(Value::as_str) {
            constraints.sizing = PreviewSizing::parse(sizing);
        }
        if let Some(min_width) = raw.get("min_width").and_then(Value::as_f64) {
            constraints.min_width = clamp_f32(min_width, 160.0, 1600.0);
        }
        if let Some(min_height) = raw.get("min_height").and_then(Value::as_f64) {
            constraints.min_height = clamp_f32(min_height, 120.0, 1200.0);
        }
        if let Some(aspect_ratio) = raw.get("aspect_ratio").and_then(Value::as_f64) {
            constraints.aspect_ratio = clamp_f32(aspect_ratio, 0.5, 3.0);
        }
        if let Some(padding) = raw.get("padding").and_then(Value::as_f64) {
            constraints.padding = clamp_f32(padding, 0.0, 80.0);
        }

        if let Some(builder) = layout.get("builder").and_then(Value::as_object) {
            if let Some(align) = builder.get("horizontal_align").and_then(Value::as_str) {
                constraints.horizontal_align = PreviewAlign::parse(align);
            }
            if let Some(align) = builder.get("vertical_align").and_then(Value::as_str) {
                constraints.vertical_align = PreviewAlign::parse(align);
            }
            if let Some(overflow) = builder.get("overflow").and_then(Value::as_str) {
                constraints.overflow = PreviewOverflow::parse(overflow);
            }
            if let Some(surface) = builder.get("surface").and_then(Value::as_str) {
                constraints.surface = PreviewSurface::parse(surface);
            }
            if let Some(gap) = builder.get("gap").and_then(Value::as_f64) {
                constraints.gap = clamp_f32(gap, 0.0, 80.0);
            }
            if let Some(border) = builder.get("border").and_then(Value::as_bool) {
                constraints.border = border;
            }
        }

        constraints
    }

    fn as_json(self) -> Value {
        json!({
            "sizing": self.sizing.as_str(),
            "min_width": self.min_width,
            "min_height": self.min_height,
            "aspect_ratio": self.aspect_ratio,
            "padding": self.padding,
        })
    }

    fn builder_json(self) -> Value {
        json!({
            "horizontal_align": self.horizontal_align.as_str(),
            "vertical_align": self.vertical_align.as_str(),
            "overflow": self.overflow.as_str(),
            "surface": self.surface.as_str(),
            "gap": self.gap,
            "border": self.border,
        })
    }

    fn frame_dimensions(self, viewport: &ViewportPreset) -> (f32, f32) {
        let viewport_width = viewport.width.min(980.0).max(240.0);
        let viewport_height = viewport.height.min(620.0).max(180.0);
        match self.sizing {
            PreviewSizing::Fixed => (self.min_width, self.min_height),
            PreviewSizing::Fit => {
                let width = self.min_width.min(viewport_width);
                let height = if self.aspect_ratio > 0.0 {
                    (width / self.aspect_ratio).max(self.min_height.min(viewport_height))
                } else {
                    self.min_height.min(viewport_height)
                };
                (width, height.min(viewport_height))
            }
            PreviewSizing::Fill => {
                let width = viewport_width.max(self.min_width).min(980.0);
                let height = if self.aspect_ratio > 0.0 {
                    (width / self.aspect_ratio).max(self.min_height)
                } else {
                    viewport_height.max(self.min_height)
                };
                (width, height.min(620.0).max(self.min_height.min(620.0)))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct InitialLabState {
    viewport_id: String,
    theme_id: String,
    motion_id: String,
    matrix_mode: bool,
    layout_constraints: PreviewLayoutConstraints,
}

impl InitialLabState {
    fn from_document(document: &StoryDocument) -> Self {
        let story = &document.story;
        Self {
            viewport_id: layout_string(&document.layout, "viewport")
                .filter(|id| {
                    story
                        .viewports
                        .iter()
                        .any(|viewport| viewport.id.as_str() == id.as_str())
                })
                .unwrap_or_else(|| first_viewport_id(story)),
            theme_id: layout_string(&document.layout, "theme")
                .filter(|id| {
                    story
                        .themes
                        .iter()
                        .any(|theme| theme.id.as_str() == id.as_str())
                })
                .unwrap_or_else(|| first_theme_id(story)),
            motion_id: layout_string(&document.layout, "motion")
                .filter(|id| {
                    story
                        .motions
                        .iter()
                        .any(|motion| motion.id.as_str() == id.as_str())
                })
                .unwrap_or_else(|| first_motion_id(story)),
            matrix_mode: document
                .layout
                .get("matrix")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            layout_constraints: PreviewLayoutConstraints::from_layout(&document.layout),
        }
    }
}

/// Launch the interactive GPUI component lab.
pub fn run_lab_app(config: LabAppConfig) -> Result<()> {
    let app_config = MiniAppConfig::new("gpui-component-lab")
        .size(1440.0, 920.0)
        .with_theme(true)
        .scrollable(false);
    MiniApp::run(app_config, move |cx| {
        let config = config.clone();
        cx.new(|cx| ComponentLab::new(config, cx))
    });
    Ok(())
}

/// Interactive storybook/designer view.
pub struct ComponentLab {
    registry: StoryRegistry,
    renderers: StoryRendererRegistry,
    documents: BTreeMap<String, StoryDocument>,
    story_ids: Vec<String>,
    ui_showcases: BTreeMap<String, Entity<Showcase>>,
    selected_story_id: String,
    selected_viewport_id: String,
    selected_theme_id: String,
    selected_motion_id: String,
    matrix_mode: bool,
    layout_constraints: PreviewLayoutConstraints,
    save_status: Option<SharedString>,
    live_status: Option<SharedString>,
    live_preview: bool,
    last_live_modified: SystemTime,
    stories_dir: PathBuf,
    token_paths: Vec<PathBuf>,
    entity: Entity<Self>,
}

impl ComponentLab {
    fn new(config: LabAppConfig, cx: &mut Context<Self>) -> Self {
        let registry = builtin_story_registry().expect("builtin story registry");
        let renderers = builtin_story_renderers().expect("builtin story renderers");
        let mut documents: BTreeMap<String, StoryDocument> = registry
            .stories()
            .cloned()
            .map(|story| (story.id.clone(), StoryDocument::new(story)))
            .collect();

        if let Ok(loaded_docs) = load_story_documents(&config.stories_dir) {
            for doc in loaded_docs {
                documents.insert(doc.story.id.clone(), doc);
            }
        }

        let story_ids: Vec<String> = registry.stories().map(|story| story.id.clone()).collect();
        let ui_showcases = build_ui_showcase_entities(&story_ids, cx);
        let selected_story_id = story_ids.first().cloned().unwrap_or_default();
        let selected_document = documents
            .get(&selected_story_id)
            .expect("selected story exists");
        let initial_state = InitialLabState::from_document(selected_document);

        let last_live_modified =
            latest_story_or_token_modified(&config.stories_dir, &config.token_paths)
                .unwrap_or(SystemTime::UNIX_EPOCH);
        let mut lab = Self {
            registry,
            renderers,
            documents,
            story_ids,
            ui_showcases,
            selected_story_id,
            selected_viewport_id: initial_state.viewport_id,
            selected_theme_id: initial_state.theme_id,
            selected_motion_id: initial_state.motion_id,
            matrix_mode: initial_state.matrix_mode,
            layout_constraints: initial_state.layout_constraints,
            save_status: None,
            live_status: config.watch.then(|| "Live preview enabled".into()),
            live_preview: config.watch,
            last_live_modified,
            stories_dir: config.stories_dir,
            token_paths: config.token_paths,
            entity: cx.entity().clone(),
        };
        if lab.live_preview {
            lab.start_live_preview(cx);
        }
        lab
    }

    fn start_live_preview(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(750))
                    .await;
                let alive = this.update(cx, |lab, cx| {
                    lab.poll_live_preview(cx);
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn poll_live_preview(&mut self, cx: &mut Context<Self>) {
        match reload_live_preview_state(
            &self.stories_dir,
            &self.token_paths,
            self.last_live_modified,
        ) {
            Ok(Some(reload)) => {
                self.apply_live_reload(reload);
                cx.notify();
            }
            Ok(None) => {}
            Err(err) => {
                if let Ok(latest) =
                    latest_story_or_token_modified(&self.stories_dir, &self.token_paths)
                {
                    self.last_live_modified = latest;
                }
                self.live_status = Some(format!("Live reload failed: {err}").into());
                cx.notify();
            }
        }
    }

    fn apply_live_reload(&mut self, reload: LivePreviewReload) {
        let selected_reloaded = reload
            .story_documents
            .iter()
            .any(|doc| doc.story.id == self.selected_story_id);
        let story_count = reload.story_documents.len();

        for doc in reload.story_documents {
            self.documents.insert(doc.story.id.clone(), doc);
        }

        if selected_reloaded && let Some(document) = self.documents.get(&self.selected_story_id) {
            let state = InitialLabState::from_document(document);
            self.selected_viewport_id = state.viewport_id;
            self.selected_theme_id = state.theme_id;
            self.selected_motion_id = state.motion_id;
            self.matrix_mode = state.matrix_mode;
            self.layout_constraints = state.layout_constraints;
        }

        self.last_live_modified = reload.latest_modified;
        self.live_status = Some(live_reload_status(story_count, &reload.token_reports).into());
    }

    fn selected_document(&self) -> &StoryDocument {
        self.documents
            .get(&self.selected_story_id)
            .expect("selected story document")
    }

    fn selected_story(&self) -> &ComponentStory {
        &self.selected_document().story
    }

    fn selected_viewport(&self) -> ViewportPreset {
        self.selected_story()
            .viewports
            .iter()
            .find(|viewport| viewport.id == self.selected_viewport_id)
            .cloned()
            .or_else(|| self.selected_story().viewports.first().cloned())
            .unwrap_or_else(|| ViewportPreset::new("desktop", "Desktop", 1280.0, 800.0))
    }

    fn selected_theme_preset(&self) -> ThemePreset {
        self.selected_story()
            .themes
            .iter()
            .find(|theme| theme.id == self.selected_theme_id)
            .cloned()
            .or_else(|| self.selected_story().themes.first().cloned())
            .unwrap_or_else(|| ThemePreset::new("neutral", "Neutral", "neutral", false))
    }

    fn selected_motion_preset(&self) -> MotionPreset {
        self.selected_story()
            .motions
            .iter()
            .find(|motion| motion.id == self.selected_motion_id)
            .cloned()
            .or_else(|| self.selected_story().motions.first().cloned())
            .unwrap_or_else(|| MotionPreset::new("system", "System", false))
    }

    fn select_story(&mut self, story_id: String) {
        if let Some(document) = self.documents.get(&story_id) {
            let state = InitialLabState::from_document(document);
            self.selected_story_id = story_id;
            self.selected_viewport_id = state.viewport_id;
            self.selected_theme_id = state.theme_id;
            self.selected_motion_id = state.motion_id;
            self.matrix_mode = state.matrix_mode;
            self.layout_constraints = state.layout_constraints;
            self.save_status = None;
        }
    }

    fn set_prop(&mut self, story_id: &str, prop_name: &str, value: StoryPropValue) {
        if let Some(doc) = self.documents.get_mut(story_id) {
            if doc.set_prop_value(prop_name, value).is_ok() {
                self.save_status = Some("Unsaved changes".into());
            }
        }
    }

    fn set_viewport(&mut self, viewport_id: String) {
        self.selected_viewport_id = viewport_id;
        self.sync_layout_state();
    }

    fn set_theme(&mut self, theme_id: String) {
        self.selected_theme_id = theme_id;
        self.sync_layout_state();
    }

    fn set_motion(&mut self, motion_id: String) {
        self.selected_motion_id = motion_id;
        self.sync_layout_state();
    }

    fn set_layout_sizing(&mut self, sizing: PreviewSizing) {
        self.layout_constraints.sizing = sizing;
        self.sync_layout_state();
    }

    fn set_layout_min_width(&mut self, width: f64) {
        self.layout_constraints.min_width = clamp_f32(width, 160.0, 1600.0);
        self.sync_layout_state();
    }

    fn set_layout_min_height(&mut self, height: f64) {
        self.layout_constraints.min_height = clamp_f32(height, 120.0, 1200.0);
        self.sync_layout_state();
    }

    fn set_layout_aspect_ratio(&mut self, aspect_ratio: f64) {
        self.layout_constraints.aspect_ratio = clamp_f32(aspect_ratio, 0.5, 3.0);
        self.sync_layout_state();
    }

    fn set_layout_padding(&mut self, padding: f64) {
        self.layout_constraints.padding = clamp_f32(padding, 0.0, 80.0);
        self.sync_layout_state();
    }

    fn set_layout_horizontal_align(&mut self, align: PreviewAlign) {
        self.layout_constraints.horizontal_align = align;
        self.sync_layout_state();
    }

    fn set_layout_vertical_align(&mut self, align: PreviewAlign) {
        self.layout_constraints.vertical_align = align;
        self.sync_layout_state();
    }

    fn set_layout_overflow(&mut self, overflow: PreviewOverflow) {
        self.layout_constraints.overflow = overflow;
        self.sync_layout_state();
    }

    fn set_layout_surface(&mut self, surface: PreviewSurface) {
        self.layout_constraints.surface = surface;
        self.sync_layout_state();
    }

    fn set_layout_gap(&mut self, gap: f64) {
        self.layout_constraints.gap = clamp_f32(gap, 0.0, 80.0);
        self.sync_layout_state();
    }

    fn set_layout_border(&mut self, border: bool) {
        self.layout_constraints.border = border;
        self.sync_layout_state();
    }

    fn toggle_matrix(&mut self) {
        self.matrix_mode = !self.matrix_mode;
        self.sync_layout_state();
    }

    fn sync_layout_state(&mut self) {
        if let Some(doc) = self.documents.get_mut(&self.selected_story_id) {
            doc.layout = json!({
                "viewport": self.selected_viewport_id,
                "theme": self.selected_theme_id,
                "motion": self.selected_motion_id,
                "matrix": self.matrix_mode,
                "constraints": self.layout_constraints.as_json(),
                "builder": self.layout_constraints.builder_json(),
            });
        }
        self.save_status = Some("Unsaved changes".into());
    }

    fn save_selected(&mut self) {
        self.sync_layout_state();
        let result = self.try_save_selected();
        self.save_status = Some(match result {
            Ok(path) => format!("Saved {}", path.display()).into(),
            Err(err) => format!("Save failed: {err}").into(),
        });
    }

    fn try_save_selected(&self) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.stories_dir)
            .with_context(|| format!("create {}", self.stories_dir.display()))?;
        let path = self
            .stories_dir
            .join(story_file_name(&self.selected_story_id));
        self.selected_document().save_story_json(&path)?;
        Ok(path)
    }

    fn reload_documents(&mut self) {
        match load_story_documents(&self.stories_dir) {
            Ok(docs) => {
                for doc in docs {
                    self.documents.insert(doc.story.id.clone(), doc);
                }
                self.save_status = Some("Reloaded story JSON".into());
            }
            Err(err) => {
                self.save_status = Some(format!("Reload failed: {err}").into());
            }
        }
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let mut list = div().flex().flex_col().gap_1();

        for story_id in &self.story_ids {
            let Some(story) = self.documents.get(story_id).map(|doc| &doc.story) else {
                continue;
            };
            let selected = *story_id == self.selected_story_id;
            let story_id_for_click = story_id.clone();
            let label = format!("{} / {}", story.crate_name, story.title);
            let entity = self.entity.clone();
            list = list.child(
                Button::new(lab_id(&["story", story_id]), label)
                    .variant(if selected {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .size(ButtonSize::Sm)
                    .full_width(true)
                    .on_click(move |_window, cx| {
                        entity.update(cx, |this, _| this.select_story(story_id_for_click.clone()));
                    }),
            );
        }

        div()
            .w(px(300.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Heading::h3("Component Lab"))
                    .child(Text::new(format!("{} stories", self.registry.len())).muted(true)),
            )
            .child(list)
            .child(self.render_token_status(cx))
            .into_any_element()
    }

    fn render_token_status(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let token_label = if self.token_paths.is_empty() {
            "No token JSON watched".to_string()
        } else {
            format!("Watching {} token file(s)", self.token_paths.len())
        };
        let live_label = if self.live_preview {
            "Live preview on"
        } else {
            "Live preview off"
        };
        div()
            .mt_auto()
            .p_3()
            .rounded_md()
            .bg(theme.surface_hover)
            .border_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap_1()
            .child(
                Text::new(live_label)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child(Text::new(token_label).size(TextSize::Xs).muted(true))
            .when_some(self.live_status.clone(), |el, status| {
                el.child(
                    Text::new(status)
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                )
            })
            .into_any_element()
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let entity = self.entity.clone();
        let story = self.selected_story();
        let viewport = self.selected_viewport();
        let theme_preset = self.selected_theme_preset();

        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .pb_4()
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Heading::h2(story.title.clone()))
                    .child(
                        Text::new(format!(
                            "{} | {} x {} | {}",
                            story.crate_name,
                            viewport.width.round(),
                            viewport.height.round(),
                            theme_preset.label
                        ))
                        .size(TextSize::Sm)
                        .color(theme.text_secondary),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new(
                            "lab-toggle-matrix",
                            if self.matrix_mode {
                                "Preview"
                            } else {
                                "Matrix"
                            },
                        )
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Sm)
                        .on_click(move |_window, cx| {
                            entity.update(cx, |this, _| this.toggle_matrix());
                        }),
                    )
                    .child({
                        let entity = self.entity.clone();
                        Button::new("lab-reload", "Reload")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .on_click(move |_window, cx| {
                                entity.update(cx, |this, _| this.reload_documents());
                            })
                    })
                    .child({
                        let entity = self.entity.clone();
                        Button::new("lab-save", "Save")
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Sm)
                            .on_click(move |_window, cx| {
                                entity.update(cx, |this, _| this.save_selected());
                            })
                    }),
            )
            .into_any_element()
    }

    fn render_controls_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let story = self.selected_story();

        let mut props = div().flex().flex_col().gap_3();
        for prop in &story.props {
            props = props.child(self.render_prop_editor(story, prop, cx));
        }

        div()
            .w(px(340.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_5()
            .p_4()
            .bg(theme.surface)
            .border_l_1()
            .border_color(theme.border)
            .child(self.render_story_metadata(story, cx))
            .child(self.render_story_renderer(story, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Heading::h3("Props"))
                    .child(
                        Text::new(story.description.clone())
                            .size(TextSize::Sm)
                            .muted(true),
                    ),
            )
            .child(props)
            .child(self.render_layout_controls(cx))
            .when_some(self.save_status.clone(), |el, status| {
                el.child(
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(theme.surface_hover)
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            Text::new(status)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_story_renderer(&self, story: &ComponentStory, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let mut rows = div().flex().flex_col().gap_2();

        if let Some(renderer) = self.renderers.renderer(&story.id) {
            for (label, value) in [
                ("Kind", renderer.kind.label().to_string()),
                ("Interactive", renderer.interactive.to_string()),
                ("Matrix", renderer.matrix_preview.to_string()),
            ] {
                rows = rows.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(Text::new(label).size(TextSize::Xs).color(theme.text_muted))
                        .child(
                            Text::new(value)
                                .size(TextSize::Xs)
                                .weight(TextWeight::Medium)
                                .color(theme.text_secondary),
                        ),
                );
            }
        } else {
            rows = rows.child(
                Text::new("No interactive renderer registered")
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            );
        }

        div()
            .p_3()
            .rounded_md()
            .bg(theme.surface_hover)
            .border_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap_2()
            .child(Heading::h3("Renderer"))
            .child(rows)
            .into_any_element()
    }

    fn render_story_metadata(&self, story: &ComponentStory, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let mut rows = div().flex().flex_col().gap_2();
        for item in &story.metadata {
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        Text::new(item.label.clone())
                            .size(TextSize::Xs)
                            .color(theme.text_muted),
                    )
                    .child(
                        Text::new(item.value.clone())
                            .size(TextSize::Xs)
                            .weight(TextWeight::Medium)
                            .color(theme.text_secondary),
                    ),
            );
        }

        div()
            .p_3()
            .rounded_md()
            .bg(theme.surface_hover)
            .border_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap_2()
            .child(Heading::h3("Metadata"))
            .child(rows)
            .into_any_element()
    }

    fn render_prop_editor(
        &self,
        story: &ComponentStory,
        prop: &StoryProp,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let story_id = story.id.clone();
        let prop_name = prop.name.clone();
        let entity = self.entity.clone();

        let control = match &prop.value {
            StoryPropValue::Bool(value) => {
                Toggle::new(lab_id(&["prop-bool", &story.id, &prop.name]))
                    .checked(*value)
                    .label(if *value { "On" } else { "Off" })
                    .size(ToggleSize::Sm)
                    .style(ToggleStyle::Sliding)
                    .on_change(move |checked, _window, cx| {
                        entity.update(cx, |this, _| {
                            this.set_prop(&story_id, &prop_name, StoryPropValue::Bool(checked));
                        });
                    })
                    .into_any_element()
            }
            StoryPropValue::Number(value) => {
                NumberInput::new(lab_id(&["prop-number", &story.id, &prop.name]))
                    .value(*value)
                    .step(number_step(&prop.name))
                    .decimals(2)
                    .width(150.0)
                    .size(NumberInputSize::Sm)
                    .on_change(move |number, _window, cx| {
                        entity.update(cx, |this, _| {
                            this.set_prop(&story_id, &prop_name, StoryPropValue::Number(number));
                        });
                    })
                    .into_any_element()
            }
            StoryPropValue::Text(value) | StoryPropValue::Color(value) => {
                let current_value = value.clone();
                let is_color = matches!(prop.value, StoryPropValue::Color(_));
                Input::new(lab_id(&["prop-text", &story.id, &prop.name]))
                    .value(current_value)
                    .size(InputSize::Sm)
                    .placeholder(prop.label.clone())
                    .on_text_change(move |text, _window, cx| {
                        entity.update(cx, |this, _| {
                            let value = if is_color {
                                StoryPropValue::Color(text)
                            } else {
                                StoryPropValue::Text(text)
                            };
                            this.set_prop(&story_id, &prop_name, value);
                        });
                    })
                    .into_any_element()
            }
            StoryPropValue::Choice(value) => {
                let mut row = div().flex().flex_wrap().gap_1();
                for option in &prop.options {
                    let option_for_click = option.clone();
                    let story_id = story.id.clone();
                    let prop_name = prop.name.clone();
                    let entity = self.entity.clone();
                    row = row.child(
                        Button::new(
                            lab_id(&["prop-choice", &story.id, &prop.name, option]),
                            option.clone(),
                        )
                        .variant(if option == value {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Ghost
                        })
                        .size(ButtonSize::Xs)
                        .on_click(move |_window, cx| {
                            entity.update(cx, |this, _| {
                                this.set_prop(
                                    &story_id,
                                    &prop_name,
                                    StoryPropValue::Choice(option_for_click.clone()),
                                );
                            });
                        }),
                    );
                }
                row.into_any_element()
            }
        };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        Text::new(prop.label.clone())
                            .size(TextSize::Sm)
                            .weight(TextWeight::Medium),
                    )
                    .child(
                        Text::new(prop_value_label(&prop.value))
                            .size(TextSize::Xs)
                            .color(theme.text_muted),
                    ),
            )
            .child(control)
            .into_any_element()
    }

    fn render_layout_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let story = self.selected_story();

        let mut viewport_row = div().flex().flex_wrap().gap_1();
        for viewport in &story.viewports {
            let viewport_id = viewport.id.clone();
            let entity = self.entity.clone();
            viewport_row = viewport_row.child(
                Button::new(lab_id(&["viewport", &viewport.id]), viewport.label.clone())
                    .variant(if viewport.id == self.selected_viewport_id {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .size(ButtonSize::Xs)
                    .on_click(move |_window, cx| {
                        entity.update(cx, |this, _| this.set_viewport(viewport_id.clone()));
                    }),
            );
        }

        let mut theme_row = div().flex().flex_wrap().gap_1();
        for theme_preset in &story.themes {
            let theme_id = theme_preset.id.clone();
            let entity = self.entity.clone();
            theme_row = theme_row.child(
                Button::new(
                    lab_id(&["theme", &theme_preset.id]),
                    theme_preset.label.clone(),
                )
                .variant(if theme_preset.id == self.selected_theme_id {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                })
                .size(ButtonSize::Xs)
                .on_click(move |_window, cx| {
                    entity.update(cx, |this, _| this.set_theme(theme_id.clone()));
                }),
            );
        }

        let mut motion_row = div().flex().flex_wrap().gap_1();
        for motion in &story.motions {
            let motion_id = motion.id.clone();
            let entity = self.entity.clone();
            motion_row = motion_row.child(
                Button::new(lab_id(&["motion", &motion.id]), motion.label.clone())
                    .variant(if motion.id == self.selected_motion_id {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .size(ButtonSize::Xs)
                    .on_click(move |_window, cx| {
                        entity.update(cx, |this, _| this.set_motion(motion_id.clone()));
                    }),
            );
        }

        let mut sizing_row = div().flex().flex_wrap().gap_1();
        for sizing in PreviewSizing::ALL {
            let entity = self.entity.clone();
            sizing_row = sizing_row.child(
                Button::new(lab_id(&["layout-sizing", sizing.as_str()]), sizing.label())
                    .variant(if sizing == self.layout_constraints.sizing {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .size(ButtonSize::Xs)
                    .on_click(move |_window, cx| {
                        entity.update(cx, |this, _| this.set_layout_sizing(sizing));
                    }),
            );
        }

        let mut horizontal_align_row = div().flex().flex_wrap().gap_1();
        for align in PreviewAlign::ALL {
            let entity = self.entity.clone();
            horizontal_align_row = horizontal_align_row.child(
                Button::new(lab_id(&["layout-h-align", align.as_str()]), align.label())
                    .variant(if align == self.layout_constraints.horizontal_align {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .size(ButtonSize::Xs)
                    .on_click(move |_window, cx| {
                        entity.update(cx, |this, _| this.set_layout_horizontal_align(align));
                    }),
            );
        }

        let mut vertical_align_row = div().flex().flex_wrap().gap_1();
        for align in PreviewAlign::ALL {
            let entity = self.entity.clone();
            vertical_align_row = vertical_align_row.child(
                Button::new(lab_id(&["layout-v-align", align.as_str()]), align.label())
                    .variant(if align == self.layout_constraints.vertical_align {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .size(ButtonSize::Xs)
                    .on_click(move |_window, cx| {
                        entity.update(cx, |this, _| this.set_layout_vertical_align(align));
                    }),
            );
        }

        let mut overflow_row = div().flex().flex_wrap().gap_1();
        for overflow in PreviewOverflow::ALL {
            let entity = self.entity.clone();
            overflow_row = overflow_row.child(
                Button::new(
                    lab_id(&["layout-overflow", overflow.as_str()]),
                    overflow.label(),
                )
                .variant(if overflow == self.layout_constraints.overflow {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                })
                .size(ButtonSize::Xs)
                .on_click(move |_window, cx| {
                    entity.update(cx, |this, _| this.set_layout_overflow(overflow));
                }),
            );
        }

        let mut surface_row = div().flex().flex_wrap().gap_1();
        for surface in PreviewSurface::ALL {
            let entity = self.entity.clone();
            surface_row = surface_row.child(
                Button::new(
                    lab_id(&["layout-surface", surface.as_str()]),
                    surface.label(),
                )
                .variant(if surface == self.layout_constraints.surface {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                })
                .size(ButtonSize::Xs)
                .on_click(move |_window, cx| {
                    entity.update(cx, |this, _| this.set_layout_surface(surface));
                }),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_3()
            .pt_4()
            .border_t_1()
            .border_color(theme.border)
            .child(Heading::h3("Layout"))
            .child(
                Text::new("Viewport")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Medium),
            )
            .child(viewport_row)
            .child(
                Text::new("Design")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Medium),
            )
            .child(theme_row)
            .child(
                Text::new("Motion")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Medium),
            )
            .child(motion_row)
            .child(
                Text::new("Sizing")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Medium),
            )
            .child(sizing_row)
            .child(
                Text::new("Horizontal Align")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Medium),
            )
            .child(horizontal_align_row)
            .child(
                Text::new("Vertical Align")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Medium),
            )
            .child(vertical_align_row)
            .child(
                Text::new("Overflow")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Medium),
            )
            .child(overflow_row)
            .child(
                Text::new("Surface")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Medium),
            )
            .child(surface_row)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        NumberInput::new("layout-min-width")
                            .label("Min W")
                            .value(self.layout_constraints.min_width as f64)
                            .range(160.0, 1600.0)
                            .step(20.0)
                            .decimals(0)
                            .unit("px")
                            .width(104.0)
                            .size(NumberInputSize::Sm)
                            .on_change({
                                let entity = self.entity.clone();
                                move |value, _window, cx| {
                                    entity.update(cx, |this, _| this.set_layout_min_width(value));
                                }
                            }),
                    )
                    .child(
                        NumberInput::new("layout-min-height")
                            .label("Min H")
                            .value(self.layout_constraints.min_height as f64)
                            .range(120.0, 1200.0)
                            .step(20.0)
                            .decimals(0)
                            .unit("px")
                            .width(104.0)
                            .size(NumberInputSize::Sm)
                            .on_change({
                                let entity = self.entity.clone();
                                move |value, _window, cx| {
                                    entity.update(cx, |this, _| this.set_layout_min_height(value));
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        NumberInput::new("layout-aspect-ratio")
                            .label("Ratio")
                            .value(self.layout_constraints.aspect_ratio as f64)
                            .range(0.5, 3.0)
                            .step(0.1)
                            .decimals(2)
                            .width(104.0)
                            .size(NumberInputSize::Sm)
                            .on_change({
                                let entity = self.entity.clone();
                                move |value, _window, cx| {
                                    entity
                                        .update(cx, |this, _| this.set_layout_aspect_ratio(value));
                                }
                            }),
                    )
                    .child(
                        NumberInput::new("layout-padding")
                            .label("Padding")
                            .value(self.layout_constraints.padding as f64)
                            .range(0.0, 80.0)
                            .step(4.0)
                            .decimals(0)
                            .unit("px")
                            .width(104.0)
                            .size(NumberInputSize::Sm)
                            .on_change({
                                let entity = self.entity.clone();
                                move |value, _window, cx| {
                                    entity.update(cx, |this, _| this.set_layout_padding(value));
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        NumberInput::new("layout-gap")
                            .label("Gap")
                            .value(self.layout_constraints.gap as f64)
                            .range(0.0, 80.0)
                            .step(4.0)
                            .decimals(0)
                            .unit("px")
                            .width(104.0)
                            .size(NumberInputSize::Sm)
                            .on_change({
                                let entity = self.entity.clone();
                                move |value, _window, cx| {
                                    entity.update(cx, |this, _| this.set_layout_gap(value));
                                }
                            }),
                    )
                    .child(
                        Toggle::new("layout-border")
                            .checked(self.layout_constraints.border)
                            .label("Border")
                            .size(ToggleSize::Sm)
                            .on_change({
                                let entity = self.entity.clone();
                                move |checked, _window, cx| {
                                    entity.update(cx, |this, _| this.set_layout_border(checked));
                                }
                            }),
                    ),
            )
            .child(
                Toggle::new("matrix-mode")
                    .checked(self.matrix_mode)
                    .label("Responsive matrix")
                    .size(ToggleSize::Sm)
                    .on_change({
                        let entity = self.entity.clone();
                        move |_checked, _window, cx| {
                            entity.update(cx, |this, _| this.toggle_matrix());
                        }
                    }),
            )
            .into_any_element()
    }

    fn render_preview_area(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.matrix_mode {
            self.render_matrix(cx)
        } else {
            self.render_single_preview(cx)
        }
    }

    fn render_single_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let story = self.selected_story();
        let viewport = self.selected_viewport();
        let theme_preset = self.selected_theme_preset();
        let motion_preset = self.selected_motion_preset();
        let preview_design = design_for_theme_preset(&theme_preset);
        let constraints = self.layout_constraints;
        let (frame_width, frame_height) = constraints.frame_dimensions(&viewport);
        let story_id = story.id.clone();

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .child(
                div()
                    .w(px(frame_width))
                    .h(px(frame_height))
                    .max_w_full()
                    .max_h_full()
                    .flex()
                    .flex_col()
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .bg(theme.surface)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                Text::new(format!("{} preview", viewport.label)).size(TextSize::Xs),
                            )
                            .child(
                                Text::new(format!(
                                    "{} / {} / {}",
                                    theme_preset.label,
                                    motion_preset.label,
                                    constraints.sizing.label()
                                ))
                                .size(TextSize::Xs)
                                .muted(true),
                            ),
                    )
                    .child(
                        apply_preview_builder_style(
                            div()
                                .id("lab-preview-builder-surface")
                                .flex_1()
                                .flex()
                                .gap(px(constraints.gap)),
                            constraints,
                            theme,
                        )
                        .p(px(constraints.padding))
                        .child(self.render_story_preview(
                            story,
                            &story_id,
                            true,
                            preview_design,
                            cx,
                        )),
                    ),
            )
            .into_any_element()
    }

    fn render_matrix(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let story = self.selected_story();
        let motion_preset = self.selected_motion_preset();
        let matrix = ResponsivePreviewMatrix::for_story(story);

        let mut grid = div().flex().flex_wrap().gap_3().items_start();
        for (index, cell) in matrix.cells.iter().enumerate() {
            let scope = format!("matrix-{index}");
            let preview_design = design_for_theme_preset(&cell.theme);
            grid = grid.child(
                div()
                    .w(px(260.0))
                    .h(px(210.0))
                    .flex()
                    .flex_col()
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .overflow_hidden()
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .bg(theme.surface)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                Text::new(format!(
                                    "{} / {} x {}",
                                    cell.viewport.label,
                                    cell.viewport.width.round(),
                                    cell.viewport.height.round()
                                ))
                                .size(TextSize::Xs),
                            )
                            .child(
                                Text::new(cell.theme.label.clone())
                                    .size(TextSize::Xs)
                                    .muted(true),
                            )
                            .child(
                                Text::new(motion_preset.label.clone())
                                    .size(TextSize::Xs)
                                    .muted(true),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .p_3()
                            .child(self.render_story_preview(
                                story,
                                &scope,
                                false,
                                preview_design,
                                cx,
                            )),
                    ),
            );
        }

        div().size_full().p_6().child(grid).into_any_element()
    }

    fn render_story_preview(
        &self,
        story: &ComponentStory,
        scope: &str,
        interactive: bool,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match story.id.as_str() {
            "ui-kit.button" => self.render_button_story(story, scope, interactive, design, cx),
            "ui-kit.form" => self.render_form_story(story, scope, interactive, design, cx),
            "ui-kit.status" => self.render_status_story(story, scope, design, cx),
            "ui-kit.navigation" => self.render_navigation_story(story, scope, design, cx),
            "ui-kit.feedback" => self.render_feedback_story(story, scope, design, cx),
            "ui-kit.card" => self.render_card_story(story, scope, design, cx),
            story_id if ui_kit_exported_component_story_id(story_id) => {
                self.render_exported_ui_kit_component_story(story, scope, interactive, design, cx)
            }
            story_id if self.ui_showcases.contains_key(story_id) => {
                self.render_ui_kit_showcase_story(story, scope, cx)
            }
            "audio-kit.potentiometer" => {
                self.render_potentiometer_story(story, scope, interactive, design, cx)
            }
            "audio-kit.vertical-slider" => {
                self.render_vertical_slider_story(story, scope, interactive, design, cx)
            }
            "audio-kit.volume-knob" => {
                self.render_volume_knob_story(story, scope, interactive, design, cx)
            }
            "audio-kit.meter" => self.render_meter_story(story, scope, design, cx),
            "audio-kit.horizontal-meter" => {
                self.render_horizontal_meter_story(story, scope, design, cx)
            }
            "audio-kit.spectrum" => self.render_spectrum_story(story, scope, design, cx),
            "audio-kit.spectrum-axis" => self.render_spectrum_axis_story(story, scope, design, cx),
            "px.line" => self.render_line_chart_story(story, scope, design, cx),
            "px.bar" => self.render_bar_chart_story(story, scope, design, cx),
            "px.scatter" => self.render_scatter_chart_story(story, scope, design, cx),
            "px.area" => self.render_area_chart_story(story, scope, design, cx),
            "px.heatmap" => self.render_heatmap_chart_story(story, scope, design, cx),
            "px.contour" => self.render_contour_chart_story(story, scope, design, cx),
            "px.isoline" => self.render_isoline_chart_story(story, scope, design, cx),
            "px.pie" => self.render_pie_chart_story(story, scope, design, cx),
            "px.donut" => self.render_donut_chart_story(story, scope, design, cx),
            "px.boxplot" => self.render_boxplot_chart_story(story, scope, design, cx),
            "px.treemap" => self.render_treemap_chart_story(story, scope, design, cx),
            "px.surface3d" => self.render_surface3d_chart_story(story, scope, design, cx),
            _ if self.renderers.contains(&story.id) => div()
                .child(
                    Text::new("Renderer metadata exists, but no preview handler is wired")
                        .muted(true),
                )
                .into_any_element(),
            _ => div()
                .child(Text::new("No renderer registered").muted(true))
                .into_any_element(),
        }
    }

    fn render_exported_ui_kit_component_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        _interactive: bool,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let label = text_prop(story, "label", story.title.as_str());
        let value = number_prop(story, "value", 0.64).clamp(0.0, 1.0);
        let disabled = bool_prop(story, "disabled", false);
        let selected = bool_prop(story, "selected", true);
        let open = bool_prop(story, "open", true);
        let variant_name = choice_prop(story, "variant", "default");
        let story_id = story.id.as_str();
        let scoped = |name: &str| lab_id(&[name, scope]);

        let element = match story_id {
            "ui-kit.button-set" => ButtonSet::new(scoped("button-set"))
                .options(vec![
                    ButtonSetOption::new("mix", "Mix"),
                    ButtonSetOption::new("edit", "Edit"),
                    ButtonSetOption::new("ship", "Ship"),
                ])
                .selected("edit")
                .disabled(disabled)
                .into_any_element(),
            "ui-kit.icon-button" => IconButton::new(scoped("icon-button"), "✦")
                .variant(icon_button_variant(&variant_name))
                .size(IconButtonSize::Lg)
                .selected(selected)
                .disabled(disabled)
                .aria_label(label)
                .into_any_element(),
            "ui-kit.alert" => Alert::new(scoped("alert"), label)
                .title("Alert")
                .variant(alert_variant(&variant_name))
                .closeable(open)
                .into_any_element(),
            "ui-kit.inline-alert" => InlineAlert::new(label)
                .variant(alert_variant(&variant_name))
                .into_any_element(),
            "ui-kit.toast" => Toast::new(scoped("toast"), label)
                .title("Toast")
                .variant(toast_variant(&variant_name))
                .closeable(open)
                .into_any_element(),
            "ui-kit.toast-container" => div()
                .relative()
                .w(px(360.0))
                .h(px(160.0))
                .child(
                    ToastContainer::new(ToastPosition::TopRight)
                        .toast(Toast::new(scoped("toast-container-item"), label).title("Toast")),
                )
                .into_any_element(),
            "ui-kit.checkbox" => Checkbox::new(scoped("checkbox"))
                .label(label)
                .checked(selected)
                .disabled(disabled)
                .size(CheckboxSize::Md)
                .design(design)
                .into_any_element(),
            "ui-kit.color-picker" => cx
                .new(|_| ColorPickerView::new(label, Color::from_hex(0x3b82f6)))
                .into_any_element(),
            "ui-kit.input" => Input::new(scoped("input"))
                .label("Label")
                .value(label)
                .placeholder("Type text")
                .size(InputSize::Md)
                .disabled(disabled)
                .into_any_element(),
            "ui-kit.number-input" => NumberInput::new(scoped("number-input"))
                .label("Value")
                .value(value)
                .width(160.0)
                .size(NumberInputSize::Md)
                .disabled(disabled)
                .into_any_element(),
            "ui-kit.select" => Select::new(scoped("select"))
                .label("Mode")
                .options(vec![
                    SelectOption::new("design", "Design"),
                    SelectOption::new("build", "Build"),
                    SelectOption::new("verify", "Verify"),
                ])
                .selected("build")
                .placeholder("Choose")
                .size(SelectSize::Md)
                .disabled(disabled)
                .is_open(open)
                .into_any_element(),
            "ui-kit.slider" => Slider::new(scoped("slider"))
                .label(label)
                .range(0.0, 1.0)
                .value(value as f32)
                .show_value(true)
                .width(260.0)
                .disabled(disabled)
                .design(design)
                .into_any_element(),
            "ui-kit.toggle" => Toggle::new(scoped("toggle"))
                .label(label)
                .checked(selected)
                .disabled(disabled)
                .size(ToggleSize::Md)
                .style(ToggleStyle::Sliding)
                .into_any_element(),
            "ui-kit.avatar" => Avatar::new()
                .name(label)
                .size(AvatarSize::Lg)
                .shape(AvatarShape::Circle)
                .status(AvatarStatus::Online)
                .into_any_element(),
            "ui-kit.avatar-group" => AvatarGroup::new()
                .avatars(vec![
                    Avatar::new().name("Ada Lovelace"),
                    Avatar::new().name("Grace Hopper"),
                    Avatar::new().name("Katherine Johnson"),
                ])
                .max_display(3)
                .size(AvatarSize::Md)
                .into_any_element(),
            "ui-kit.badge" => Badge::new(label)
                .variant(badge_variant(&variant_name))
                .size(BadgeSize::Lg)
                .rounded(true)
                .into_any_element(),
            "ui-kit.badge-dot" => BadgeDot::new()
                .variant(badge_variant(&variant_name))
                .size(px(12.0))
                .into_any_element(),
            "ui-kit.empty-state-component" => EmptyState::new(label)
                .description("No matching items")
                .action(Button::new(scoped("empty-action"), "Create"))
                .into_any_element(),
            "ui-kit.image-view-component" => ImageView::new(scoped("image-view"))
                .size(px(160.0))
                .placeholder_icon("image")
                .into_any_element(),
            "ui-kit.keyboard-shortcut-label" => KeyboardShortcutLabel::new("⌘ K")
                .size(KeyboardShortcutSize::Md)
                .into_any_element(),
            "ui-kit.progress-bar" => Progress::new(value as f32)
                .variant(progress_variant(&variant_name))
                .size(ProgressSize::Lg)
                .show_label(true)
                .into_any_element(),
            "ui-kit.circular-progress" => CircularProgress::new(value as f32)
                .variant(progress_variant(&variant_name))
                .size(px(64.0))
                .show_label(true)
                .into_any_element(),
            "ui-kit.qr-code-component" => QrCode::new("https://sotf.dev")
                .size(px(128.0))
                .into_any_element(),
            "ui-kit.animated-qr-code" => cx
                .new(|cx| AnimatedQrCode::new("https://sotf.dev/lab", px(48.0), cx))
                .into_any_element(),
            "ui-kit.spinner" => Spinner::new()
                .size(SpinnerSize::Lg)
                .label(label)
                .into_any_element(),
            "ui-kit.loading-dots" => LoadingDots::new().size(SpinnerSize::Lg).into_any_element(),
            "ui-kit.step-indicator-component" => StepIndicator::new(
                scoped("step-indicator"),
                vec![
                    StepItem::new("Props").status(StepItemStatus::Completed),
                    StepItem::new("Preview").status(StepItemStatus::Active),
                    StepItem::new("Ship").status(StepItemStatus::NotVisited),
                ],
            )
            .orientation(StepOrientation::Horizontal)
            .size(StepIndicatorSize::Md)
            .into_any_element(),
            "ui-kit.text-component" => Text::new(label).size(TextSize::Lg).into_any_element(),
            "ui-kit.heading" => Heading::new(label).level(2).into_any_element(),
            "ui-kit.code" => Code::new("ComponentStory::new(...)").into_any_element(),
            "ui-kit.link" => Link::new(scoped("link"), label)
                .href("https://sotf.dev")
                .external(true)
                .into_any_element(),
            "ui-kit.search-bar-component" => SearchBar::new(scoped("search-bar"))
                .value(label)
                .placeholder("Search stories")
                .size(SearchBarSize::Md)
                .show_clear(true)
                .into_any_element(),
            "ui-kit.tooltip-component" => Tooltip::new(label).into_any_element(),
            "ui-kit.with-tooltip" => WithTooltip::new(
                Button::new(scoped("with-tooltip-button"), "Hover target"),
                label,
            )
            .into_any_element(),
            "ui-kit.loading-overlay-component" => div()
                .relative()
                .w(px(300.0))
                .h(px(180.0))
                .bg(theme.surface)
                .border_1()
                .border_color(theme.border)
                .rounded_md()
                .child(LoadingOverlay::new(scoped("loading-overlay")).message(label))
                .into_any_element(),
            "ui-kit.pane-divider" => div()
                .h(px(120.0))
                .child(
                    PaneDivider::vertical(
                        scoped("pane-divider"),
                        gpui_ui_kit::CollapseDirection::Left,
                    )
                    .label(label),
                )
                .into_any_element(),
            "ui-kit.settings-row" => {
                let row = SettingsRow::new(label)
                    .description("A reusable settings row")
                    .control(Toggle::new(scoped("settings-row-toggle")).checked(selected));
                SettingsForm::new(scoped("settings-row-form"))
                    .row(row)
                    .into_any_element()
            }
            "ui-kit.settings-form-component" => SettingsForm::new(scoped("settings-form"))
                .section("Audio")
                .row(
                    SettingsRow::new(label)
                        .description("Design-token aware setting")
                        .control(Toggle::new(scoped("settings-form-toggle")).checked(selected)),
                )
                .into_any_element(),
            "ui-kit.sidebar-component" => Sidebar::new(scoped("sidebar"))
                .content(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(Heading::h3(label))
                        .child(Text::new("Sidebar content").muted(true)),
                )
                .design(design)
                .into_any_element(),
            "ui-kit.split-pane-component" => div()
                .w(px(420.0))
                .h(px(180.0))
                .child(
                    SplitPane::new(scoped("split-pane"))
                        .direction(SplitDirection::Horizontal)
                        .first(div().p_3().child("Left"))
                        .second(div().p_3().child("Right"))
                        .design(design),
                )
                .into_any_element(),
            "ui-kit.vstack" => VStack::new()
                .child(Text::new(label.clone()))
                .child(Button::new(scoped("vstack-button"), "Action"))
                .into_any_element(),
            "ui-kit.hstack" => HStack::new()
                .child(Text::new(label.clone()))
                .child(Badge::new("Live").variant(BadgeVariant::Success))
                .into_any_element(),
            "ui-kit.spacer" => HStack::new()
                .child(Text::new("Start"))
                .child(Spacer::new())
                .child(Text::new("End"))
                .into_any_element(),
            "ui-kit.divider" => VStack::new()
                .child(Text::new("Above"))
                .child(Divider::new())
                .child(Text::new("Below"))
                .into_any_element(),
            "ui-kit.status-bar-component" => StatusBar::new(scoped("status-bar"))
                .left(Text::new(label))
                .center(Badge::new("Ready").variant(BadgeVariant::Success))
                .right(Text::new("42ms"))
                .into_any_element(),
            "ui-kit.accordion-component" => Accordion::new()
                .items(vec![
                    AccordionItem::new("one", label).content(Text::new("Expanded content")),
                    AccordionItem::new("two", "Details").content(Text::new("Second panel")),
                ])
                .into_any_element(),
            "ui-kit.breadcrumbs-component" => Breadcrumbs::new()
                .items(vec![
                    BreadcrumbItem::new("home", "Home"),
                    BreadcrumbItem::new("lab", "Lab"),
                    BreadcrumbItem::new("story", label),
                ])
                .into_any_element(),
            "ui-kit.menu-component" => Menu::new(
                scoped("menu"),
                vec![
                    MenuItem::new("copy", "Copy"),
                    MenuItem::new("paste", "Paste").disabled(disabled),
                ],
            )
            .into_any_element(),
            "ui-kit.menu-bar" => MenuBar::new(vec![
                MenuBarItem::new("file", "File").with_items(vec![
                    MenuItem::new("new", "New"),
                    MenuItem::new("save", "Save"),
                ]),
                MenuBarItem::new("view", "View")
                    .with_items(vec![MenuItem::new("matrix", "Matrix")]),
            ])
            .into_any_element(),
            "ui-kit.dialog-component" => Dialog::new(scoped("dialog"))
                .title(label)
                .size(DialogSize::Md)
                .content(Text::new("Dialog content"))
                .into_any_element(),
            "ui-kit.confirm-dialog-component" => ConfirmDialog::new(scoped("confirm-dialog"))
                .title(label)
                .message("This action can be reviewed before it runs.")
                .variant(confirm_dialog_variant(&variant_name))
                .into_any_element(),
            "ui-kit.popover-component" => Popover::new(scoped("popover"))
                .content(div().p_3().child(label))
                .width(px(220.0))
                .into_any_element(),
            "ui-kit.context-menu-component" => ContextMenu::new(
                scoped("context-menu"),
                vec![
                    MenuItem::new("inspect", "Inspect"),
                    MenuItem::new("copy", "Copy"),
                ],
            )
            .into_any_element(),
            "ui-kit.tabs-component" => Tabs::new(scoped("tabs-component"))
                .tabs(vec![
                    TabItem::new("props", "Props"),
                    TabItem::new("preview", "Preview").badge("2"),
                    TabItem::new("qa", "QA"),
                ])
                .selected_index(1)
                .variant(tab_variant(&variant_name))
                .into_any_element(),
            "ui-kit.wizard-component" => Wizard::new()
                .steps(sample_wizard_steps())
                .variant(WizardVariant::Horizontal)
                .into_any_element(),
            "ui-kit.wizard-header" => WizardHeader::new()
                .title(label)
                .steps(sample_wizard_steps())
                .step_statuses(vec![
                    StepStatus::Completed,
                    StepStatus::Active,
                    StepStatus::NotVisited,
                ])
                .current_step(1)
                .into_any_element(),
            "ui-kit.wizard-navigation" => WizardNavigation::new(1, 3)
                .progress(value as f32)
                .status_message(label)
                .show_cancel(true)
                .into_any_element(),
            "ui-kit.command-palette-component" => CommandPalette::new(
                scoped("command-palette"),
                vec![
                    CommandItem::new("open", "Open Story").shortcut("⌘O"),
                    CommandItem::new("save", "Save Story").shortcut("⌘S"),
                    CommandItem::new("qa", "Run Conformance").category("QA"),
                ],
            )
            .query("story")
            .selected_index(0)
            .into_any_element(),
            "ui-kit.drag-list-component" => DragList::new(
                scoped("drag-list"),
                vec![
                    DragItem::new("one", Text::new("Props")),
                    DragItem::new("two", Text::new("Preview")),
                    DragItem::new("three", Text::new("QA")),
                ],
            )
            .into_any_element(),
            "ui-kit.notification-component" => Notification::new(scoped("notification"), label)
                .description("Conformance report passed")
                .variant(notification_variant(&variant_name))
                .dismissible(open)
                .into_any_element(),
            "ui-kit.tag-component" => Tag::new(scoped("tag"), label)
                .variant(tag_variant(&variant_name))
                .removable(open)
                .into_any_element(),
            "ui-kit.toolbar-component" => Toolbar::new(scoped("toolbar"))
                .item(ToolbarItem::button(scoped("toolbar-save"), "Save").active(selected))
                .separator()
                .item(ToolbarItem::button(scoped("toolbar-run"), "Run").disabled(disabled))
                .design(design)
                .into_any_element(),
            "ui-kit.tree-view-component" => {
                let mut expanded = HashSet::new();
                expanded.insert(SharedString::from("root"));
                TreeView::new(
                    scoped("tree-view"),
                    vec![TreeNode::new("root", label).children(vec![
                        TreeNode::new("child-props", "Props").leaf(true),
                        TreeNode::new("child-renderer", "Renderer").leaf(true),
                    ])],
                )
                .expanded(expanded)
                .selected("child-renderer")
                .into_any_element()
            }
            "ui-kit.table-component" => {
                let rows = vec![
                    ("Button".to_string(), "interactive".to_string()),
                    ("Chart".to_string(), "responsive".to_string()),
                ];
                Table::new(scoped("table"), rows)
                    .columns(vec![
                        Column::new("component", "Component").cell_render(
                            |row: &(String, String), _, _, _| Text::new(row.0.clone()),
                        ),
                        Column::new("status", "Status").cell_render(
                            |row: &(String, String), _, _, _| Badge::new(row.1.clone()),
                        ),
                    ])
                    .design(design)
                    .into_any_element()
            }
            "ui-kit.workflow-node" => WorkflowNode::new(
                scoped("workflow-node"),
                WorkflowNodeData::new(label, Position::new(0.0, 0.0)).with_ports(2, 1),
            )
            .selected(selected)
            .into_any_element(),
            "ui-kit.focus-group" => FocusGroup::new(scoped("focus-group"))
                .direction(FocusDirection::Horizontal)
                .wraparound(open)
                .child(Button::new(scoped("focus-first"), label.clone()).disabled(disabled))
                .child(Button::new(scoped("focus-second"), "Second"))
                .child(Input::new(scoped("focus-input")).placeholder("Focusable input"))
                .into_any_element(),
            "ui-kit.workflow-port" => HStack::new()
                .child(
                    Port::new(scoped("workflow-port-in"), PortDirection::Input, 0)
                        .connected(selected),
                )
                .child(Text::new(label.clone()).size(TextSize::Sm))
                .child(
                    Port::new(scoped("workflow-port-out"), PortDirection::Output, 0)
                        .connected(open)
                        .valid_target(Some(!disabled)),
                )
                .into_any_element(),
            "ui-kit.workflow-canvas" => div()
                .w(px(420.0))
                .h(px(260.0))
                .border_1()
                .border_color(theme.border)
                .rounded_md()
                .overflow_hidden()
                .child(cx.new(|cx| WorkflowCanvas::with_graph(sample_workflow_graph(label), cx)))
                .into_any_element(),
            "ui-kit.showcase-component" => div()
                .id(scoped("showcase-component"))
                .w(px(420.0))
                .max_h(px(300.0))
                .overflow_y_scroll()
                .child(cx.new(|cx| Showcase::embedded_section(ShowcaseSection::Buttons, cx)))
                .into_any_element(),
            _ => div()
                .child(Text::new("No exported component renderer registered").muted(true))
                .into_any_element(),
        };

        div()
            .max_w_full()
            .flex()
            .items_center()
            .justify_center()
            .child(element)
            .into_any_element()
    }

    fn render_button_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        interactive: bool,
        design: Arc<DesignSystem>,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = text_prop(story, "label", "Save");
        let variant = button_variant(&choice_prop(story, "variant", "primary"));
        let disabled = bool_prop(story, "disabled", false);
        let entity = self.entity.clone();
        let mut button = Button::new(lab_id(&["preview-button", scope]), label)
            .variant(variant)
            .size(ButtonSize::Lg)
            .design(design)
            .disabled(disabled);
        if interactive {
            button = button.on_click(move |_window, cx| {
                entity.update(cx, |this, _| {
                    this.save_status = Some("Preview button clicked".into());
                });
            });
        }
        div().child(button).into_any_element()
    }

    fn render_form_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        interactive: bool,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let label = text_prop(story, "label", "Gain");
        let value = number_prop(story, "value", 0.5).clamp(0.0, 1.0);
        let story_id = story.id.clone();
        let entity = self.entity.clone();

        let mut slider = Slider::new(lab_id(&["form-slider", scope]))
            .label(label.clone())
            .range(0.0, 1.0)
            .value(value as f32)
            .show_value(true)
            .design(design)
            .width(220.0);
        if interactive {
            slider = slider.on_change(move |new_value, _window, cx| {
                entity.update(cx, |this, _| {
                    this.set_prop(&story_id, "value", StoryPropValue::Number(new_value as f64));
                });
            });
        }

        div()
            .w(px(320.0))
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .child(
                Input::new(lab_id(&["form-input", scope]))
                    .value(label.clone())
                    .label("Label")
                    .readonly(true),
            )
            .child(slider)
            .child(
                Toggle::new(lab_id(&["form-toggle", scope]))
                    .checked(value > 0.5)
                    .label("Above midpoint")
                    .disabled(true),
            )
            .into_any_element()
    }

    fn render_status_story(
        &self,
        story: &ComponentStory,
        _scope: &str,
        _design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let label = text_prop(story, "label", "Ready");
        let variant = badge_variant(&choice_prop(story, "variant", "success"));
        let progress_variant = progress_variant(&choice_prop(story, "variant", "success"));
        let value = number_prop(story, "value", 0.72).clamp(0.0, 1.0) as f32;

        div()
            .w(px(360.0))
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Text::new("Build Status").size(TextSize::Sm))
                    .child(
                        Badge::new(label)
                            .variant(variant)
                            .size(BadgeSize::Lg)
                            .rounded(true),
                    ),
            )
            .child(
                Progress::new(value)
                    .variant(progress_variant)
                    .size(ProgressSize::Lg)
                    .show_label(true)
                    .aria_label("Story progress"),
            )
            .into_any_element()
    }

    fn render_navigation_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        _design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let selected = number_prop(story, "selected", 1.0).round().clamp(0.0, 2.0) as usize;
        let variant = tab_variant(&choice_prop(story, "variant", "pills"));
        let tabs = Tabs::new(lab_id(&["preview-tabs", scope]))
            .tabs(vec![
                TabItem::new("overview", "Overview"),
                TabItem::new("tokens", "Tokens").badge("4"),
                TabItem::new("motion", "Motion"),
            ])
            .selected_index(selected)
            .variant(variant)
            .aria_label("Component lab navigation");

        div()
            .w(px(420.0))
            .p_4()
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .child(tabs)
            .into_any_element()
    }

    fn render_feedback_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        _design: Arc<DesignSystem>,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let variant = alert_variant(&choice_prop(story, "variant", "info"));
        let message = text_prop(story, "message", "Design tokens validated");
        Alert::new(lab_id(&["preview-alert", scope]), message)
            .title("Conformance")
            .variant(variant)
            .closeable(false)
            .into_any_element()
    }

    fn render_card_story(
        &self,
        story: &ComponentStory,
        _scope: &str,
        _design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title = text_prop(story, "title", "Preview");
        let content = text_prop(story, "content", "Responsive component composition");
        let theme = cx.theme();

        Card::new()
            .header(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Heading::new(title).level(3))
                    .child(Badge::new("Lab").variant(BadgeVariant::Info).rounded(true)),
            )
            .content(
                div()
                    .w(px(360.0))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Text::new(content).size(TextSize::Sm).muted(true))
                    .child(
                        div()
                            .h(px(8.0))
                            .rounded_full()
                            .bg(theme.accent_muted)
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(0.62))
                                    .rounded_full()
                                    .bg(theme.accent),
                            ),
                    ),
            )
            .footer(
                Text::new("Theme-aware slots")
                    .size(TextSize::Xs)
                    .muted(true),
            )
            .into_any_element()
    }

    fn render_ui_kit_showcase_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        if scope.starts_with("matrix-") {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .justify_center()
                .items_center()
                .gap_2()
                .p_4()
                .child(
                    Badge::new("Showcase")
                        .variant(BadgeVariant::Info)
                        .rounded(true),
                )
                .child(Text::new(story.title.clone()).size(TextSize::Sm))
                .child(
                    Text::new(story.description.clone())
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                )
                .into_any_element();
        }

        self.ui_showcases
            .get(&story.id)
            .cloned()
            .map(|showcase| {
                div()
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .child(showcase)
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                div()
                    .child(Text::new("Showcase section unavailable").muted(true))
                    .into_any_element()
            })
    }

    fn render_potentiometer_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        interactive: bool,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let label = text_prop(story, "label", "Frequency");
        let value = number_prop(story, "value", 1000.0).clamp(20.0, 20_000.0);
        let scale = if choice_prop(story, "scale", "logarithmic") == "logarithmic" {
            AudioScale::Logarithmic
        } else {
            AudioScale::Linear
        };
        let story_id = story.id.clone();
        let entity = self.entity.clone();

        let mut knob = Potentiometer::new(lab_id(&["preview-pot", scope]))
            .label(label)
            .value(value)
            .min(20.0)
            .max(20_000.0)
            .unit("Hz")
            .scale(scale)
            .design(design)
            .size(PotentiometerSize::Lg);
        if interactive {
            knob = knob.on_change(move |new_value, _window, cx| {
                entity.update(cx, |this, _| {
                    this.set_prop(&story_id, "value", StoryPropValue::Number(new_value));
                });
            });
        }

        div()
            .p_6()
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .child(knob)
            .into_any_element()
    }

    fn render_vertical_slider_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        interactive: bool,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let label = text_prop(story, "label", "Gain");
        let min = number_prop(story, "min", -60.0);
        let max = number_prop(story, "max", 6.0).max(min + 0.001);
        let value = number_prop(story, "value", -6.0).clamp(min, max);
        let peak = number_prop(story, "peak", -1.5).clamp(min, max);
        let scale = if choice_prop(story, "scale", "linear") == "logarithmic" {
            AudioScale::Logarithmic
        } else {
            AudioScale::Linear
        };
        let story_id = story.id.clone();
        let entity = self.entity.clone();

        let mut slider = VerticalSlider::new(lab_id(&["preview-vertical-slider", scope]))
            .label(label)
            .value(value)
            .min(min)
            .max(max)
            .unit("dB")
            .peak(Some(peak))
            .scale(scale)
            .size(VerticalSliderSize::Lg)
            .height(170.0)
            .selected(interactive)
            .design(design);

        if bool_prop(story, "ticks", true) {
            slider = slider.with_ticks();
        }

        if interactive {
            slider = slider.on_change(move |new_value, _window, cx| {
                entity.update(cx, |this, _| {
                    this.set_prop(&story_id, "value", StoryPropValue::Number(new_value));
                });
            });
        }

        div()
            .min_w(px(140.0))
            .h(px(260.0))
            .flex()
            .items_center()
            .justify_center()
            .p_5()
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .child(slider)
            .into_any_element()
    }

    fn render_volume_knob_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        interactive: bool,
        _design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let label = text_prop(story, "label", "Output");
        let value = number_prop(story, "value", 0.72).clamp(0.0, 1.0) as f32;
        let muted = bool_prop(story, "muted", false);
        let story_id = story.id.clone();
        let mute_story_id = story.id.clone();
        let entity = self.entity.clone();
        let mute_entity = self.entity.clone();

        let mut knob = VolumeKnob::new()
            .id(lab_id(&["preview-volume-knob", scope]))
            .label(label)
            .value(value)
            .muted(muted)
            .size(px(if scope.starts_with("matrix-") {
                52.0
            } else {
                72.0
            }))
            .accent_color(theme.accent)
            .bg_color(theme.background)
            .text_color(theme.text_primary)
            .muted_color(theme.text_muted);

        if interactive {
            knob = knob
                .on_change(move |new_value, _window, cx| {
                    entity.update(cx, |this, _| {
                        this.set_prop(&story_id, "value", StoryPropValue::Number(new_value as f64));
                    });
                })
                .on_mute_toggle(move |new_muted, _window, cx| {
                    mute_entity.update(cx, |this, _| {
                        this.set_prop(&mute_story_id, "muted", StoryPropValue::Bool(new_muted));
                    });
                });
        }

        div()
            .min_w(px(150.0))
            .h(px(170.0))
            .flex()
            .items_center()
            .justify_center()
            .p_5()
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .child(knob)
            .into_any_element()
    }

    fn render_meter_story(
        &self,
        story: &ComponentStory,
        _scope: &str,
        _design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let level_db = number_prop(story, "level_db", -12.0);
        let peak_db = number_prop(story, "peak_db", -3.0);

        div()
            .w(px(140.0))
            .h(px(180.0))
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .p_4()
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .child(
                div().h(px(120.0)).flex().items_stretch().child(
                    LevelMeterElement::new(level_db, "L")
                        .peak(peak_db)
                        .width(px(24.0)),
                ),
            )
            .child(Text::new(format!("{level_db:.1} dB")).size(TextSize::Sm))
            .into_any_element()
    }

    fn render_horizontal_meter_story(
        &self,
        story: &ComponentStory,
        _scope: &str,
        _design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let kind = choice_prop(story, "kind", "lufs");
        let label = text_prop(story, "label", "LUFS");
        let raw_value = number_prop(story, "value", -18.0);
        let mut tick_config = match kind.as_str() {
            "stereo_width" => TickConfig::stereo_width(),
            "peak_spread" => TickConfig::peak_spread(),
            _ => TickConfig::lufs(),
        };
        tick_config.tick_color = theme.border_hover;
        let value = raw_value.clamp(tick_config.min, tick_config.max);
        let meter_theme = HorizontalMeterTheme {
            color_normal: theme.success,
            color_warning: theme.warning,
            color_critical: theme.error,
            color_info: theme.info,
            color_background: theme.background,
            color_border: theme.border,
            color_text: theme.text_secondary,
            use_gradient: bool_prop(story, "gradient", true),
            ..HorizontalMeterTheme::default()
        };

        div()
            .w(px(430.0))
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .child(render_horizontal_meter_bar(
                label,
                value,
                &tick_config,
                meter_theme.clone(),
            ))
            .child(render_tick_row(
                &tick_config,
                meter_theme.label_width,
                meter_theme.value_width,
            ))
            .into_any_element()
    }

    fn render_spectrum_story(
        &self,
        story: &ComponentStory,
        _scope: &str,
        _design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let bins = number_prop(story, "bins", 64.0).clamp(8.0, 128.0).round() as usize;
        let magnitudes: Vec<f32> = (0..bins)
            .map(|index| {
                let t = index as f32 / bins.max(1) as f32;
                -80.0 + (t * std::f32::consts::TAU).sin().abs() * 60.0
            })
            .collect();

        div()
            .w(px(360.0))
            .p_4()
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .child(SpectrumElement::new(magnitudes).height(px(150.0)))
            .into_any_element()
    }

    fn render_spectrum_axis_story(
        &self,
        story: &ComponentStory,
        _scope: &str,
        _design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let min_freq = number_prop(story, "min_freq", 20.0).clamp(1.0, 96_000.0) as f32;
        let max_freq =
            number_prop(story, "max_freq", 20_000.0).clamp(min_freq as f64 + 1.0, 192_000.0) as f32;
        let axis_theme = SpectrumAxisTheme {
            text_color: theme.text_secondary,
            ..SpectrumAxisTheme::default()
        };
        let db_axis_width = axis_theme.db_axis_width;
        let magnitudes: Vec<f32> = (0..72)
            .map(|index| {
                let t = index as f32 / 71.0;
                -86.0 + (t * std::f32::consts::TAU * 1.5).sin().abs() * 54.0
            })
            .collect();

        div()
            .w(px(460.0))
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .child(
                div()
                    .h(px(170.0))
                    .flex()
                    .gap_1()
                    .child(render_spectrum_db_axis(axis_theme.clone()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_hidden()
                            .rounded_sm()
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.background)
                            .child(
                                SpectrumElement::new(magnitudes)
                                    .frequency_range(min_freq, max_freq)
                                    .height(px(170.0)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(div().w(px(db_axis_width)))
                    .child(render_spectrum_frequency_axis(
                        min_freq, max_freq, axis_theme,
                    )),
            )
            .into_any_element()
    }

    fn render_line_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let fill = bool_prop(story, "fill", true);
        let compact = scope.starts_with("matrix-");
        let (min_width, min_height) = self.chart_minimum(scope);
        let data = line_story_data(&choice_prop(story, "series", "sine"));

        let mut chart = line(&data.x, &data.y)
            .title(data.title)
            .x_label(data.x_label)
            .y_label(data.y_label)
            .label(data.primary_label)
            .color(0x2563eb)
            .stroke_width(if compact { 2.0 } else { 2.5 })
            .show_points(!compact)
            .x_scale(data.x_scale)
            .design(design)
            .legend_position(if compact {
                LegendPosition::Hidden
            } else {
                LegendPosition::Bottom
            });

        if let Some((min, max)) = data.y_range {
            chart = chart.y_range(min, max);
        }

        if let Some(extra) = data.comparison_y {
            chart = chart
                .add_series(&extra, Some(data.comparison_label), 0xf97316, 1.75, 0.9)
                .series_dash_array(StrokeDashArray::Dashed);
        }

        chart = if fill {
            chart
                .fill()
                .min_size(min_width, min_height)
                .aspect_ratio(self.layout_constraints.aspect_ratio)
        } else {
            chart.size(min_width, min_height)
        };

        match chart.build() {
            Ok(chart) => div()
                .w_full()
                .h_full()
                .min_w_0()
                .min_h_0()
                .child(chart)
                .into_any_element(),
            Err(err) => render_chart_error(err, theme),
        }
    }

    fn render_bar_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let fill = bool_prop(story, "fill", true);
        let compact = scope.starts_with("matrix-");
        let (min_width, min_height) = self.chart_minimum(scope);
        let bar_count = number_prop(story, "bars", 8.0).round().clamp(3.0, 12.0) as usize;
        let data = bar_story_data(bar_count);

        let mut chart = bar(&data.categories, &data.values)
            .title("Category Mix")
            .label("Current")
            .color(0x2563eb)
            .bar_gap(if compact { 2.0 } else { 4.0 })
            .border_radius(if compact { 2.0 } else { 4.0 })
            .add_series(&data.comparison_values, Some("Target"), 0xf97316, 0.76)
            .design(design)
            .legend_position(if compact {
                LegendPosition::Hidden
            } else {
                LegendPosition::Bottom
            });

        chart = if fill {
            chart
                .fill()
                .min_size(min_width, min_height)
                .aspect_ratio(self.layout_constraints.aspect_ratio)
        } else {
            chart.size(min_width, min_height)
        };

        match chart.build() {
            Ok(chart) => div()
                .w_full()
                .h_full()
                .min_w_0()
                .min_h_0()
                .child(chart)
                .into_any_element(),
            Err(err) => render_chart_error(err, theme),
        }
    }

    fn render_scatter_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let fill = bool_prop(story, "fill", true);
        let compact = scope.starts_with("matrix-");
        let (min_width, min_height) = self.chart_minimum(scope);
        let count = number_prop(story, "points", 48.0).round().clamp(12.0, 96.0) as usize;
        let (x, y) = scatter_story_data(count);

        let mut chart = scatter(&x, &y)
            .title("Correlation")
            .color(0x2563eb)
            .point_radius(if compact { 3.0 } else { 4.5 })
            .opacity(0.78)
            .design(design);

        chart = if fill {
            chart
                .fill()
                .min_size(min_width, min_height)
                .aspect_ratio(self.layout_constraints.aspect_ratio)
        } else {
            chart.size(min_width, min_height)
        };

        render_chart_result(chart.build(), theme)
    }

    fn render_area_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let fill = bool_prop(story, "fill", true);
        let (min_width, min_height) = self.chart_minimum(scope);
        let data = area_story_data(&choice_prop(story, "series", "envelope"));

        let mut chart = area(&data.x, &data.y)
            .title(data.title)
            .color(0x14b8a6)
            .opacity(0.58)
            .design(design);
        if let Some(y0) = data.y0 {
            chart = chart.y0(&y0);
        }
        chart = if fill {
            chart
                .fill()
                .min_size(min_width, min_height)
                .aspect_ratio(self.layout_constraints.aspect_ratio)
        } else {
            chart.size(min_width, min_height)
        };

        render_chart_result(chart.build(), theme)
    }

    fn render_heatmap_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let fill = bool_prop(story, "fill", true);
        let (min_width, min_height) = self.chart_minimum(scope);
        let size = number_prop(story, "size", 18.0).round().clamp(8.0, 32.0) as usize;
        let z = scalar_field_data(size, size);
        let mut chart = heatmap(&z, size, size)
            .title("Response Field")
            .color_scale(color_scale(&choice_prop(story, "scale", "viridis")))
            .design(design);
        chart = if fill {
            chart
                .fill()
                .min_size(min_width, min_height)
                .aspect_ratio(self.layout_constraints.aspect_ratio)
        } else {
            chart.size(min_width, min_height)
        };

        render_chart_result(chart.build(), theme)
    }

    fn render_contour_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let fill = bool_prop(story, "fill", true);
        let (min_width, min_height) = self.chart_minimum(scope);
        let size = number_prop(story, "size", 24.0).round().clamp(12.0, 40.0) as usize;
        let z = scalar_field_data(size, size);
        let mut chart = contour(&z, size, size)
            .title("Density Bands")
            .thresholds(vec![-0.8, -0.4, 0.0, 0.4, 0.8])
            .color_scale(ColorScale::Plasma)
            .design(design);
        chart = if fill {
            chart
                .fill()
                .min_size(min_width, min_height)
                .aspect_ratio(self.layout_constraints.aspect_ratio)
        } else {
            chart.size(min_width, min_height)
        };

        render_chart_result(chart.build(), theme)
    }

    fn render_isoline_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let fill = bool_prop(story, "fill", true);
        let (min_width, min_height) = self.chart_minimum(scope);
        let size = number_prop(story, "size", 24.0).round().clamp(12.0, 40.0) as usize;
        let z = scalar_field_data(size, size);
        let mut chart = isoline(&z, size, size)
            .title("Level Curves")
            .levels(vec![-0.6, -0.2, 0.2, 0.6])
            .color(0x334155)
            .stroke_width(1.5)
            .design(design);
        chart = if fill {
            chart
                .fill()
                .min_size(min_width, min_height)
                .aspect_ratio(self.layout_constraints.aspect_ratio)
        } else {
            chart.size(min_width, min_height)
        };

        render_chart_result(chart.build(), theme)
    }

    fn render_pie_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_pie_like_chart_story(story, scope, design, cx, bool_prop(story, "donut", false))
    }

    fn render_donut_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_pie_like_chart_story(story, scope, design, cx, true)
    }

    fn render_pie_like_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
        donut_chart: bool,
    ) -> AnyElement {
        let theme = cx.theme();
        let fill = bool_prop(story, "fill", true);
        let (min_width, min_height) = self.chart_minimum(scope);
        let count = number_prop(story, "slices", 5.0).round().clamp(3.0, 8.0) as usize;
        let labels = (0..count)
            .map(|index| format!("S{}", index + 1))
            .collect::<Vec<_>>();
        let values = (0..count)
            .map(|index| 12.0 + (index as f64 * 1.7).sin().abs() * 36.0 + index as f64 * 4.0)
            .collect::<Vec<_>>();
        let mut chart = if donut_chart {
            donut(&values)
        } else {
            pie(&values)
        }
        .labels(&labels)
        .title(if donut_chart { "Share" } else { "Mix" })
        .design(design);

        chart = if fill {
            chart
                .fill()
                .min_size(min_width, min_height)
                .aspect_ratio(self.layout_constraints.aspect_ratio)
        } else {
            chart.size(min_width, min_height)
        };

        render_chart_result(chart.build(), theme)
    }

    fn render_boxplot_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let fill = bool_prop(story, "fill", true);
        let (min_width, min_height) = self.chart_minimum(scope);
        let groups = number_prop(story, "groups", 5.0).round().clamp(3.0, 8.0) as usize;
        let (x, y) = boxplot_story_data(groups);
        let mut chart = boxplot(&x, &y)
            .title("Distribution")
            .bins(groups)
            .box_color(0x2563eb)
            .median_color(0xf97316)
            .design(design);

        chart = if fill {
            chart
                .fill()
                .min_size(min_width, min_height)
                .aspect_ratio(self.layout_constraints.aspect_ratio)
        } else {
            chart.size(min_width, min_height)
        };

        render_chart_result(chart.build(), theme)
    }

    fn render_treemap_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let fill = bool_prop(story, "fill", true);
        let (min_width, min_height) = self.chart_minimum(scope);
        let root = treemap_story_data();
        let mut chart = treemap(&root)
            .title("Toolkit Surface")
            .tiling_method(tiling_method(&choice_prop(story, "tiling", "squarify")))
            .padding(2.0)
            .design(design);

        chart = if fill {
            chart
                .fill()
                .min_size(min_width, min_height)
                .aspect_ratio(self.layout_constraints.aspect_ratio)
        } else {
            chart.size(min_width, min_height)
        };

        render_chart_result(chart.build(), theme)
    }

    fn render_surface3d_chart_story(
        &self,
        story: &ComponentStory,
        scope: &str,
        design: Arc<DesignSystem>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let fill = bool_prop(story, "fill", true);
        let (min_width, min_height) = self.chart_minimum(scope);
        let size = number_prop(story, "size", 22.0).round().clamp(8.0, 34.0) as usize;
        let z = scalar_field_data(size, size);
        let mut chart = surface3d(&z, size, size)
            .title("Surface Response")
            .colormap(surface_colormap(&choice_prop(story, "colormap", "viridis")))
            .wireframe(bool_prop(story, "wireframe", false))
            .design(design);

        chart = if fill {
            chart
                .fill()
                .min_size(min_width, min_height)
                .aspect_ratio(self.layout_constraints.aspect_ratio)
        } else {
            chart.size(min_width, min_height)
        };

        render_chart_result(chart.build(), theme)
    }

    fn chart_minimum(&self, scope: &str) -> (f32, f32) {
        if scope.starts_with("matrix-") {
            (220.0, 145.0)
        } else {
            (
                self.layout_constraints.min_width,
                self.layout_constraints.min_height,
            )
        }
    }
}

fn build_ui_showcase_entities(
    story_ids: &[String],
    cx: &mut Context<ComponentLab>,
) -> BTreeMap<String, Entity<Showcase>> {
    let mut showcases = BTreeMap::new();
    for story_id in story_ids {
        if let Some(section) = showcase_section_for_story_id(story_id) {
            let showcase = cx.new(|cx| Showcase::embedded_section(section, cx));
            showcases.insert(story_id.clone(), showcase);
        }
    }
    showcases
}

fn showcase_section_for_story_id(story_id: &str) -> Option<ShowcaseSection> {
    Some(match story_id {
        "ui-kit.buttons" => ShowcaseSection::Buttons,
        "ui-kit.text" => ShowcaseSection::Text,
        "ui-kit.badges" => ShowcaseSection::Badges,
        "ui-kit.avatars" => ShowcaseSection::Avatars,
        "ui-kit.form-controls" => ShowcaseSection::FormControls,
        "ui-kit.progress" => ShowcaseSection::Progress,
        "ui-kit.alerts" => ShowcaseSection::Alerts,
        "ui-kit.tabs" => ShowcaseSection::Tabs,
        "ui-kit.cards" => ShowcaseSection::Cards,
        "ui-kit.breadcrumbs" => ShowcaseSection::Breadcrumbs,
        "ui-kit.spinners" => ShowcaseSection::Spinners,
        "ui-kit.layout" => ShowcaseSection::Layout,
        "ui-kit.icon-buttons" => ShowcaseSection::IconButtons,
        "ui-kit.toasts" => ShowcaseSection::Toasts,
        "ui-kit.dialog" => ShowcaseSection::Dialog,
        "ui-kit.menu" => ShowcaseSection::Menu,
        "ui-kit.table" => ShowcaseSection::Table,
        "ui-kit.tooltips" => ShowcaseSection::Tooltips,
        "ui-kit.accordion" => ShowcaseSection::Accordion,
        "ui-kit.wizard" => ShowcaseSection::Wizard,
        "ui-kit.workflow" => ShowcaseSection::Workflow,
        "ui-kit.qr-code" => ShowcaseSection::QrCode,
        "ui-kit.context-menu" => ShowcaseSection::ContextMenu,
        "ui-kit.popover" => ShowcaseSection::Popover,
        "ui-kit.sidebar" => ShowcaseSection::Sidebar,
        "ui-kit.status-bar" => ShowcaseSection::StatusBar,
        "ui-kit.search-bar" => ShowcaseSection::SearchBar,
        "ui-kit.keyboard-shortcut" => ShowcaseSection::KeyboardShortcut,
        "ui-kit.empty-state" => ShowcaseSection::EmptyState,
        "ui-kit.confirm-dialog" => ShowcaseSection::ConfirmDialog,
        "ui-kit.split-pane" => ShowcaseSection::SplitPane,
        "ui-kit.image-view" => ShowcaseSection::ImageView,
        "ui-kit.settings-form" => ShowcaseSection::SettingsForm,
        "ui-kit.step-indicator" => ShowcaseSection::StepIndicator,
        "ui-kit.loading-overlay" => ShowcaseSection::LoadingOverlay,
        "ui-kit.tag" => ShowcaseSection::Tag,
        "ui-kit.toolbar" => ShowcaseSection::Toolbar,
        "ui-kit.notification" => ShowcaseSection::Notification,
        "ui-kit.tree-view" => ShowcaseSection::TreeView,
        "ui-kit.drag-list" => ShowcaseSection::DragList,
        "ui-kit.command-palette" => ShowcaseSection::CommandPalette,
        "ui-kit.accessibility" => ShowcaseSection::Accessibility,
        _ => return None,
    })
}

fn render_chart_error(err: impl std::fmt::Display, theme: gpui_ui_kit::theme::Theme) -> AnyElement {
    div()
        .w(px(320.0))
        .p_4()
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .rounded_md()
        .child(
            Text::new(format!("Chart failed: {err}"))
                .size(TextSize::Xs)
                .color(theme.text_secondary),
        )
        .into_any_element()
}

fn render_chart_result<E, Err>(
    result: Result<E, Err>,
    theme: gpui_ui_kit::theme::Theme,
) -> AnyElement
where
    E: IntoElement,
    Err: std::fmt::Display,
{
    match result {
        Ok(chart) => div()
            .w_full()
            .h_full()
            .min_w_0()
            .min_h_0()
            .child(chart)
            .into_any_element(),
        Err(err) => render_chart_error(err, theme),
    }
}

fn apply_preview_builder_style(
    el: Stateful<Div>,
    constraints: PreviewLayoutConstraints,
    theme: gpui_ui_kit::theme::Theme,
) -> Stateful<Div> {
    let el = match constraints.horizontal_align {
        PreviewAlign::Start => el.justify_start(),
        PreviewAlign::Center | PreviewAlign::Stretch => el.justify_center(),
        PreviewAlign::End => el.justify_end(),
    };
    let el = match constraints.vertical_align {
        PreviewAlign::Start => el.items_start(),
        PreviewAlign::Center => el.items_center(),
        PreviewAlign::End => el.items_end(),
        PreviewAlign::Stretch => el.items_stretch(),
    };
    let el = match constraints.overflow {
        PreviewOverflow::Hidden => el.overflow_hidden(),
        PreviewOverflow::Scroll => el.overflow_y_scroll(),
        PreviewOverflow::Visible => el,
    };
    let el = match constraints.surface {
        PreviewSurface::Background => el.bg(theme.background),
        PreviewSurface::Surface => el.bg(theme.surface),
        PreviewSurface::Transparent => el,
    };
    if constraints.border {
        el.border_1().border_color(theme.border)
    } else {
        el
    }
}

fn live_reload_status(story_count: usize, token_reports: &[LivePreviewTokenReload]) -> String {
    if token_reports.is_empty() {
        return format!("Live reloaded {story_count} story document(s)");
    }

    let failed_tokens = token_reports
        .iter()
        .filter(|token| !token.report.passed)
        .count();
    if failed_tokens == 0 {
        let token_count = token_reports
            .iter()
            .map(|token| token.report.token_count)
            .sum::<usize>();
        format!(
            "Live reloaded {story_count} story document(s), {} token file(s), {token_count} token(s)",
            token_reports.len()
        )
    } else {
        format!(
            "Live reloaded {story_count} story document(s); {failed_tokens} token file(s) failed validation"
        )
    }
}

fn ui_kit_exported_component_story_id(story_id: &str) -> bool {
    UI_KIT_EXPORTED_COMPONENT_STORY_IDS.contains(&story_id)
}

fn icon_button_variant(value: &str) -> IconButtonVariant {
    match value {
        "outline" => IconButtonVariant::Outline,
        "primary" | "secondary" | "success" | "warning" | "error" | "info" => {
            IconButtonVariant::Filled
        }
        _ => IconButtonVariant::Ghost,
    }
}

fn toast_variant(value: &str) -> ToastVariant {
    match value {
        "success" => ToastVariant::Success,
        "warning" => ToastVariant::Warning,
        "error" => ToastVariant::Error,
        _ => ToastVariant::Info,
    }
}

fn notification_variant(value: &str) -> NotificationVariant {
    match value {
        "success" => NotificationVariant::Success,
        "warning" => NotificationVariant::Warning,
        "error" => NotificationVariant::Error,
        _ => NotificationVariant::Info,
    }
}

fn tag_variant(value: &str) -> TagVariant {
    match value {
        "primary" | "info" => TagVariant::Primary,
        "success" => TagVariant::Success,
        "warning" => TagVariant::Warning,
        "error" => TagVariant::Error,
        "outline" | "ghost" | "secondary" => TagVariant::Outlined,
        _ => TagVariant::Default,
    }
}

fn confirm_dialog_variant(value: &str) -> ConfirmDialogVariant {
    match value {
        "warning" => ConfirmDialogVariant::Warning,
        "error" => ConfirmDialogVariant::Destructive,
        _ => ConfirmDialogVariant::Default,
    }
}

fn sample_wizard_steps() -> Vec<WizardStep> {
    vec![
        WizardStep::new("props", "Props").description("Edit story props"),
        WizardStep::new("preview", "Preview").description("Inspect responsive output"),
        WizardStep::new("qa", "QA").description("Run conformance"),
    ]
}

fn sample_workflow_graph(label: impl Into<String>) -> WorkflowGraph {
    let mut graph = WorkflowGraph::new();
    let source = graph.add_node(
        WorkflowNodeData::new(label, Position::new(48.0, 72.0))
            .with_ports(0, 1)
            .with_size(150.0, 90.0),
    );
    let sink = graph.add_node(
        WorkflowNodeData::new("Preview", Position::new(250.0, 96.0))
            .with_ports(1, 0)
            .with_size(150.0, 90.0),
    );
    let _ = graph.add_connection(source, 0, sink, 0);
    graph
}

struct LineStoryData {
    x: Vec<f64>,
    y: Vec<f64>,
    comparison_y: Option<Vec<f64>>,
    title: &'static str,
    x_label: &'static str,
    y_label: &'static str,
    primary_label: &'static str,
    comparison_label: &'static str,
    x_scale: ScaleType,
    y_range: Option<(f64, f64)>,
}

struct AreaStoryData {
    x: Vec<f64>,
    y: Vec<f64>,
    y0: Option<Vec<f64>>,
    title: &'static str,
}

fn line_story_data(series: &str) -> LineStoryData {
    match series {
        "sweep" => {
            let x: Vec<f64> = (0..72)
                .map(|index| 20.0 * 1000.0_f64.powf(index as f64 / 71.0))
                .collect();
            let y: Vec<f64> = x
                .iter()
                .map(|frequency| {
                    let octave = (frequency / 1000.0).log2();
                    (octave * 1.7).sin() * 2.4 - (frequency / 18_000.0).sqrt() * 1.6
                })
                .collect();
            let comparison_y = x
                .iter()
                .map(|frequency| -0.8 * (frequency / 20_000.0).sqrt())
                .collect();
            LineStoryData {
                x,
                y,
                comparison_y: Some(comparison_y),
                title: "Frequency Sweep",
                x_label: "Hz",
                y_label: "dB",
                primary_label: "Measured",
                comparison_label: "Target",
                x_scale: ScaleType::Log,
                y_range: Some((-7.0, 5.0)),
            }
        }
        "flat" => {
            let x: Vec<f64> = (0..40).map(|index| index as f64).collect();
            let y: Vec<f64> = x
                .iter()
                .map(|value| (value * 0.41).sin() * 0.18 + (value * 0.09).cos() * 0.08)
                .collect();
            LineStoryData {
                x,
                y,
                comparison_y: None,
                title: "Flat Reference",
                x_label: "Step",
                y_label: "Delta",
                primary_label: "Reference",
                comparison_label: "Target",
                x_scale: ScaleType::Linear,
                y_range: Some((-1.0, 1.0)),
            }
        }
        _ => {
            let x: Vec<f64> = (0..64).map(|index| index as f64 / 6.0).collect();
            let y: Vec<f64> = x.iter().map(|value| value.sin()).collect();
            let comparison_y: Vec<f64> =
                x.iter().map(|value| (value * 0.72).cos() * 0.62).collect();
            LineStoryData {
                x,
                y,
                comparison_y: Some(comparison_y),
                title: "Sine Envelope",
                x_label: "Time",
                y_label: "Value",
                primary_label: "Sine",
                comparison_label: "Cosine",
                x_scale: ScaleType::Linear,
                y_range: Some((-1.2, 1.2)),
            }
        }
    }
}

fn area_story_data(series: &str) -> AreaStoryData {
    match series {
        "decay" => {
            let x: Vec<f64> = (0..64).map(|index| index as f64 / 8.0).collect();
            let y: Vec<f64> = x
                .iter()
                .map(|value| (value * 1.2).sin().abs() * (-value / 8.0).exp() + 0.04)
                .collect();
            AreaStoryData {
                x,
                y,
                y0: None,
                title: "Decay Envelope",
            }
        }
        "baseline" => {
            let x: Vec<f64> = (0..72).map(|index| index as f64 / 9.0).collect();
            let y0: Vec<f64> = x.iter().map(|value| value.sin() * 0.12 - 0.25).collect();
            let y: Vec<f64> = x
                .iter()
                .zip(y0.iter())
                .map(|(value, base)| base + 0.42 + (value * 1.4).cos().abs() * 0.28)
                .collect();
            AreaStoryData {
                x,
                y,
                y0: Some(y0),
                title: "Baseline Band",
            }
        }
        _ => {
            let x: Vec<f64> = (0..72).map(|index| index as f64 / 9.0).collect();
            let y: Vec<f64> = x
                .iter()
                .map(|value| (value * 1.1).sin().abs() * 0.72 + (value * 0.45).cos() * 0.08)
                .collect();
            AreaStoryData {
                x,
                y,
                y0: None,
                title: "Signal Envelope",
            }
        }
    }
}

fn scatter_story_data(count: usize) -> (Vec<f64>, Vec<f64>) {
    let mut x = Vec::with_capacity(count);
    let mut y = Vec::with_capacity(count);
    for index in 0..count {
        let t = index as f64 / count.max(1) as f64;
        let cluster = (index % 3) as f64;
        x.push(t * 12.0 + cluster * 0.9);
        y.push((t * std::f64::consts::TAU * 1.8).sin() * 2.2 + cluster * 1.7 + t * 2.4);
    }
    (x, y)
}

fn scalar_field_data(width: usize, height: usize) -> Vec<f64> {
    let mut z = Vec::with_capacity(width * height);
    for row in 0..height {
        let y = row as f64 / height.saturating_sub(1).max(1) as f64 * 4.0 - 2.0;
        for col in 0..width {
            let x = col as f64 / width.saturating_sub(1).max(1) as f64 * 4.0 - 2.0;
            let ridge = (x * 2.3).sin() * (y * 1.7).cos();
            let mound = (-(x * x + y * y) * 0.75).exp() * 1.4;
            z.push(ridge * 0.62 + mound - 0.45);
        }
    }
    z
}

fn boxplot_story_data(groups: usize) -> (Vec<f64>, Vec<f64>) {
    let samples_per_group = 18usize;
    let mut x = Vec::with_capacity(groups * samples_per_group);
    let mut y = Vec::with_capacity(groups * samples_per_group);
    for group in 0..groups {
        for sample in 0..samples_per_group {
            let t = sample as f64 / samples_per_group as f64;
            x.push(group as f64 + 1.0);
            y.push(
                group as f64 * 0.55
                    + (t * std::f64::consts::TAU).sin() * (0.7 + group as f64 * 0.05)
                    + (sample % 5) as f64 * 0.16,
            );
        }
    }
    (x, y)
}

fn treemap_story_data() -> TreemapNode {
    TreemapNode::new("gpui-toolkit", 0.0)
        .add_child(
            TreemapNode::new("UI Kit", 0.0)
                .add_child(TreemapNode::new("Controls", 32.0))
                .add_child(TreemapNode::new("Layout", 18.0))
                .add_child(TreemapNode::new("Feedback", 14.0)),
        )
        .add_child(
            TreemapNode::new("Charts", 0.0)
                .add_child(TreemapNode::new("1D", 22.0))
                .add_child(TreemapNode::new("2D", 28.0))
                .add_child(TreemapNode::new("Hierarchy", 12.0)),
        )
        .add_child(
            TreemapNode::new("Audio", 0.0)
                .add_child(TreemapNode::new("Controls", 24.0))
                .add_child(TreemapNode::new("Meters", 16.0)),
        )
}

struct BarStoryData {
    categories: Vec<String>,
    values: Vec<f64>,
    comparison_values: Vec<f64>,
}

fn bar_story_data(count: usize) -> BarStoryData {
    let categories = (0..count)
        .map(|index| format!("B{}", index + 1))
        .collect::<Vec<_>>();
    let values = (0..count)
        .map(|index| {
            let t = index as f64 / count.max(1) as f64;
            34.0 + (t * std::f64::consts::TAU).sin().abs() * 42.0 + index as f64 * 1.8
        })
        .collect::<Vec<_>>();
    let comparison_values = (0..count)
        .map(|index| {
            let t = index as f64 / count.max(1) as f64;
            38.0 + (t * std::f64::consts::TAU + 0.8).cos().abs() * 34.0
        })
        .collect();

    BarStoryData {
        categories,
        values,
        comparison_values,
    }
}

impl Render for ComponentLab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("gpui-component-lab-root")
            .size_full()
            .bg(theme.background)
            .text_color(theme.text_primary)
            .flex()
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .p_5()
                    .child(self.render_toolbar(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .child(self.render_preview_area(cx)),
                            )
                            .child(self.render_controls_panel(cx)),
                    ),
            )
    }
}

fn lab_id(parts: &[&str]) -> String {
    let mut id = String::from("lab");
    for part in parts {
        id.push('-');
        id.push_str(&id_fragment(part));
    }
    id
}

fn id_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn layout_string(layout: &Value, key: &str) -> Option<String> {
    layout.get(key).and_then(Value::as_str).map(str::to_string)
}

fn first_viewport_id(story: &ComponentStory) -> String {
    story
        .viewports
        .first()
        .map(|viewport| viewport.id.clone())
        .unwrap_or_else(|| "desktop".to_string())
}

fn first_theme_id(story: &ComponentStory) -> String {
    story
        .themes
        .first()
        .map(|theme| theme.id.clone())
        .unwrap_or_else(|| "neutral".to_string())
}

fn first_motion_id(story: &ComponentStory) -> String {
    story
        .motions
        .first()
        .map(|motion| motion.id.clone())
        .unwrap_or_else(|| "system".to_string())
}

fn design_for_theme_preset(theme: &ThemePreset) -> Arc<DesignSystem> {
    Arc::new(DesignSystem::from_language_id(&theme.design).unwrap_or_else(DesignSystem::neutral))
}

fn clamp_f32(value: f64, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        (value as f32).clamp(min, max)
    } else {
        min
    }
}

fn story_file_name(story_id: &str) -> String {
    let mut name = String::with_capacity(story_id.len() + ".story.json".len());
    for ch in story_id.chars() {
        if ch.is_ascii_alphanumeric() {
            name.push(ch);
        } else {
            name.push('_');
        }
    }
    name.push_str(".story.json");
    name
}

fn number_step(prop_name: &str) -> f64 {
    match prop_name {
        "value" => 0.01,
        "level_db" | "peak_db" => 1.0,
        "bins" | "bars" | "points" | "size" | "slices" | "groups" | "selected" => 1.0,
        "min_freq" | "max_freq" => 10.0,
        _ => 0.1,
    }
}

fn prop_value_label(value: &StoryPropValue) -> String {
    match value {
        StoryPropValue::Bool(value) => value.to_string(),
        StoryPropValue::Number(value) => format!("{value:.2}"),
        StoryPropValue::Text(value)
        | StoryPropValue::Choice(value)
        | StoryPropValue::Color(value) => value.clone(),
    }
}

fn story_prop<'a>(story: &'a ComponentStory, name: &str) -> Option<&'a StoryPropValue> {
    story
        .props
        .iter()
        .find(|prop| prop.name == name)
        .map(|prop| &prop.value)
}

fn text_prop(story: &ComponentStory, name: &str, fallback: &str) -> String {
    match story_prop(story, name) {
        Some(StoryPropValue::Text(value)) | Some(StoryPropValue::Color(value)) => value.clone(),
        Some(StoryPropValue::Choice(value)) => value.clone(),
        _ => fallback.to_string(),
    }
}

fn choice_prop(story: &ComponentStory, name: &str, fallback: &str) -> String {
    match story_prop(story, name) {
        Some(StoryPropValue::Choice(value)) => value.clone(),
        _ => fallback.to_string(),
    }
}

fn number_prop(story: &ComponentStory, name: &str, fallback: f64) -> f64 {
    match story_prop(story, name) {
        Some(StoryPropValue::Number(value)) => *value,
        _ => fallback,
    }
}

fn bool_prop(story: &ComponentStory, name: &str, fallback: bool) -> bool {
    match story_prop(story, name) {
        Some(StoryPropValue::Bool(value)) => *value,
        _ => fallback,
    }
}

fn button_variant(value: &str) -> ButtonVariant {
    match value {
        "secondary" => ButtonVariant::Secondary,
        "destructive" => ButtonVariant::Destructive,
        "ghost" => ButtonVariant::Ghost,
        "outline" => ButtonVariant::Outline,
        _ => ButtonVariant::Primary,
    }
}

fn badge_variant(value: &str) -> BadgeVariant {
    match value {
        "primary" => BadgeVariant::Primary,
        "success" => BadgeVariant::Success,
        "warning" => BadgeVariant::Warning,
        "error" => BadgeVariant::Error,
        "info" => BadgeVariant::Info,
        _ => BadgeVariant::Default,
    }
}

fn progress_variant(value: &str) -> ProgressVariant {
    match value {
        "success" => ProgressVariant::Success,
        "warning" => ProgressVariant::Warning,
        "error" => ProgressVariant::Error,
        _ => ProgressVariant::Default,
    }
}

fn alert_variant(value: &str) -> AlertVariant {
    match value {
        "success" => AlertVariant::Success,
        "warning" => AlertVariant::Warning,
        "error" => AlertVariant::Error,
        _ => AlertVariant::Info,
    }
}

fn tab_variant(value: &str) -> TabVariant {
    match value {
        "enclosed" => TabVariant::Enclosed,
        "pills" => TabVariant::Pills,
        "vertical_card" => TabVariant::VerticalCard,
        _ => TabVariant::Underline,
    }
}

fn color_scale(value: &str) -> ColorScale {
    match value {
        "plasma" => ColorScale::Plasma,
        "inferno" => ColorScale::Inferno,
        "heat" => ColorScale::Heat,
        "coolwarm" => ColorScale::Coolwarm,
        "greys" => ColorScale::Greys,
        _ => ColorScale::Viridis,
    }
}

fn surface_colormap(value: &str) -> Colormap {
    match value {
        "plasma" => Colormap::Plasma,
        "inferno" => Colormap::Inferno,
        "turbo" => Colormap::Turbo,
        "coolwarm" => Colormap::CoolWarm,
        _ => Colormap::Viridis,
    }
}

fn tiling_method(value: &str) -> TilingMethod {
    match value {
        "binary" => TilingMethod::Binary,
        "slice" => TilingMethod::Slice,
        "dice" => TilingMethod::Dice,
        _ => TilingMethod::Squarify,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn story_file_names_are_stable() {
        assert_eq!(
            story_file_name("audio-kit.potentiometer"),
            "audio_kit_potentiometer.story.json"
        );
    }

    #[test]
    fn prop_helpers_fall_back_by_type() {
        let story =
            ComponentStory::new("test", "crate", "Title", "Description").props([StoryProp::new(
                "value",
                "Value",
                StoryPropValue::Number(0.25),
            )]);
        assert_eq!(number_prop(&story, "value", 1.0), 0.25);
        assert_eq!(text_prop(&story, "value", "fallback"), "fallback");
        assert_eq!(button_variant("outline"), ButtonVariant::Outline);
    }

    #[test]
    fn theme_presets_resolve_design_languages() {
        let apple = ThemePreset::new("apple-hig", "Apple HIG", "apple_hig", false);
        let unknown = ThemePreset::new("custom", "Custom", "missing", false);

        assert_eq!(
            design_for_theme_preset(&apple).language.as_str(),
            "apple_hig"
        );
        assert_eq!(
            design_for_theme_preset(&unknown).language.as_str(),
            "neutral"
        );
    }

    #[test]
    fn integer_story_props_use_integer_steps() {
        for prop_name in ["bars", "points", "size", "slices", "groups", "selected"] {
            assert_eq!(number_step(prop_name), 1.0);
        }
    }

    #[test]
    fn layout_state_is_serializable() {
        let mut doc = StoryDocument::new(ComponentStory::new(
            "ui-kit.button",
            "gpui-ui-kit",
            "Button",
            "Primary action button",
        ));
        doc.layout = json!({
            "viewport": "mobile",
            "theme": "neutral",
            "motion": "reduced",
            "matrix": true,
            "constraints": {
                "sizing": "fixed",
                "min_width": 420.0,
                "min_height": 260.0,
                "aspect_ratio": 1.4,
                "padding": 16.0
            },
            "builder": {
                "horizontal_align": "start",
                "vertical_align": "stretch",
                "overflow": "scroll",
                "surface": "surface",
                "gap": 12.0,
                "border": false
            }
        });
        let serialized = serde_json::to_string(&doc).unwrap();
        assert!(serialized.contains("\"matrix\":true"));
        assert!(serialized.contains("\"motion\":\"reduced\""));
        assert!(serialized.contains("\"sizing\":\"fixed\""));
        assert!(serialized.contains("\"horizontal_align\":\"start\""));
        assert!(serialized.contains("\"overflow\":\"scroll\""));
    }

    #[test]
    fn layout_constraints_parse_with_clamps() {
        let layout = json!({
            "constraints": {
                "sizing": "fit",
                "min_width": 80.0,
                "min_height": 9000.0,
                "aspect_ratio": 9.0,
                "padding": -8.0
            },
            "builder": {
                "horizontal_align": "end",
                "vertical_align": "stretch",
                "overflow": "scroll",
                "surface": "transparent",
                "gap": 900.0,
                "border": false
            }
        });
        let constraints = PreviewLayoutConstraints::from_layout(&layout);
        assert_eq!(constraints.sizing, PreviewSizing::Fit);
        assert_eq!(constraints.min_width, 160.0);
        assert_eq!(constraints.min_height, 1200.0);
        assert_eq!(constraints.aspect_ratio, 3.0);
        assert_eq!(constraints.padding, 0.0);
        assert_eq!(constraints.horizontal_align, PreviewAlign::End);
        assert_eq!(constraints.vertical_align, PreviewAlign::Stretch);
        assert_eq!(constraints.overflow, PreviewOverflow::Scroll);
        assert_eq!(constraints.surface, PreviewSurface::Transparent);
        assert_eq!(constraints.gap, 80.0);
        assert!(!constraints.border);
    }

    #[test]
    fn initial_state_uses_saved_layout_when_valid() {
        let story = ComponentStory::new("ui-kit.button", "gpui-ui-kit", "Button", "Button");
        let mut doc = StoryDocument::new(story);
        doc.layout = json!({
            "viewport": "tablet",
            "theme": "apple-hig",
            "motion": "reduced",
            "matrix": true,
            "constraints": { "sizing": "fixed", "min_width": 440.0 },
            "builder": { "horizontal_align": "start", "overflow": "visible" }
        });
        let state = InitialLabState::from_document(&doc);
        assert_eq!(state.viewport_id, "tablet");
        assert_eq!(state.theme_id, "apple-hig");
        assert_eq!(state.motion_id, "reduced");
        assert!(state.matrix_mode);
        assert_eq!(state.layout_constraints.sizing, PreviewSizing::Fixed);
        assert_eq!(state.layout_constraints.min_width, 440.0);
        assert_eq!(
            state.layout_constraints.horizontal_align,
            PreviewAlign::Start
        );
        assert_eq!(state.layout_constraints.overflow, PreviewOverflow::Visible);
    }

    #[test]
    fn px_line_story_data_is_chart_safe() {
        let sweep = line_story_data("sweep");
        assert_eq!(sweep.x.len(), sweep.y.len());
        assert!(sweep.x.iter().all(|value| *value > 0.0));
        assert_eq!(sweep.x_scale, ScaleType::Log);
        assert_eq!(
            sweep.comparison_y.as_ref().map(Vec::len),
            Some(sweep.x.len())
        );

        let flat = line_story_data("flat");
        assert_eq!(flat.x.len(), flat.y.len());
        assert!(flat.comparison_y.is_none());
    }

    #[test]
    fn px_bar_story_data_matches_categories() {
        let bars = bar_story_data(7);
        assert_eq!(bars.categories.len(), 7);
        assert_eq!(bars.values.len(), bars.categories.len());
        assert_eq!(bars.comparison_values.len(), bars.categories.len());
    }

    #[test]
    fn showcase_story_ids_map_to_sections() {
        assert_eq!(
            showcase_section_for_story_id("ui-kit.command-palette"),
            Some(ShowcaseSection::CommandPalette)
        );
        assert_eq!(
            showcase_section_for_story_id("ui-kit.accessibility"),
            Some(ShowcaseSection::Accessibility)
        );
        assert_eq!(showcase_section_for_story_id("ui-kit.button"), None);
    }

    #[test]
    fn builtin_renderer_story_ids_have_preview_handlers() {
        let missing = crate::BUILTIN_RENDERER_STORY_IDS
            .iter()
            .copied()
            .filter(|story_id| !builtin_preview_handler_story_id(story_id))
            .collect::<Vec<_>>();

        assert!(
            missing.is_empty(),
            "builtin renderer stories without preview handlers: {missing:?}"
        );
    }

    fn builtin_preview_handler_story_id(story_id: &str) -> bool {
        matches!(
            story_id,
            "ui-kit.button"
                | "ui-kit.form"
                | "ui-kit.status"
                | "ui-kit.navigation"
                | "ui-kit.feedback"
                | "ui-kit.card"
                | "audio-kit.potentiometer"
                | "audio-kit.vertical-slider"
                | "audio-kit.volume-knob"
                | "audio-kit.meter"
                | "audio-kit.horizontal-meter"
                | "audio-kit.spectrum"
                | "audio-kit.spectrum-axis"
        ) || UI_KIT_EXPORTED_COMPONENT_STORY_IDS.contains(&story_id)
            || showcase_section_for_story_id(story_id).is_some()
            || crate::PX_CHART_STORY_IDS.contains(&story_id)
    }

    #[test]
    fn surface_colormap_names_are_stable() {
        assert_eq!(surface_colormap("plasma"), Colormap::Plasma);
        assert_eq!(surface_colormap("coolwarm"), Colormap::CoolWarm);
        assert_eq!(surface_colormap("missing"), Colormap::Viridis);
    }
}

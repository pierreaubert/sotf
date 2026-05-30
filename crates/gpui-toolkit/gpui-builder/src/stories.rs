//! Layout story catalog primitives.
//!
//! Stories give examples, docs, tests, and future showcase tooling a shared way
//! to name layout trees and solve them across standard viewport scenarios.

use std::fmt;

use crate::{Axis, LayoutNode, LayoutPreferences, SolvedNode, solve};

/// One named viewport/preferences combination for a layout story.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutScenario<'a> {
    /// Stable scenario identifier, such as `desktop` or `narrow`.
    pub id: &'a str,
    /// Human-readable scenario title.
    pub title: &'a str,
    /// Viewport width in pixels.
    pub width: f32,
    /// Viewport height in pixels.
    pub height: f32,
    /// Per-slot ratio overrides for this scenario.
    pub ratios: &'a [(&'a str, Axis, f32)],
    /// Per-slot collapsed state for this scenario.
    pub collapsed: &'a [(&'a str, bool)],
}

impl<'a> LayoutScenario<'a> {
    /// Create a scenario with default layout preferences.
    pub const fn new(id: &'a str, title: &'a str, width: f32, height: f32) -> Self {
        Self {
            id,
            title,
            width,
            height,
            ratios: &[],
            collapsed: &[],
        }
    }

    /// Attach preference overrides to this scenario.
    pub const fn with_preferences(
        mut self,
        ratios: &'a [(&'a str, Axis, f32)],
        collapsed: &'a [(&'a str, bool)],
    ) -> Self {
        self.ratios = ratios;
        self.collapsed = collapsed;
        self
    }

    /// Build solver preferences for this scenario.
    pub const fn preferences(&self) -> LayoutPreferences<'a> {
        LayoutPreferences {
            ratios: self.ratios,
            collapsed: self.collapsed,
        }
    }

    /// Return a stable preference summary.
    pub fn preferences_text(&self) -> String {
        preferences_text(self.ratios, self.collapsed)
    }
}

/// A named layout tree plus the scenarios that should be shown for it.
#[derive(Debug, Clone, Copy)]
pub struct LayoutStory<'a> {
    /// Stable story identifier.
    pub id: &'a str,
    /// Human-readable story title.
    pub title: &'a str,
    /// Optional story description for docs/showcase tooling.
    pub description: Option<&'a str>,
    /// Root layout declaration.
    pub root: LayoutNode<'a>,
    /// Scenarios to solve and present for this story.
    pub scenarios: &'a [LayoutScenario<'a>],
}

impl<'a> LayoutStory<'a> {
    /// Create a layout story.
    pub const fn new(
        id: &'a str,
        title: &'a str,
        root: LayoutNode<'a>,
        scenarios: &'a [LayoutScenario<'a>],
    ) -> Self {
        Self {
            id,
            title,
            description: None,
            root,
            scenarios,
        }
    }

    /// Attach a description to this story.
    pub const fn with_description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Find a scenario by id.
    pub fn scenario(&self, id: &str) -> Option<&LayoutScenario<'a>> {
        self.scenarios.iter().find(|scenario| scenario.id == id)
    }

    /// Solve one scenario.
    pub fn solve(&self, scenario: &LayoutScenario<'a>) -> SolvedLayoutScenario {
        SolvedLayoutScenario {
            story_id: self.id.to_string(),
            story_title: self.title.to_string(),
            scenario_id: scenario.id.to_string(),
            scenario_title: scenario.title.to_string(),
            width: scenario.width,
            height: scenario.height,
            ratios: scenario
                .ratios
                .iter()
                .map(|(slot_id, axis, ratio)| StoryRatioOverride {
                    slot_id: (*slot_id).to_string(),
                    axis: *axis,
                    ratio: *ratio,
                })
                .collect(),
            collapsed: scenario
                .collapsed
                .iter()
                .map(|(slot_id, collapsed)| StoryCollapsedState {
                    slot_id: (*slot_id).to_string(),
                    collapsed: *collapsed,
                })
                .collect(),
            solved: solve(
                &self.root,
                scenario.width,
                scenario.height,
                &scenario.preferences(),
            ),
        }
    }

    /// Find and solve a scenario by id.
    pub fn solve_scenario(&self, id: &str) -> Option<SolvedLayoutScenario> {
        self.scenario(id).map(|scenario| self.solve(scenario))
    }

    /// Solve every scenario for this story.
    pub fn solve_all(&self) -> Vec<SolvedLayoutScenario> {
        self.scenarios
            .iter()
            .map(|scenario| self.solve(scenario))
            .collect()
    }
}

/// A collection of stories for docs, examples, tests, or a future showcase app.
#[derive(Debug, Clone, Copy)]
pub struct LayoutStoryCatalog<'a> {
    stories: &'a [LayoutStory<'a>],
}

impl<'a> LayoutStoryCatalog<'a> {
    /// Create a story catalog from a story slice.
    pub const fn new(stories: &'a [LayoutStory<'a>]) -> Self {
        Self { stories }
    }

    /// Return all stories in declaration order.
    pub fn stories(&self) -> &'a [LayoutStory<'a>] {
        self.stories
    }

    /// Return true when the catalog contains no stories.
    pub fn is_empty(&self) -> bool {
        self.stories.is_empty()
    }

    /// Count stories in the catalog.
    pub fn len(&self) -> usize {
        self.stories.len()
    }

    /// Find a story by id.
    pub fn find(&self, id: &str) -> Option<&'a LayoutStory<'a>> {
        self.stories.iter().find(|story| story.id == id)
    }

    /// Solve every scenario from every story.
    pub fn solve_all(&self) -> Vec<SolvedLayoutScenario> {
        self.stories
            .iter()
            .flat_map(LayoutStory::solve_all)
            .collect()
    }

    /// Render a stable line-oriented catalog index.
    pub fn to_text(&self) -> String {
        let mut output = String::from("layout story catalog:\n");
        if self.stories.is_empty() {
            output.push_str("- <empty>\n");
            return output;
        }

        for story in self.stories {
            output.push_str(&format!(
                "- {id}: {title} ({count} scenario(s))\n",
                id = story.id,
                title = story.title,
                count = story.scenarios.len()
            ));
            for scenario in story.scenarios {
                output.push_str(&format!(
                    "  - {id}: {title} {width}x{height} prefs={prefs}\n",
                    id = scenario.id,
                    title = scenario.title,
                    width = format_number(scenario.width),
                    height = format_number(scenario.height),
                    prefs = scenario.preferences_text()
                ));
            }
        }
        output
    }
}

impl fmt::Display for LayoutStoryCatalog<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text())
    }
}

/// Owned ratio override copied from a solved scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct StoryRatioOverride {
    /// Slot id whose ratio was overridden.
    pub slot_id: String,
    /// Axis for the override.
    pub axis: Axis,
    /// Ratio override value.
    pub ratio: f32,
}

/// Owned collapsed-state override copied from a solved scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct StoryCollapsedState {
    /// Slot id whose collapsed state was overridden.
    pub slot_id: String,
    /// Collapsed state.
    pub collapsed: bool,
}

/// Result of solving one story scenario.
#[derive(Debug, Clone)]
pub struct SolvedLayoutScenario {
    /// Story id.
    pub story_id: String,
    /// Story title.
    pub story_title: String,
    /// Scenario id.
    pub scenario_id: String,
    /// Scenario title.
    pub scenario_title: String,
    /// Solved viewport width.
    pub width: f32,
    /// Solved viewport height.
    pub height: f32,
    /// Ratio overrides used by the scenario.
    pub ratios: Vec<StoryRatioOverride>,
    /// Collapsed-state overrides used by the scenario.
    pub collapsed: Vec<StoryCollapsedState>,
    /// Solved layout tree.
    pub solved: SolvedNode,
}

impl SolvedLayoutScenario {
    /// Return a stable preference summary for this solved scenario.
    pub fn preferences_text(&self) -> String {
        let ratios = self
            .ratios
            .iter()
            .map(|entry| (entry.slot_id.as_str(), entry.axis, entry.ratio))
            .collect::<Vec<_>>();
        let collapsed = self
            .collapsed
            .iter()
            .map(|entry| (entry.slot_id.as_str(), entry.collapsed))
            .collect::<Vec<_>>();
        preferences_text(&ratios, &collapsed)
    }

    /// Render a stable line-oriented solved report.
    pub fn to_text(&self) -> String {
        let mut output = format!(
            "layout story solution:\nstory={story} scenario={scenario} viewport={width}x{height} prefs={prefs}\n",
            story = self.story_id,
            scenario = self.scenario_id,
            width = format_number(self.width),
            height = format_number(self.height),
            prefs = self.preferences_text()
        );
        write_solved_node(&self.solved, None, 0, &mut output);
        output
    }
}

impl fmt::Display for SolvedLayoutScenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text())
    }
}

fn write_solved_node(
    node: &SolvedNode,
    parent_path: Option<&str>,
    indent: usize,
    output: &mut String,
) {
    let path = node_path(parent_path, &node.id);
    let pad = "  ".repeat(indent);
    let status = if node.visible { "visible" } else { "collapsed" };
    output.push_str(&format!(
        "{pad}- {path} {width}x{height} {status} axis={axis} tier={tier} collapse_label={label} children={children}\n",
        width = format_number(node.width),
        height = format_number(node.height),
        axis = node.resolved_axis.map(axis_name).unwrap_or("-"),
        tier = option_text(node.active_tier.as_deref()),
        label = option_text(node.collapse_label.as_deref()),
        children = node.children.len()
    ));
    for child in &node.children {
        write_solved_node(child, Some(&path), indent + 1, output);
    }
}

fn preferences_text(ratios: &[(&str, Axis, f32)], collapsed: &[(&str, bool)]) -> String {
    if ratios.is_empty() && collapsed.is_empty() {
        return "default".to_string();
    }

    let mut parts = Vec::new();
    if !ratios.is_empty() {
        let entries = ratios
            .iter()
            .map(|(slot_id, axis, ratio)| {
                format!("{slot_id}@{}={}", axis_name(*axis), format_number(*ratio))
            })
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(format!("ratios=[{entries}]"));
    }
    if !collapsed.is_empty() {
        let entries = collapsed
            .iter()
            .map(|(slot_id, collapsed)| format!("{slot_id}={collapsed}"))
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(format!("collapsed=[{entries}]"));
    }
    parts.join(", ")
}

fn node_path(parent_path: Option<&str>, id: &str) -> String {
    let segment = if id.is_empty() { "<empty>" } else { id };
    match parent_path {
        Some(parent_path) => format!("{parent_path}/{segment}"),
        None => segment.to_string(),
    }
}

fn axis_name(axis: Axis) -> &'static str {
    match axis {
        Axis::Horizontal => "horizontal",
        Axis::Vertical => "vertical",
    }
}

fn option_text(value: Option<&str>) -> &str {
    value.filter(|value| !value.is_empty()).unwrap_or("-")
}

fn format_number(value: f32) -> String {
    if !value.is_finite() {
        return value.to_string();
    }

    let mut text = format!("{value:.2}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text == "-0" { "0".to_string() } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContainerNode, Sizing, SlotNode};

    fn sample_story<'a>(
        children: &'a [LayoutNode<'a>],
        scenarios: &'a [LayoutScenario<'a>],
    ) -> LayoutStory<'a> {
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children,
            divider_size: 4.0,
        });
        LayoutStory::new("player", "Player Layout", root, scenarios)
            .with_description("Music player shell layout")
    }

    fn sample_children<'a>() -> [LayoutNode<'a>; 2] {
        [
            LayoutNode::Slot(SlotNode {
                id: "sidebar",
                sizing: Sizing::fractional(0.25, 80.0),
                priority: 0.4,
                collapsible: true,
                display_tiers: &[],
                collapse_label: Some("Sidebar"),
            }),
            LayoutNode::Slot(SlotNode {
                id: "content",
                sizing: Sizing::flex(120.0),
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
        ]
    }

    #[test]
    fn catalog_lookup_and_text_are_stable() {
        let children = sample_children();
        let ratios = [("sidebar", Axis::Horizontal, 0.4)];
        let collapsed = [("sidebar", true)];
        let scenarios = [
            LayoutScenario::new("desktop", "Desktop", 300.0, 160.0),
            LayoutScenario::new("custom", "Custom", 300.0, 160.0)
                .with_preferences(&ratios, &collapsed),
        ];
        let story = sample_story(&children, &scenarios);
        let stories = [story];
        let catalog = LayoutStoryCatalog::new(&stories);

        assert_eq!(catalog.len(), 1);
        assert!(!catalog.is_empty());
        assert_eq!(
            catalog
                .find("player")
                .unwrap()
                .scenario("custom")
                .unwrap()
                .title,
            "Custom"
        );
        assert!(catalog.find("missing").is_none());
        assert_eq!(
            catalog.to_text(),
            concat!(
                "layout story catalog:\n",
                "- player: Player Layout (2 scenario(s))\n",
                "  - desktop: Desktop 300x160 prefs=default\n",
                "  - custom: Custom 300x160 prefs=ratios=[sidebar@horizontal=0.4], ",
                "collapsed=[sidebar=true]\n",
            )
        );
    }

    #[test]
    fn solves_story_scenarios() {
        let children = sample_children();
        let collapsed = [("sidebar", true)];
        let scenarios = [
            LayoutScenario::new("desktop", "Desktop", 300.0, 160.0),
            LayoutScenario::new("collapsed", "Collapsed", 300.0, 160.0)
                .with_preferences(&[], &collapsed),
        ];
        let story = sample_story(&children, &scenarios);

        let desktop = story.solve_scenario("desktop").unwrap();
        assert_eq!(desktop.solved.find("sidebar").unwrap().width, 80.0);
        assert!(desktop.solved.find("sidebar").unwrap().visible);

        let collapsed = story.solve_scenario("collapsed").unwrap();
        assert!(!collapsed.solved.find("sidebar").unwrap().visible);
        assert_eq!(collapsed.solved.find("content").unwrap().width, 300.0);
        assert!(story.solve_scenario("missing").is_none());
    }

    #[test]
    fn catalog_solves_all_scenarios() {
        let children = sample_children();
        let scenarios = [
            LayoutScenario::new("desktop", "Desktop", 300.0, 160.0),
            LayoutScenario::new("narrow", "Narrow", 180.0, 160.0),
        ];
        let story = sample_story(&children, &scenarios);
        let stories = [story];
        let catalog = LayoutStoryCatalog::new(&stories);

        let solved = catalog.solve_all();

        assert_eq!(solved.len(), 2);
        assert_eq!(solved[0].story_id, "player");
        assert_eq!(solved[1].scenario_id, "narrow");
    }

    #[test]
    fn solved_story_text_is_stable() {
        let children = sample_children();
        let scenarios = [LayoutScenario::new("desktop", "Desktop", 300.0, 160.0)];
        let story = sample_story(&children, &scenarios);

        let solved = story.solve_scenario("desktop").unwrap();

        assert_eq!(
            solved.to_text(),
            concat!(
                "layout story solution:\n",
                "story=player scenario=desktop viewport=300x160 prefs=default\n",
                "- root 300x160 visible axis=horizontal tier=- collapse_label=- children=2\n",
                "  - root/sidebar 80x160 visible axis=- tier=- collapse_label=Sidebar children=0\n",
                "  - root/content 216x160 visible axis=- tier=- collapse_label=- children=0\n",
            )
        );
    }
}

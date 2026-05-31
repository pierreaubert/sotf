//! Visual regression manifest helpers for layout stories.
//!
//! The builder crate does not capture pixels itself; this module produces a
//! stable manifest that screenshot runners can consume.

use crate::{LayoutStoryCatalog, SolvedLayoutScenario};

/// Color scheme requested by a visual regression capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualColorScheme {
    Light,
    Dark,
    HighContrast,
}

impl VisualColorScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::HighContrast => "high_contrast",
        }
    }
}

/// One screenshot capture case.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualRegressionCase {
    pub id: String,
    pub story_id: String,
    pub scenario_id: String,
    pub width: f32,
    pub height: f32,
    pub color_scheme: VisualColorScheme,
    pub solved_text: String,
}

impl VisualRegressionCase {
    pub fn from_solved(
        solved: &SolvedLayoutScenario,
        color_scheme: VisualColorScheme,
    ) -> VisualRegressionCase {
        let id = capture_id(&solved.story_id, &solved.scenario_id, color_scheme);
        Self {
            id,
            story_id: solved.story_id.clone(),
            scenario_id: solved.scenario_id.clone(),
            width: solved.width,
            height: solved.height,
            color_scheme,
            solved_text: solved.to_text(),
        }
    }
}

/// A stable list of screenshot captures.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VisualRegressionManifest {
    pub cases: Vec<VisualRegressionCase>,
}

impl VisualRegressionManifest {
    pub fn from_catalog(
        catalog: &LayoutStoryCatalog<'_>,
        color_schemes: &[VisualColorScheme],
    ) -> Self {
        let schemes = if color_schemes.is_empty() {
            &[VisualColorScheme::Dark][..]
        } else {
            color_schemes
        };

        let mut cases = Vec::new();
        for solved in catalog.solve_all() {
            for &scheme in schemes {
                cases.push(VisualRegressionCase::from_solved(&solved, scheme));
            }
        }
        cases.sort_by(|a, b| a.id.cmp(&b.id));
        Self { cases }
    }

    pub fn to_markdown_table(&self) -> String {
        let mut output = String::from(
            "| capture | story | scenario | size | scheme |\n\
             | --- | --- | --- | ---: | --- |\n",
        );
        for case in &self.cases {
            output.push_str(&format!(
                "| {} | {} | {} | {}x{} | {} |\n",
                case.id,
                case.story_id,
                case.scenario_id,
                format_number(case.width),
                format_number(case.height),
                case.color_scheme.as_str()
            ));
        }
        output
    }
}

fn capture_id(story_id: &str, scenario_id: &str, color_scheme: VisualColorScheme) -> String {
    format!("{story_id}__{scenario_id}__{}", color_scheme.as_str())
}

fn format_number(value: f32) -> String {
    let mut text = format!("{value:.2}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Axis, ContainerNode, LayoutNode, LayoutScenario, LayoutStory, Sizing, SlotNode};

    #[test]
    fn manifest_expands_story_scenarios_across_color_schemes() {
        let children = [LayoutNode::Slot(SlotNode {
            id: "content",
            sizing: Sizing::flex(120.0),
            priority: 1.0,
            collapsible: false,
            display_tiers: &[],
            collapse_label: None,
        })];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Vertical,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 0.0,
        });
        let scenarios = [LayoutScenario::new("phone", "Phone", 390.0, 844.0)];
        let story = LayoutStory::new("player", "Player", root, &scenarios);
        let stories = [story];
        let catalog = LayoutStoryCatalog::new(&stories);

        let manifest = VisualRegressionManifest::from_catalog(
            &catalog,
            &[VisualColorScheme::Light, VisualColorScheme::Dark],
        );

        assert_eq!(manifest.cases.len(), 2);
        assert_eq!(manifest.cases[0].id, "player__phone__dark");
        assert!(manifest.to_markdown_table().contains("390x844"));
    }
}

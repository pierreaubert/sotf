//! Plugin chassis layout descriptors.
//!
//! Owned, platform-agnostic data structures describing the visual structure
//! of an audio plugin's UI chassis: a header strip, a horizontal row of
//! sections (each with an eyebrow / title / caption + a stack of rows), and
//! an optional footer.
//!
//! These descriptors are consumed by the app-gpui renderer to paint the
//! plugin UI using the active `PluginTheme`. The renderer walks
//! [`ChassisLayout::sections`] in order and uses [`ChassisLayout::solve`] to
//! determine which sections fit at the available width and which collapse.
//!
//! No rendering code lives here. No GPUI dependencies.

use crate::compat::KnobSize;

// ============================================================================
// Header / Footer / Section / Row / Knob descriptors
// ============================================================================

/// Header strip at the top of a plugin chassis.
#[derive(Debug, Clone)]
pub struct HeaderSpec {
    /// Small caps mark in the top-left (e.g. "SOTF · Module 07").
    pub brand_mark: String,
    /// Italic display title in the center (e.g. "Loudness").
    pub title: String,
    /// Caption below the title (e.g. "Equal-Loudness Compensation · ISO 226:2003").
    pub subtitle: String,
}

/// Footer strip at the bottom of a plugin chassis.
#[derive(Debug, Clone)]
pub struct FooterSpec {
    /// Tick segments on the left (sample rate, latency, CPU, etc.).
    pub ticks: Vec<String>,
    /// Build / serial readout on the right.
    pub serial: String,
}

/// One numbered section of the chassis (e.g. "01 · Reference").
#[derive(Debug, Clone)]
pub struct SectionSpec {
    /// Stable id used by the renderer to look up the section's layout slot.
    pub id: String,
    /// Eyebrow label shown before the title (e.g. "01").
    pub eyebrow: String,
    /// Italic display title (e.g. "Reference", "Low", "Output").
    pub title: String,
    /// Optional caption right-aligned in the section header (e.g. "SPL", "band 1").
    pub caption: Option<String>,
    /// Vertical stack of rows inside the section.
    pub rows: Vec<RowSpec>,
    /// Minimum width at which the section is rendered. Below this, the
    /// section collapses to a tab.
    pub min_width: f32,
    /// Preferred width when there is enough space; the section grows up to
    /// this value before flexing.
    pub preferred_width: f32,
    /// Collapse priority — lowest values collapse first when space is tight.
    /// 1.0 = never collapses. The Main / wide section should be 1.0.
    pub priority: f32,
}

/// A row inside a section.
#[derive(Debug, Clone)]
pub enum RowSpec {
    /// A horizontal row of knobs.
    KnobRow { id: String, knobs: Vec<KnobSlot> },
    /// A horizontal "label + toggle pip" row used as a band-enable header
    /// (e.g. "Mid Enabled  [Off | On]").
    BandToggle {
        id: String,
        label: String,
        /// `true` shows an actual toggle pip; `false` shows just an LED dot
        /// (used for "always on" indicators).
        has_toggle: bool,
    },
    /// A read-only value tile (e.g. "Playback Volume   0.0 dB"). Used for
    /// signal-tracking values that the user cannot set directly.
    ReadoutTile { id: String, label: String },
    /// A standalone toggle group with its own label above ("Auto Gain").
    ToggleGroup { id: String, label: String },
}

impl RowSpec {
    pub fn id(&self) -> &str {
        match self {
            RowSpec::KnobRow { id, .. }
            | RowSpec::BandToggle { id, .. }
            | RowSpec::ReadoutTile { id, .. }
            | RowSpec::ToggleGroup { id, .. } => id,
        }
    }
}

/// A single knob slot inside a [`RowSpec::KnobRow`].
#[derive(Debug, Clone)]
pub struct KnobSlot {
    /// Stable id (e.g. "freq", "gain").
    pub id: String,
    /// Index into the plugin's parameter list — the renderer uses this to
    /// look up the live value and bind drag handlers.
    pub param_idx: usize,
    /// Visible label below the knob (e.g. "Frequency").
    pub label: String,
    /// Knob size tier — `KnobSize` is reused from `gpui_builder::compat`.
    pub size: KnobSize,
    /// `true` for ± gain knobs that draw outward from 12 o'clock.
    pub bipolar: bool,
}

// ============================================================================
// ChassisLayout — the root descriptor
// ============================================================================

/// Complete chassis layout for one plugin.
#[derive(Debug, Clone)]
pub struct ChassisLayout {
    pub header: HeaderSpec,
    pub sections: Vec<SectionSpec>,
    pub footer: Option<FooterSpec>,
}

impl ChassisLayout {
    /// Build a new chassis with the given header and sections (no footer).
    pub fn new(header: HeaderSpec, sections: Vec<SectionSpec>) -> Self {
        Self {
            header,
            sections,
            footer: None,
        }
    }

    /// Attach a footer strip.
    pub fn with_footer(mut self, footer: FooterSpec) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Look up a section by id.
    pub fn section(&self, id: &str) -> Option<&SectionSpec> {
        self.sections.iter().find(|s| s.id == id)
    }

    /// Solve the section row for the given available width.
    ///
    /// Returns one [`SolvedSection`] per input section, in input order. Each
    /// solved section has a final `width` (≥ 0) and a `visible` flag. When
    /// there is not enough space, sections collapse in ascending `priority`
    /// order — lowest first.
    ///
    /// The algorithm:
    /// 1. Sum `min_width` across all sections.
    /// 2. While the sum exceeds `available_width`, drop the section with
    ///    the lowest `priority` (mark `visible = false`, contributes 0
    ///    width) and repeat.
    /// 3. Distribute remaining space proportionally to `preferred_width -
    ///    min_width` of visible sections (clamped at preferred_width).
    /// 4. Any leftover space flexes the highest-priority visible section.
    pub fn solve(&self, available_width: f32) -> SolvedChassis {
        let n = self.sections.len();
        let mut visible = vec![true; n];
        let mut widths = vec![0.0_f32; n];

        if n == 0 {
            return SolvedChassis {
                sections: vec![],
                total_width: 0.0,
            };
        }

        // Step 1+2: collapse lowest-priority sections until min-sum fits.
        loop {
            let min_sum: f32 = self
                .sections
                .iter()
                .zip(visible.iter())
                .filter(|(_, v)| **v)
                .map(|(s, _)| s.min_width)
                .sum();

            if min_sum <= available_width {
                break;
            }

            // Find the visible section with the lowest priority. If multiple
            // tie, drop the rightmost (later in input order) — this keeps
            // earlier "primary" sections visible longer.
            let drop_idx = (0..n).filter(|i| visible[*i]).min_by(|a, b| {
                self.sections[*a]
                    .priority
                    .partial_cmp(&self.sections[*b].priority)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.cmp(a)) // tie-break: later index drops first
            });

            match drop_idx {
                Some(i) => visible[i] = false,
                None => break, // nothing left to drop
            }

            // Defensive: priority 1.0 sections should never be dropped. If
            // every visible section has priority >= 1.0 we exit the loop —
            // the chassis simply doesn't fit. We accept clipping rather
            // than dropping a never-collapse section.
            if visible
                .iter()
                .zip(self.sections.iter())
                .filter(|(v, _)| **v)
                .all(|(_, s)| s.priority >= 1.0)
            {
                break;
            }
        }

        // Step 3+4: distribute width across visible sections.
        let visible_indices: Vec<usize> = (0..n).filter(|i| visible[*i]).collect();
        let min_sum: f32 = visible_indices
            .iter()
            .map(|&i| self.sections[i].min_width)
            .sum();
        let preferred_extra: f32 = visible_indices
            .iter()
            .map(|&i| (self.sections[i].preferred_width - self.sections[i].min_width).max(0.0))
            .sum();

        let extra_space = (available_width - min_sum).max(0.0);

        if preferred_extra <= 0.0 || visible_indices.is_empty() {
            // No section wants more than its min — give everyone min, leftover
            // accumulates on the highest-priority section if there is one.
            for &i in &visible_indices {
                widths[i] = self.sections[i].min_width;
            }
        } else {
            let factor = (extra_space / preferred_extra).min(1.0);
            for &i in &visible_indices {
                let span = (self.sections[i].preferred_width - self.sections[i].min_width).max(0.0);
                widths[i] = self.sections[i].min_width + span * factor;
            }

            // Distribute any leftover (when extra_space > preferred_extra) to
            // the highest-priority visible section.
            let allocated: f32 = widths.iter().sum();
            let leftover = available_width - allocated;
            if leftover > 0.0
                && let Some(&i) = visible_indices.iter().max_by(|a, b| {
                    self.sections[**a]
                        .priority
                        .partial_cmp(&self.sections[**b].priority)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            {
                widths[i] += leftover;
            }
        }

        let solved_sections: Vec<SolvedSection> = (0..n)
            .map(|i| SolvedSection {
                id: self.sections[i].id.clone(),
                width: widths[i],
                visible: visible[i],
            })
            .collect();

        let total_width: f32 = solved_sections
            .iter()
            .filter(|s| s.visible)
            .map(|s| s.width)
            .sum();

        SolvedChassis {
            sections: solved_sections,
            total_width,
        }
    }
}

// ============================================================================
// Solved output
// ============================================================================

/// Result of solving a [`ChassisLayout`] at a specific available width.
#[derive(Debug, Clone)]
pub struct SolvedChassis {
    /// One entry per input section, in input order.
    pub sections: Vec<SolvedSection>,
    /// Sum of widths of visible sections.
    pub total_width: f32,
}

impl SolvedChassis {
    /// Look up a solved section by id.
    pub fn section(&self, id: &str) -> Option<&SolvedSection> {
        self.sections.iter().find(|s| s.id == id)
    }

    /// Iterate visible sections in input order.
    pub fn visible(&self) -> impl Iterator<Item = &SolvedSection> {
        self.sections.iter().filter(|s| s.visible)
    }
}

#[derive(Debug, Clone)]
pub struct SolvedSection {
    pub id: String,
    pub width: f32,
    pub visible: bool,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn knob(id: &str, param_idx: usize, label: &str) -> KnobSlot {
        KnobSlot {
            id: id.to_string(),
            param_idx,
            label: label.to_string(),
            size: KnobSize::Sm,
            bipolar: false,
        }
    }

    fn section(id: &str, title: &str, min: f32, pref: f32, priority: f32) -> SectionSpec {
        SectionSpec {
            id: id.to_string(),
            eyebrow: "01".to_string(),
            title: title.to_string(),
            caption: None,
            rows: vec![],
            min_width: min,
            preferred_width: pref,
            priority,
        }
    }

    fn loudness_chassis() -> ChassisLayout {
        ChassisLayout::new(
            HeaderSpec {
                brand_mark: "SOTF · Module 07".to_string(),
                title: "Loudness".to_string(),
                subtitle: "Equal-Loudness Compensation".to_string(),
            },
            vec![
                section("reference", "Reference", 240.0, 280.0, 0.9),
                section("low", "Low", 220.0, 240.0, 0.8),
                section("mid", "Mid", 280.0, 360.0, 1.0), // primary — never collapses
                section("high", "High", 220.0, 240.0, 0.7),
                section("output", "Output", 220.0, 240.0, 0.6),
            ],
        )
    }

    #[test]
    fn lookup_section_by_id() {
        let chassis = loudness_chassis();
        assert_eq!(chassis.section("mid").unwrap().title, "Mid");
        assert!(chassis.section("nonexistent").is_none());
    }

    #[test]
    fn wide_chassis_all_visible() {
        let solved = loudness_chassis().solve(1500.0);
        assert!(solved.sections.iter().all(|s| s.visible));
        assert_eq!(solved.visible().count(), 5);
        // Total width must not exceed available.
        assert!(solved.total_width <= 1500.0 + 0.01);
    }

    #[test]
    fn wide_chassis_uses_full_width() {
        let solved = loudness_chassis().solve(1500.0);
        // With plenty of room, every section gets at least its preferred width
        // and the leftover lands on the highest-priority section ("mid").
        let mid = solved.section("mid").unwrap();
        assert!(
            mid.width >= 360.0,
            "mid should reach at least preferred (360), got {}",
            mid.width
        );
        // The total should equal available (no wasted space).
        assert!((solved.total_width - 1500.0).abs() < 0.5);
    }

    #[test]
    fn narrow_chassis_drops_lowest_priority() {
        // Sum of min widths = 240+220+280+220+220 = 1180.
        // At 1000 available, we must drop one section. Lowest priority is
        // "output" (0.6).
        let solved = loudness_chassis().solve(1000.0);
        assert!(!solved.section("output").unwrap().visible);
        assert!(solved.section("mid").unwrap().visible);
        assert!(solved.section("reference").unwrap().visible);
    }

    #[test]
    fn very_narrow_drops_in_priority_order() {
        // Drop one at a time from lowest priority up.
        // priorities: mid 1.0, reference 0.9, low 0.8, high 0.7, output 0.6
        // 1180 → drop output (0.6) → 960
        // 960 → drop high (0.7) → 740
        // 740 → drop low (0.8) → 520
        // 520 → drop reference (0.9) → 280
        let solved = loudness_chassis().solve(300.0);
        assert!(solved.section("mid").unwrap().visible);
        assert!(!solved.section("reference").unwrap().visible);
        assert!(!solved.section("low").unwrap().visible);
        assert!(!solved.section("high").unwrap().visible);
        assert!(!solved.section("output").unwrap().visible);
    }

    #[test]
    fn primary_section_never_dropped_even_if_too_narrow() {
        // Available width below mid's min_width (280). Mid (priority 1.0)
        // must remain visible — clipping is preferred over dropping a
        // never-collapse section.
        let solved = loudness_chassis().solve(100.0);
        assert!(solved.section("mid").unwrap().visible);
    }

    #[test]
    fn empty_chassis_solves_to_empty() {
        let chassis = ChassisLayout::new(
            HeaderSpec {
                brand_mark: String::new(),
                title: String::new(),
                subtitle: String::new(),
            },
            vec![],
        );
        let solved = chassis.solve(800.0);
        assert!(solved.sections.is_empty());
        assert_eq!(solved.total_width, 0.0);
    }

    #[test]
    fn knob_row_holds_param_indices() {
        let row = RowSpec::KnobRow {
            id: "freq-gain".to_string(),
            knobs: vec![knob("freq", 0, "Frequency"), knob("gain", 1, "Gain")],
        };
        match row {
            RowSpec::KnobRow { knobs, .. } => {
                assert_eq!(knobs.len(), 2);
                assert_eq!(knobs[0].param_idx, 0);
                assert_eq!(knobs[1].param_idx, 1);
            }
            _ => panic!("expected KnobRow"),
        }
    }

    #[test]
    fn row_id_accessor_works_for_each_variant() {
        assert_eq!(
            RowSpec::KnobRow {
                id: "kr".to_string(),
                knobs: vec![]
            }
            .id(),
            "kr"
        );
        assert_eq!(
            RowSpec::BandToggle {
                id: "bt".to_string(),
                label: "x".to_string(),
                has_toggle: true,
            }
            .id(),
            "bt"
        );
        assert_eq!(
            RowSpec::ReadoutTile {
                id: "rt".to_string(),
                label: "x".to_string(),
            }
            .id(),
            "rt"
        );
        assert_eq!(
            RowSpec::ToggleGroup {
                id: "tg".to_string(),
                label: "x".to_string(),
            }
            .id(),
            "tg"
        );
    }

    #[test]
    fn total_width_never_exceeds_available() {
        let chassis = loudness_chassis();
        for w in [200.0, 400.0, 800.0, 1000.0, 1500.0, 2400.0_f32] {
            let solved = chassis.solve(w);
            // Total of visible widths must fit in available unless a
            // priority-1.0 section has been clipped (when w < mid's min_width).
            if w >= 280.0 {
                assert!(
                    solved.total_width <= w + 0.5,
                    "width={w}: total={} exceeded",
                    solved.total_width
                );
            }
        }
    }

    #[test]
    fn ties_drop_later_index_first() {
        // Two sections with the same priority — the later one should drop first.
        let chassis = ChassisLayout::new(
            HeaderSpec {
                brand_mark: String::new(),
                title: String::new(),
                subtitle: String::new(),
            },
            vec![
                section("a", "A", 200.0, 200.0, 0.5),
                section("b", "B", 200.0, 200.0, 1.0), // primary
                section("c", "C", 200.0, 200.0, 0.5),
            ],
        );
        // Total min = 600; available 400 forces one drop.
        let solved = chassis.solve(400.0);
        assert!(solved.section("a").unwrap().visible);
        assert!(solved.section("b").unwrap().visible);
        assert!(!solved.section("c").unwrap().visible);
    }
}

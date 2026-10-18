//! Lay out fixed-geometry charts inside their measured grid cells.

use gpui::prelude::*;
use gpui::{AnyElement, AvailableSpace, ElementId, canvas, div, px};

pub(crate) fn chart_canvas(
    id: impl Into<ElementId>,
    height: f32,
    build: impl FnOnce(f32) -> AnyElement + 'static,
) -> AnyElement {
    // The stable parent scopes gpui-px's series and legend IDs per plot.
    // Without it, retained renderer state can be shared by unrelated charts.
    div()
        .id(id)
        .w_full()
        .min_w_0()
        .child(
            canvas(
                move |bounds, window, cx| {
                    let mut chart = build(f32::from(bounds.size.width).max(1.0));
                    chart.layout_as_root(bounds.size.map(AvailableSpace::Definite), window, cx);
                    chart.prepaint_at(bounds.origin, window, cx);
                    chart
                },
                |_, mut chart, window, cx| chart.paint(window, cx),
            )
            .w_full()
            // intentional: chart canvas height is determined by the plot layout.
            .h(px(height)),
        )
        .into_any_element()
}

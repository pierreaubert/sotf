//! RoomEQ plots must fit their cells and retain independent renderer state.

#[path = "../components/room_eq/render/chart_canvas.rs"]
mod chart_canvas;

use gpui::{
    Context, IntoElement, ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window,
    canvas, div, px,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct PlotGrid {
    width: Rc<Cell<f32>>,
    observed: Rc<RefCell<Vec<(f32, f64)>>>,
}

impl Render for PlotGrid {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let columns = if self.width.get() >= 600.0 { 3 } else { 1 };
        div()
            .w(px(self.width.get()))
            .grid()
            .grid_cols(columns)
            .gap(px(12.0))
            .children(
                [("original", -42.0), ("eq", 3.0), ("impulse", 0.1)]
                    .into_iter()
                    .map(|(id, value)| {
                        let observed = self.observed.clone();
                        chart_canvas::chart_canvas(id, 100.0, move |width| {
                            canvas(
                                move |_, window, cx| {
                                    // Match gpui-px: every plot uses the same local series ID.
                                    let state =
                                        window.use_keyed_state("px-line-primary", cx, |_, _| value);
                                    observed.borrow_mut().push((width, *state.read(cx)));
                                },
                                |_, _, _, _| {},
                            )
                            .w(px(width))
                            .h(px(100.0))
                            .into_any_element()
                        })
                    }),
            )
    }
}

#[gpui::test]
fn plots_fit_measured_cells_and_do_not_share_retained_data(cx: &mut TestAppContext) {
    let width = Rc::new(Cell::new(600.0));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let window = cx.add_window({
        let width = width.clone();
        let observed = observed.clone();
        move |_, _| PlotGrid { width, observed }
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    assert_eq!(
        &*observed.borrow(),
        &[(192.0, -42.0), (192.0, 3.0), (192.0, 0.1)]
    );

    // Rebuild at a narrow width, with the same retained plot identities.
    observed.borrow_mut().clear();
    width.set(240.0);
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    assert_eq!(
        &*observed.borrow(),
        &[(240.0, -42.0), (240.0, 3.0), (240.0, 0.1)]
    );
}

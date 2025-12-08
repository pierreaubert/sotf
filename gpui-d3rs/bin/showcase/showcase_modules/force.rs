use gpui::*;
use crate::ShowcaseApp;

pub fn render(app: &ShowcaseApp, cx: &mut Context<ShowcaseApp>) -> Div {
    
    // START SHORTCUT: Since we can't easily change the signature everywhere right now without refactoring main.rs dispatch,
    // let's assume the loop is started in main.rs or we handle it differently.
    // Actually, I will modify main.rs to pass &mut ShowcaseApp to render_content modules.
    // Wait, main.rs calls `render_content(&mut self, ...)` and then matches on `self.current_section`.
    // The modules signatures in other files currently take `&ShowcaseApp`.
    // I should check `overview.rs` signature.
    
    // Inspecting `overview::render`: `pub fn render(app: &ShowcaseApp) -> Div`
    // So I need to update all signatures if I want to pass mutable app.
    // OR I can use Interior Mutability for the simulation if strictly necessary, but `ShowcaseApp` owns it.
    
    // Better plan: Initialize the loop in `ShowcaseApp::new` if it's meant to run always, or handle the specific Force tick in `main.rs` via `cx.spawn`.
    // Given `force_demo.rs` experience, `Simulation` is lightweight.
    
    // For now, let's just render the nodes.
    
    let mut elements = Vec::new();
    
    for node_rc in &app.force_simulation.nodes {
        let n = node_rc.borrow();
        elements.push(
            div()
                .absolute()
                .left(px(n.x as f32 - 5.0))
                .top(px(n.y as f32 - 5.0))
                .size(px(10.0))
                .bg(rgb(0xff4444))
                .rounded_full()
        );
    }
    
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .child("Force Directed Graph"),
        )
         .child(
            div()
                .text_sm()
                .text_color(rgb(0x666666))
                .child("Nodes repel each other and are attracted to the center."),
        )
        .child(
            div()
                .w(px(800.0))
                .h(px(600.0))
                .bg(rgb(0xf0f0f0))
                .border_1()
                .border_color(rgb(0xcccccc))
                .relative()
                .overflow_hidden()
                .children(elements)
        )
}

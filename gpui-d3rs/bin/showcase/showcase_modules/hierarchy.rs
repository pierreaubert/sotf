use d3rs::hierarchy::{HierarchyNode, TreeLayout};
use gpui::*;
use crate::ShowcaseApp;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    // Recreate hierarchy for stateless render
    // In a real app, this would be computed once and stored in app state
    
    let root = HierarchyNode::new("Root".to_string());
    
    let child_a = HierarchyNode::new("Child A".to_string());
    let child_b = HierarchyNode::new("Child B".to_string());
    
    let grandchild_a1 = HierarchyNode::new("GC A1".to_string());
    let grandchild_a2 = HierarchyNode::new("GC A2".to_string());
    
    let grandchild_b1 = HierarchyNode::new("GC B1".to_string());
    let grandchild_b2 = HierarchyNode::new("GC B2".to_string());
    
    {
        let mut a = child_a.borrow_mut();
        a.set_children(&child_a, vec![grandchild_a1, grandchild_a2]);
    }
    
    {
        let mut b = child_b.borrow_mut();
        b.set_children(&child_b, vec![grandchild_b1, grandchild_b2]);
    }
    
    {
        let mut r = root.borrow_mut();
        r.set_children(&root, vec![child_a, child_b]);
    }
    
    HierarchyNode::count(root.clone());
    
    let width = 600.0;
    let height = 400.0;
    
    // Layout
    TreeLayout::new()
        .size((width - 100.0, height - 100.0))
        .layout(root.clone());

    let mut nodes = Vec::new();
    let mut links = Vec::new();

    HierarchyNode::each(root.clone(), |node| {
        let n = node.borrow();
        
        if let Some(parent_weak) = &n.parent {
            if let Some(parent_rc) = parent_weak.upgrade() {
                let p = parent_rc.borrow();
                
                let x1 = n.x as f32 + 50.0;
                let y1 = n.y as f32 + 50.0;
                let x2 = p.x as f32 + 50.0;
                let y2 = p.y as f32 + 50.0;

                links.push(
                    div()
                       .absolute()
                       .size_full()
                       .child(
                           svg()
                            .size_full()
                            .path(format!("M {},{} L {},{}", x2, y2, x1, y1))
                            .text_color(rgb(0xaaaaaa))
                       )
                );
            }
        }
        
        nodes.push(
            div()
                .absolute()
                .left(px(n.x as f32 + 50.0 - 20.0))
                .top(px(n.y as f32 + 50.0 - 20.0))
                .size(px(40.0))
                .bg(rgb(0x4a90e2))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(rgb(0xffffff))
                .child(n.data.clone())
        );
    });

    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .child("Hierarchy Layout (Tree)"),
        )
        .child(
            div()
                .w(px(width as f32))
                .h(px(height as f32))
                .bg(rgb(0xf0f0f0))
                .rounded_lg()
                .border_1()
                .border_color(rgb(0xcccccc))
                .relative()
                .children(links)
                .children(nodes)
        )
}

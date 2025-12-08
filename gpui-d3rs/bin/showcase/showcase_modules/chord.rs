use d3rs::chord::{ChordLayout, RibbonGenerator};
use gpui::*;
use crate::ShowcaseApp;
use std::f64::consts::PI;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
     let matrix = vec![
        vec![11975.0, 5871.0, 8916.0, 2868.0],
        vec![1951.0, 10048.0, 2060.0, 6171.0],
        vec![8010.0, 16145.0, 8090.0, 8045.0],
        vec![1013.0, 990.0, 940.0, 6907.0],
    ];

    let layout = ChordLayout::new().pad_angle(0.05);
    let chords = layout.compute(&matrix);
    
    let outer_radius = 200.0;
    let inner_radius = 180.0;
    let ribbon = RibbonGenerator::new(inner_radius);
    
    let colors = vec![
        rgb(0x000000), 
        rgb(0xffdd89), 
        rgb(0x957244), 
        rgb(0xf26223)
    ];
    
    let width = 600.0;
    let height = 600.0;
    let center_x = width / 2.0;
    let center_y = height / 2.0;

    // Arcs
    use d3rs::shape::arc::{Arc, ArcDatum};
    let arc_gen = Arc::new();
    
    let mut elements = Vec::new();
    
    for group in &chords.groups {
        let datum = ArcDatum::new()
            .inner_radius(inner_radius)
            .outer_radius(outer_radius)
            .start_angle(group.start_angle - PI/2.0)
            .end_angle(group.end_angle - PI/2.0);
            
        let path = arc_gen.generate(&datum);
        
        elements.push(
            div()
                .absolute()
                .left(px(center_x as f32))
                .top(px(center_y as f32))
                .child(
                    svg()
                        .path(path.to_svg_string())
                        .text_color(colors[group.index % colors.len()])
                )
        );
    }
    
    // Ribbons
    for chord in &chords.chords {
        let path_d = ribbon.generate(chord);
        
         elements.push(
            div()
                .absolute()
                .left(px(center_x as f32))
                .top(px(center_y as f32))
                .child(
                    svg()
                        .path(path_d)
                        .text_color(colors[chord.target.index % colors.len()])
                        .opacity(0.67)
                )
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
                .child("Chord Diagram"),
        )
        .child(
             div()
                .w(px(width as f32))
                .h(px(height as f32))
                .bg(rgb(0xffffff))
                .relative()
                .children(elements)
        )
}

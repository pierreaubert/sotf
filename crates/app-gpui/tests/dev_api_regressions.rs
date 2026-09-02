#![cfg(feature = "dev-api")]

use gpui::{Bounds, point, px, size};
use sotf_audio_player_gpui::app::dev_api::{merge_missing_tracked_elements, registry};

#[test]
fn tracked_elements_are_isolated_by_window() {
    let live_window = u64::MAX - 1;
    let capture_window = u64::MAX;
    registry::clear(live_window);
    registry::clear(capture_window);
    let bounds = Bounds {
        origin: point(px(10.0), px(20.0)),
        size: size(px(30.0), px(40.0)),
    };

    registry::record(live_window, "phone.tab.Home", bounds);

    assert_eq!(
        registry::lookup(live_window, "phone.tab.Home"),
        Some(bounds)
    );
    assert_eq!(registry::lookup(capture_window, "phone.tab.Home"), None);
    assert_eq!(registry::snapshot_for(live_window).len(), 1);
    assert!(registry::snapshot_for(capture_window).is_empty());
}

#[test]
fn merge_restores_only_elements_missing_from_refreshed_scene() {
    let mut initial = image::RgbaImage::from_pixel(20, 20, image::Rgba([0, 0, 0, 255]));
    let mut refreshed = initial.clone();
    for y in 4..10 {
        for x in 2..8 {
            initial.put_pixel(x, y, image::Rgba([255, 32, 16, 255]));
        }
    }
    let home = (
        "settings.language.fr".to_string(),
        registry::TrackedElement {
            bounds: Bounds {
                origin: point(px(1.0), px(2.0)),
                size: size(px(3.0), px(3.0)),
            },
            state: registry::DevElementState::default(),
        },
    );

    merge_missing_tracked_elements(
        &mut refreshed,
        &initial,
        size(px(10.0), px(10.0)),
        std::slice::from_ref(&home),
        &[],
    );

    assert_eq!(*refreshed.get_pixel(2, 4), image::Rgba([255, 32, 16, 255]));
    assert_eq!(*refreshed.get_pixel(8, 10), image::Rgba([0, 0, 0, 255]));

    let mut already_present = image::RgbaImage::from_pixel(20, 20, image::Rgba([0, 0, 0, 255]));
    merge_missing_tracked_elements(
        &mut already_present,
        &initial,
        size(px(10.0), px(10.0)),
        std::slice::from_ref(&home),
        std::slice::from_ref(&home),
    );
    assert_eq!(
        *already_present.get_pixel(2, 4),
        image::Rgba([0, 0, 0, 255])
    );
}

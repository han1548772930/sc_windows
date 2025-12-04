use sc_windows::types::{DrawingElement, DrawingTool};
use windows::Win32::Foundation::POINT;

#[test]
fn rectangle_contains_point() {
    let mut el = DrawingElement::new(DrawingTool::Rectangle);
    el.points = vec![
        POINT { x: 10, y: 10 },
        POINT { x: 100, y: 100 },
    ];
    el.update_bounding_rect();
    assert!(el.contains_point(50, 50));
    assert!(!el.contains_point(150, 150));
}

#[test]
fn text_font_size_clamped() {
    let mut el = DrawingElement::new(DrawingTool::Text);
    el.set_font_size(1.0);
    assert!(el.get_effective_font_size() >= sc_windows::constants::MIN_FONT_SIZE);
    el.set_font_size(10_000.0);
    assert!(el.get_effective_font_size() <= sc_windows::constants::MAX_FONT_SIZE);
}

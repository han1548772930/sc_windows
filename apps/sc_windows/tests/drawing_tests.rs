use sc_drawing::defaults::{MAX_FONT_SIZE, MIN_FONT_SIZE};
use sc_windows::drawing::{DrawingElement, DrawingTool};

#[test]
fn rectangle_contains_point() {
    let mut el = DrawingElement::new(DrawingTool::Rectangle);
    el.add_point(10, 10);
    el.set_end_point(100, 100);
    el.update_bounding_rect();
    assert!(el.contains_point(50, 50));
    assert!(!el.contains_point(150, 150));
}

#[test]
fn text_font_size_clamped() {
    let mut el = DrawingElement::new(DrawingTool::Text);
    el.set_font_size(1.0);
    assert!(el.get_effective_font_size() >= MIN_FONT_SIZE);
    el.set_font_size(10_000.0);
    assert!(el.get_effective_font_size() <= MAX_FONT_SIZE);
}

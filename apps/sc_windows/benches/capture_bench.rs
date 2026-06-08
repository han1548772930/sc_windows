use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use sc_app::selection::{RectI32, is_drag_threshold_exceeded};
use sc_drawing::windows::GeometryCache;
use sc_drawing::{Rect, is_rect_valid};
use sc_windows::drawing::{
    DragMode, DrawingAction, DrawingElement, DrawingTool, ElementManager, HistoryManager,
};
use sc_windows::screenshot::selection::SelectionState;

/// Benchmark selection state management.
fn bench_selection_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("SelectionState");

    group.bench_function("new", |b| {
        b.iter(|| {
            let state = SelectionState::new();
            black_box(state)
        });
    });

    group.bench_function("start_end_selection", |b| {
        let mut state = SelectionState::new();
        let start_x = 100;
        let start_y = 100;

        b.iter(|| {
            state.set_mouse_pressed(true);
            state.start_interaction(black_box(start_x), black_box(start_y), DragMode::Moving);
            state.end_interaction();

            state.reset();
        });
    });

    group.bench_function("set_mouse_pressed", |b| {
        let mut state = SelectionState::new();
        b.iter(|| {
            state.set_mouse_pressed(true);
            state.clear_selection();
        });
    });

    group.bench_function("is_interacting", |b| {
        let mut state = SelectionState::new();
        state.start_interaction(0, 0, DragMode::Moving);
        b.iter(|| black_box(state.is_interacting()));
        state.end_interaction();
    });

    group.finish();
}

/// Benchmark selection handle detection.
fn bench_handle_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("Handle Detection");

    group.bench_function("get_handle_at_position_hit", |b| {
        let state = SelectionState::new();
        let rect = RectI32::from_points(100, 100, 500, 500);

        b.iter(|| black_box(state.get_handle_at_position(Some(rect), 100, 100)));
    });

    group.bench_function("get_handle_at_position_miss", |b| {
        let state = SelectionState::new();
        let rect = RectI32::from_points(100, 100, 500, 500);

        b.iter(|| black_box(state.get_handle_at_position(Some(rect), 300, 300)));
    });

    group.finish();
}

/// Benchmark utility functions.
fn bench_utils(c: &mut Criterion) {
    #[inline]
    fn clamp_to_rect_i32(x: i32, y: i32, rect: &RectI32) -> (i32, i32) {
        (
            x.max(rect.left).min(rect.right),
            y.max(rect.top).min(rect.bottom),
        )
    }

    #[inline]
    fn point_to_line_distance(px: i32, py: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> f64 {
        let px = f64::from(px);
        let py = f64::from(py);
        let x1 = f64::from(x1);
        let y1 = f64::from(y1);
        let x2 = f64::from(x2);
        let y2 = f64::from(y2);

        let a = px - x1;
        let b = py - y1;
        let c = x2 - x1;
        let d = y2 - y1;

        let dot = b.mul_add(d, a * c);
        let len_sq = d.mul_add(d, c * c);

        if len_sq == 0.0 {
            return (px - x1).hypot(py - y1);
        }

        let param = dot / len_sq;

        let (xx, yy) = if param < 0.0 {
            (x1, y1)
        } else if param > 1.0 {
            (x2, y2)
        } else {
            (param.mul_add(c, x1), param.mul_add(d, y1))
        };

        (px - xx).hypot(py - yy)
    }

    let mut group = c.benchmark_group("Utils");

    group.bench_function("point_to_line_distance", |b| {
        b.iter(|| {
            point_to_line_distance(
                black_box(50),
                black_box(50),
                black_box(0),
                black_box(0),
                black_box(100),
                black_box(100),
            )
        });
    });

    group.bench_function("clamp_to_rect", |b| {
        let rect = RectI32 {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        b.iter(|| clamp_to_rect_i32(black_box(2000), black_box(1500), &rect));
    });

    group.bench_function("is_drag_threshold_exceeded", |b| {
        b.iter(|| {
            is_drag_threshold_exceeded(
                black_box(100),
                black_box(100),
                black_box(110),
                black_box(110),
            )
        });
    });

    group.bench_function("is_rect_valid", |b| {
        let rect = Rect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };
        b.iter(|| is_rect_valid(&rect, black_box(50)));
    });

    group.finish();
}

/// Benchmark history management.
fn bench_history_manager(c: &mut Criterion) {
    let mut group = c.benchmark_group("HistoryManager");

    group.bench_function("save_state", |b| {
        let mut history = HistoryManager::new();
        let mut elements = ElementManager::new();

        for _ in 0..10 {
            let mut el = DrawingElement::new(DrawingTool::Rectangle);
            el.add_point(0, 0);
            el.set_end_point(100, 100);
            el.update_bounding_rect();
            elements.add_element(el);
        }

        b.iter(|| {
            history.save_state(elements.get_elements(), None);
        });
    });

    group.bench_function("record_action", |b| {
        let mut history = HistoryManager::new();

        b.iter(|| {
            let el = DrawingElement::new(DrawingTool::Rectangle);
            let action = DrawingAction::AddElement {
                element: el,
                index: 0,
            };
            history.record_action(black_box(action), None, None);
        });
    });

    group.bench_function("undo_action", |b| {
        let mut history = HistoryManager::new();

        for i in 0usize..20 {
            let mut el = DrawingElement::new(DrawingTool::Rectangle);
            let offset = i as i32 * 10;
            el.add_point(offset, offset);
            el.set_end_point(offset + 100, offset + 100);
            el.update_bounding_rect();
            let action = DrawingAction::AddElement {
                element: el,
                index: i,
            };
            history.record_action(action, None, None);
        }

        b.iter(|| {
            if let Some((action, sel)) = history.undo_action() {
                let _ = black_box(action);
                let _ = black_box(sel);
                if let Some((action2, _)) = history.redo_action() {
                    let _ = black_box(action2);
                }
            }
        });
    });

    group.finish();
}

/// Benchmark element management.
fn bench_element_manager(c: &mut Criterion) {
    let mut group = c.benchmark_group("ElementManager");

    group.bench_function("add_element", |b| {
        let mut manager = ElementManager::new();
        b.iter(|| {
            let mut el = DrawingElement::new(DrawingTool::Rectangle);
            el.add_point(0, 0);
            el.set_end_point(100, 100);
            el.update_bounding_rect();
            manager.add_element(el);
        });
        manager.clear();
    });

    for count in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("get_element_at_position", count),
            &count,
            |b, &count| {
                let mut manager = ElementManager::new();
                for i in 0..count {
                    let mut el = DrawingElement::new(DrawingTool::Rectangle);
                    el.add_point(i * 50, i * 50);
                    el.set_end_point(i * 50 + 40, i * 50 + 40);
                    el.update_bounding_rect();
                    manager.add_element(el);
                }

                b.iter(|| {
                    black_box(
                        manager
                            .get_element_at_position((count / 2) * 50 + 20, (count / 2) * 50 + 20),
                    )
                });
            },
        );
    }

    group.finish();
}

/// Benchmark `GeometryCache`.
fn bench_geometry_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("GeometryCache");

    group.bench_function("new", |b| {
        b.iter(|| {
            let cache = GeometryCache::new();
            black_box(cache)
        });
    });

    group.bench_function("mark_dirty", |b| {
        let mut cache = GeometryCache::new();
        let mut id = 0u64;
        b.iter(|| {
            cache.mark_dirty(black_box(id));
            id = id.wrapping_add(1);
        });
    });

    group.bench_function("mark_dirty_batch_100", |b| {
        let mut cache = GeometryCache::new();
        let ids: Vec<u64> = (0..100u64).collect();
        b.iter(|| {
            cache.mark_dirty_batch(black_box(&ids));
        });
    });

    group.bench_function("invalidate_all", |b| {
        let mut cache = GeometryCache::new();
        for i in 0..100 {
            cache.mark_dirty(i);
        }
        b.iter(|| {
            cache.invalidate_all();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_selection_state,
    bench_handle_detection,
    bench_utils,
    bench_history_manager,
    bench_element_manager,
    bench_geometry_cache,
);

criterion_main!(benches);

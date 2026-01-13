use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use sc_app::selection::{RectI32, is_drag_threshold_exceeded};
use sc_drawing::windows::GeometryCache;
use sc_drawing::{Rect, is_rect_valid};
use sc_windows::drawing::{
    DrawingAction, DrawingElement, DrawingTool, ElementManager, HistoryManager,
};
use sc_windows::screenshot::selection::SelectionState;

/// 测试选择状态管理性能
fn bench_selection_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("SelectionState");

    // 测试创建选择状态
    group.bench_function("new", |b| {
        b.iter(|| {
            let state = SelectionState::new();
            black_box(state)
        });
    });

    // 测试开始和结束选择（新版本：drag-create 几何由 core 计算；host 仅维护 selecting 标记 + 已确认 rect）
    group.bench_function("start_end_selection", |b| {
        let mut state = SelectionState::new();
        let start_x = 100;
        let start_y = 100;
        let end_x = 500;
        let end_y = 500;

        b.iter(|| {
            state.set_mouse_pressed(true);
            state.set_interaction_start_pos(black_box(start_x), black_box(start_y));

            // Simulate confirming a selection on mouse-up.
            let rect = RectI32::from_points(start_x, start_y, end_x, end_y);
            state.set_confirmed_selection_rect(rect);
            state.set_mouse_pressed(false);

            state.reset();
        });
    });

    // 测试直接设置选择矩形（作为已确认选区）
    group.bench_function("set_confirmed_selection_rect", |b| {
        let mut state = SelectionState::new();
        let rect = RectI32 {
            left: 100,
            top: 100,
            right: 500,
            bottom: 500,
        };

        b.iter(|| {
            state.set_confirmed_selection_rect(black_box(rect));
            state.clear_selection();
        });
    });

    // 测试获取选择
    group.bench_function("get_selection", |b| {
        let mut state = SelectionState::new();
        let rect = RectI32::from_points(100, 100, 500, 500);
        state.set_confirmed_selection_rect(rect);
        b.iter(|| black_box(state.get_selection()));
    });

    group.finish();
}

/// 测试手柄检测性能
fn bench_handle_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("Handle Detection");

    // 测试手柄位置检测
    group.bench_function("get_handle_at_position_hit", |b| {
        let mut state = SelectionState::new();
        state.set_confirmed_selection_rect(RectI32::from_points(100, 100, 500, 500));

        b.iter(|| {
            // 测试点击左上角手柄
            black_box(state.get_handle_at_position(100, 100))
        });
    });

    group.bench_function("get_handle_at_position_miss", |b| {
        let mut state = SelectionState::new();
        state.set_confirmed_selection_rect(RectI32::from_points(100, 100, 500, 500));

        b.iter(|| {
            // 测试点击不在任何手柄上的位置
            black_box(state.get_handle_at_position(300, 300))
        });
    });

    group.finish();
}

/// 测试工具函数性能
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
        let px = px as f64;
        let py = py as f64;
        let x1 = x1 as f64;
        let y1 = y1 as f64;
        let x2 = x2 as f64;
        let y2 = y2 as f64;

        let a = px - x1;
        let b = py - y1;
        let c = x2 - x1;
        let d = y2 - y1;

        let dot = a * c + b * d;
        let len_sq = c * c + d * d;

        if len_sq == 0.0 {
            return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
        }

        let param = dot / len_sq;

        let (xx, yy) = if param < 0.0 {
            (x1, y1)
        } else if param > 1.0 {
            (x2, y2)
        } else {
            (x1 + param * c, y1 + param * d)
        };

        ((px - xx).powi(2) + (py - yy).powi(2)).sqrt()
    }

    let mut group = c.benchmark_group("Utils");

    // 测试点到线段距离计算
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

    // 测试坐标钳位
    group.bench_function("clamp_to_rect", |b| {
        let rect = RectI32 {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        b.iter(|| clamp_to_rect_i32(black_box(2000), black_box(1500), &rect));
    });

    // 测试拖拽阈值检查
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

    // 测试矩形有效性检查
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

/// 测试历史管理器性能
fn bench_history_manager(c: &mut Criterion) {
    let mut group = c.benchmark_group("HistoryManager");

    // 测试保存状态
    group.bench_function("save_state", |b| {
        let mut history = HistoryManager::new();
        let mut elements = ElementManager::new();

        // 添加一些元素
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

    // 测试记录操作
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

    // 测试撤销操作
    group.bench_function("undo_action", |b| {
        let mut history = HistoryManager::new();

        // 记录多个操作
        for i in 0..20 {
            let mut el = DrawingElement::new(DrawingTool::Rectangle);
            el.add_point(i * 10, i * 10);
            el.set_end_point(i * 10 + 100, i * 10 + 100);
            el.update_bounding_rect();
            let action = DrawingAction::AddElement {
                element: el,
                index: i as usize,
            };
            history.record_action(action, None, None);
        }

        b.iter(|| {
            // 撤销一次，然后重做恢复，保持基准测试可重复
            if let Some((action, sel)) = history.undo_action() {
                let _ = black_box(action);
                let _ = black_box(sel);
                // 重做恢复
                if let Some((action2, _)) = history.redo_action() {
                    let _ = black_box(action2);
                }
            }
        });
    });

    group.finish();
}

/// 测试元素管理器性能
fn bench_element_manager(c: &mut Criterion) {
    let mut group = c.benchmark_group("ElementManager");

    // 测试添加元素
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

    // 测试查找元素（不同元素数量）
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
                    // 查找中间位置的元素
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

/// 测试 GeometryCache 性能
fn bench_geometry_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("GeometryCache");

    // 测试创建缓存
    group.bench_function("new", |b| {
        b.iter(|| {
            let cache = GeometryCache::new();
            black_box(cache)
        });
    });

    // 测试标记脏
    group.bench_function("mark_dirty", |b| {
        let mut cache = GeometryCache::new();
        let mut id = 0u64;
        b.iter(|| {
            cache.mark_dirty(black_box(id));
            id = id.wrapping_add(1);
        });
    });

    // 测试批量标记脏
    group.bench_function("mark_dirty_batch_100", |b| {
        let mut cache = GeometryCache::new();
        let ids: Vec<u64> = (0..100u64).collect();
        b.iter(|| {
            cache.mark_dirty_batch(black_box(&ids));
        });
    });

    // 测试失效所有缓存
    group.bench_function("invalidate_all", |b| {
        let mut cache = GeometryCache::new();
        // 预先标记一些脏
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

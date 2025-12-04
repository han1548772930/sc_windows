//! 截图捕获性能基准测试
//!
//! 测试截图相关操作的性能（不含实际屏幕捕获，因为需要GUI环境）。
//! 运行: `cargo bench --bench capture_bench`

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

/// 测试选择状态管理性能
fn bench_selection_state(c: &mut Criterion) {
    use sc_windows::screenshot::selection::SelectionState;
    use windows::Win32::Foundation::RECT;

    let mut group = c.benchmark_group("SelectionState");

    // 测试创建选择状态
    group.bench_function("new", |b| {
        b.iter(|| {
            let state = SelectionState::new();
            black_box(state)
        });
    });

    // 测试开始和结束选择
    group.bench_function("start_end_selection", |b| {
        let mut state = SelectionState::new();
        b.iter(|| {
            state.start_selection(black_box(100), black_box(100));
            state.update_end_point(500, 500);
            state.end_selection(500, 500);
            state.reset();
        });
    });

    // 测试设置自动高亮选择
    group.bench_function("set_auto_highlight", |b| {
        let mut state = SelectionState::new();
        let rect = RECT {
            left: 100,
            top: 100,
            right: 500,
            bottom: 500,
        };
        b.iter(|| {
            state.set_auto_highlight_selection(black_box(rect));
            state.clear_auto_highlight();
        });
    });

    // 测试获取有效选择
    group.bench_function("get_effective_selection", |b| {
        let mut state = SelectionState::new();
        state.start_selection(100, 100);
        state.end_selection(500, 500);
        b.iter(|| black_box(state.get_effective_selection()));
    });

    group.finish();
}

/// 测试手柄检测性能
fn bench_handle_detection(c: &mut Criterion) {
    use sc_windows::screenshot::selection::SelectionState;

    let mut group = c.benchmark_group("Handle Detection");

    // 测试手柄位置检测
    group.bench_function("get_handle_at_position_hit", |b| {
        let mut state = SelectionState::new();
        state.start_selection(100, 100);
        state.end_selection(500, 500);

        b.iter(|| {
            // 测试点击左上角手柄
            black_box(state.get_handle_at_position(100, 100))
        });
    });

    group.bench_function("get_handle_at_position_miss", |b| {
        let mut state = SelectionState::new();
        state.start_selection(100, 100);
        state.end_selection(500, 500);

        b.iter(|| {
            // 测试点击不在任何手柄上的位置
            black_box(state.get_handle_at_position(300, 300))
        });
    });

    group.finish();
}

/// 测试工具函数性能
fn bench_utils(c: &mut Criterion) {
    use windows::Win32::Foundation::RECT;

    let mut group = c.benchmark_group("Utils");

    // 测试点到线段距离计算
    group.bench_function("point_to_line_distance", |b| {
        b.iter(|| {
            sc_windows::utils::point_to_line_distance(
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
        let rect = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        b.iter(|| sc_windows::utils::clamp_to_rect(black_box(2000), black_box(1500), &rect));
    });

    // 测试拖拽阈值检查
    group.bench_function("is_drag_threshold_exceeded", |b| {
        b.iter(|| {
            sc_windows::utils::is_drag_threshold_exceeded(
                black_box(100),
                black_box(100),
                black_box(110),
                black_box(110),
            )
        });
    });

    // 测试矩形有效性检查
    group.bench_function("is_rect_valid", |b| {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };
        b.iter(|| sc_windows::utils::is_rect_valid(&rect, black_box(50)));
    });

    group.finish();
}

/// 测试历史管理器性能
fn bench_history_manager(c: &mut Criterion) {
    use sc_windows::drawing::elements::ElementManager;
    use sc_windows::drawing::history::HistoryManager;
    use sc_windows::types::{DrawingElement, DrawingTool};
    use windows::Win32::Foundation::POINT;

    let mut group = c.benchmark_group("HistoryManager");

    // 测试保存状态
    group.bench_function("save_state", |b| {
        let mut history = HistoryManager::new();
        let mut elements = ElementManager::new();

        // 添加一些元素
        for _ in 0..10 {
            let mut el = DrawingElement::new(DrawingTool::Rectangle);
            el.points.push(POINT { x: 0, y: 0 });
            el.points.push(POINT { x: 100, y: 100 });
            elements.add_element(el);
        }

        b.iter(|| {
            history.save_state(&elements, None);
        });
    });

    // 测试撤销操作
    group.bench_function("undo", |b| {
        let mut history = HistoryManager::new();
        let mut elements = ElementManager::new();

        // 保存多个状态
        for i in 0..20 {
            let mut el = DrawingElement::new(DrawingTool::Rectangle);
            el.points.push(POINT {
                x: i * 10,
                y: i * 10,
            });
            el.points.push(POINT {
                x: i * 10 + 100,
                y: i * 10 + 100,
            });
            elements.add_element(el);
            history.save_state(&elements, None);
        }

        b.iter(|| {
            // 撤销一次，然后重做恢复，保持基准测试可重复
            if let Some((restored, sel)) = history.undo() {
                elements.restore_state(restored);
                let _ = black_box(sel);
                // 重做恢复
                if let Some((restored2, _)) = history.redo() {
                    elements.restore_state(restored2);
                }
            }
        });
    });

    group.finish();
}

/// 测试元素管理器性能
fn bench_element_manager(c: &mut Criterion) {
    use sc_windows::drawing::elements::ElementManager;
    use sc_windows::types::{DrawingElement, DrawingTool};
    use windows::Win32::Foundation::POINT;

    let mut group = c.benchmark_group("ElementManager");

    // 测试添加元素
    group.bench_function("add_element", |b| {
        let mut manager = ElementManager::new();
        b.iter(|| {
            let mut el = DrawingElement::new(DrawingTool::Rectangle);
            el.points.push(POINT { x: 0, y: 0 });
            el.points.push(POINT { x: 100, y: 100 });
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
                    el.points.push(POINT {
                        x: i * 50,
                        y: i * 50,
                    });
                    el.points.push(POINT {
                        x: i * 50 + 40,
                        y: i * 50 + 40,
                    });
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
    use sc_windows::drawing::cache::GeometryCache;

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
        let mut id = 0usize;
        b.iter(|| {
            cache.mark_dirty(black_box(id));
            id = id.wrapping_add(1);
        });
    });

    // 测试批量标记脏
    group.bench_function("mark_dirty_batch_100", |b| {
        let mut cache = GeometryCache::new();
        let ids: Vec<usize> = (0..100).collect();
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

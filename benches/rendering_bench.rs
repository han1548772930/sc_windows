//! 渲染性能基准测试
//!
//! 测试绘图元素渲染的性能。
//! 运行: `cargo bench --bench rendering_bench`

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

/// 测试创建 DrawingElement 的性能
fn bench_create_element(c: &mut Criterion) {
    use sc_windows::drawing::{DrawingElement, DrawingTool};

    let mut group = c.benchmark_group("DrawingElement Creation");

    for tool in [
        DrawingTool::Rectangle,
        DrawingTool::Circle,
        DrawingTool::Arrow,
        DrawingTool::Pen,
        DrawingTool::Text,
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", tool)),
            &tool,
            |b, &tool| {
                b.iter(|| {
                    let element = DrawingElement::new(black_box(tool));
                    black_box(element)
                });
            },
        );
    }

    group.finish();
}

/// 测试 DrawingElement 的边界矩形计算性能
fn bench_bounding_rect(c: &mut Criterion) {
    use sc_windows::drawing::{DrawingElement, DrawingTool};
    use windows::Win32::Foundation::POINT;

    let mut group = c.benchmark_group("Bounding Rect Calculation");

    // 测试不同点数的 Pen 元素
    for point_count in [10, 50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::new("Pen", point_count),
            &point_count,
            |b, &count| {
                let mut element = DrawingElement::new(DrawingTool::Pen);
                for i in 0..count {
                    element.points.push(POINT { x: i * 5, y: i * 3 });
                }
                b.iter(|| {
                    element.update_bounding_rect();
                    black_box(element.rect)
                });
            },
        );
    }

    // 测试矩形和圆形元素
    group.bench_function("Rectangle", |b| {
        let mut element = DrawingElement::new(DrawingTool::Rectangle);
        element.points.push(POINT { x: 0, y: 0 });
        element.points.push(POINT { x: 100, y: 100 });
        b.iter(|| {
            element.update_bounding_rect();
            black_box(element.rect)
        });
    });

    group.finish();
}

/// 测试点击检测性能
fn bench_contains_point(c: &mut Criterion) {
    use sc_windows::drawing::{DrawingElement, DrawingTool};
    use windows::Win32::Foundation::POINT;

    let mut group = c.benchmark_group("Contains Point Check");

    // 测试矩形包含点检测
    group.bench_function("Rectangle", |b| {
        let mut element = DrawingElement::new(DrawingTool::Rectangle);
        element.points.push(POINT { x: 0, y: 0 });
        element.points.push(POINT { x: 100, y: 100 });
        element.update_bounding_rect();

        b.iter(|| black_box(element.contains_point(50, 50)));
    });

    // 测试 Pen 路径包含点检测（较复杂）
    for point_count in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("Pen", point_count),
            &point_count,
            |b, &count| {
                let mut element = DrawingElement::new(DrawingTool::Pen);
                for i in 0..count {
                    element.points.push(POINT { x: i * 5, y: i * 3 });
                }
                element.update_bounding_rect();

                b.iter(|| black_box(element.contains_point(50, 50)));
            },
        );
    }

    group.finish();
}

/// 测试 DrawingElement 移动性能
fn bench_move_element(c: &mut Criterion) {
    use sc_windows::drawing::{DrawingElement, DrawingTool};
    use windows::Win32::Foundation::POINT;

    let mut group = c.benchmark_group("Element Move");

    for point_count in [2, 50, 200] {
        group.bench_with_input(
            BenchmarkId::new("Pen", point_count),
            &point_count,
            |b, &count| {
                let mut element = DrawingElement::new(DrawingTool::Pen);
                for i in 0..count {
                    element.points.push(POINT { x: i * 5, y: i * 3 });
                }
                element.update_bounding_rect();

                b.iter(|| {
                    element.move_by(black_box(10), black_box(10));
                    element.move_by(-10, -10); // 恢复位置
                });
            },
        );
    }

    group.finish();
}

/// 测试 DrawingElement 调整大小性能
fn bench_resize_element(c: &mut Criterion) {
    use sc_windows::drawing::{DrawingElement, DrawingTool};
    use windows::Win32::Foundation::{POINT, RECT};

    let mut group = c.benchmark_group("Element Resize");

    // 测试矩形调整大小
    group.bench_function("Rectangle", |b| {
        let mut element = DrawingElement::new(DrawingTool::Rectangle);
        element.points.push(POINT { x: 0, y: 0 });
        element.points.push(POINT { x: 100, y: 100 });
        element.rect = RECT {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };

        let new_rect = RECT {
            left: 10,
            top: 10,
            right: 200,
            bottom: 200,
        };
        let original_rect = RECT {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };

        b.iter(|| {
            element.resize(black_box(new_rect));
            element.resize(original_rect); // 恢复
        });
    });

    // 测试 Pen 调整大小（涉及所有点的缩放）
    for point_count in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("Pen", point_count),
            &point_count,
            |b, &count| {
                let mut element = DrawingElement::new(DrawingTool::Pen);
                for i in 0..count {
                    element.points.push(POINT { x: i * 5, y: i * 3 });
                }
                element.update_bounding_rect();
                let original_rect = element.rect;
                let new_rect = RECT {
                    left: original_rect.left + 10,
                    top: original_rect.top + 10,
                    right: original_rect.right + 50,
                    bottom: original_rect.bottom + 50,
                };

                b.iter(|| {
                    element.resize(black_box(new_rect));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_create_element,
    bench_bounding_rect,
    bench_contains_point,
    bench_move_element,
    bench_resize_element,
);

criterion_main!(benches);

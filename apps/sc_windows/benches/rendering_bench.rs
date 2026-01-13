use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use sc_drawing::Rect;
use sc_windows::drawing::{DrawingElement, DrawingTool};

/// 测试创建 DrawingElement 的性能
fn bench_create_element(c: &mut Criterion) {
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
    let mut group = c.benchmark_group("Bounding Rect Calculation");

    // 测试不同点数的 Pen 元素
    for point_count in [10, 50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::new("Pen", point_count),
            &point_count,
            |b, &count| {
                let mut element = DrawingElement::new(DrawingTool::Pen);
                for i in 0..count {
                    element.add_point(i * 5, i * 3);
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
        element.add_point(0, 0);
        element.set_end_point(100, 100);
        b.iter(|| {
            element.update_bounding_rect();
            black_box(element.rect)
        });
    });

    group.finish();
}

/// 测试点击检测性能
fn bench_contains_point(c: &mut Criterion) {
    let mut group = c.benchmark_group("Contains Point Check");

    // 测试矩形包含点检测
    group.bench_function("Rectangle", |b| {
        let mut element = DrawingElement::new(DrawingTool::Rectangle);
        element.add_point(0, 0);
        element.set_end_point(100, 100);
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
                    element.add_point(i * 5, i * 3);
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
    let mut group = c.benchmark_group("Element Move");

    for point_count in [2, 50, 200] {
        group.bench_with_input(
            BenchmarkId::new("Pen", point_count),
            &point_count,
            |b, &count| {
                let mut element = DrawingElement::new(DrawingTool::Pen);
                for i in 0..count {
                    element.add_point(i * 5, i * 3);
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
    let mut group = c.benchmark_group("Element Resize");

    // 测试矩形调整大小
    group.bench_function("Rectangle", |b| {
        let mut element = DrawingElement::new(DrawingTool::Rectangle);
        element.add_point(0, 0);
        element.set_end_point(100, 100);
        element.update_bounding_rect();

        let new_rect = Rect {
            left: 10,
            top: 10,
            right: 200,
            bottom: 200,
        };
        let original_rect = element.rect;

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
                    element.add_point(i * 5, i * 3);
                }
                element.update_bounding_rect();
                let original_rect = element.rect;
                let new_rect = Rect {
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

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use sc_drawing::Rect;
use sc_windows::drawing::{DrawingElement, DrawingTool};

/// Benchmark `DrawingElement` creation.
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
            BenchmarkId::from_parameter(format!("{tool:?}")),
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

/// Benchmark `DrawingElement` bounds calculation.
fn bench_bounding_rect(c: &mut Criterion) {
    let mut group = c.benchmark_group("Bounding Rect Calculation");

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

/// Benchmark hit testing.
fn bench_contains_point(c: &mut Criterion) {
    let mut group = c.benchmark_group("Contains Point Check");

    group.bench_function("Rectangle", |b| {
        let mut element = DrawingElement::new(DrawingTool::Rectangle);
        element.add_point(0, 0);
        element.set_end_point(100, 100);
        element.update_bounding_rect();

        b.iter(|| black_box(element.contains_point(50, 50)));
    });

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

/// Benchmark `DrawingElement` movement.
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
                    element.move_by(-10, -10);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark `DrawingElement` resize behavior.
fn bench_resize_element(c: &mut Criterion) {
    let mut group = c.benchmark_group("Element Resize");

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
            element.resize(original_rect);
        });
    });

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

use std::collections::HashMap;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN};
use windows::Win32::Graphics::Direct2D::ID2D1PathGeometry;
use windows_numerics::Vector2;

use super::common::{render_handles_8, render_selection_border};
use crate::windows::context::{RenderContext, RenderOptions};
use crate::windows::renderable::{RenderError, RenderResult, Renderable};
use crate::{DrawingElement, Point, Rect};

/// 画笔渲染器
///
/// 使用 PathGeometry 优化多点路径的渲染性能。
pub struct PenRenderer {
    /// PathGeometry 缓存（按元素 ID 索引）
    geometry_cache: HashMap<u64, ID2D1PathGeometry>,
}

impl Default for PenRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl PenRenderer {
    /// 创建新的画笔渲染器
    pub fn new() -> Self {
        Self {
            geometry_cache: HashMap::new(),
        }
    }

    /// 清除几何体缓存
    pub fn clear_cache(&mut self) {
        self.geometry_cache.clear();
    }

    /// 移除指定元素的缓存
    pub fn remove_cached(&mut self, element_id: u64) {
        self.geometry_cache.remove(&element_id);
    }

    /// 从点集创建 PathGeometry
    fn create_path_geometry(ctx: &RenderContext, points: &[Point]) -> Option<ID2D1PathGeometry> {
        if points.len() < 2 {
            return None;
        }

        unsafe {
            let path_geometry = ctx.factory.CreatePathGeometry().ok()?;
            let sink = path_geometry.Open().ok()?;

            // 移动到第一个点
            let first = Vector2 {
                X: points[0].x as f32,
                Y: points[0].y as f32,
            };
            sink.BeginFigure(first, D2D1_FIGURE_BEGIN_HOLLOW);

            // 添加后续点
            for point in points.iter().skip(1) {
                let p = Vector2 {
                    X: point.x as f32,
                    Y: point.y as f32,
                };
                sink.AddLine(p);
            }

            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close().ok()?;

            Some(path_geometry)
        }
    }
}

impl Renderable for PenRenderer {
    fn render(&self, element: &DrawingElement, ctx: &mut RenderContext) -> RenderResult {
        if element.points.len() < 2 {
            return Ok(());
        }

        let brush = ctx
            .get_brush(element.color)
            .ok_or_else(|| RenderError::ResourceCreation("Failed to create brush".into()))?
            .clone();

        // 创建 PathGeometry（每次渲染时创建，因为 self 是不可变借用）
        // 如果需要缓存，应在调用方使用 PenRenderer::with_cache 或外部缓存
        let path_geometry = Self::create_path_geometry(ctx, &element.points).ok_or_else(|| {
            RenderError::ResourceCreation("Failed to create path geometry".into())
        })?;

        unsafe {
            ctx.render_target
                .DrawGeometry(&path_geometry, &brush, element.thickness, None);
        }

        Ok(())
    }

    fn render_selection(
        &self,
        bounds: Rect,
        ctx: &mut RenderContext,
        options: &RenderOptions,
    ) -> RenderResult {
        render_selection_border(bounds, ctx, options)
    }

    fn render_handles(
        &self,
        bounds: Rect,
        ctx: &mut RenderContext,
        options: &RenderOptions,
    ) -> RenderResult {
        render_handles_8(bounds, ctx, options)
    }
}

/// 带缓存的画笔渲染
///
/// 用于需要重复渲染同一元素的场景（如静态层）。
pub struct CachedPenRenderer<'a> {
    cache: &'a mut HashMap<u64, ID2D1PathGeometry>,
}

impl<'a> CachedPenRenderer<'a> {
    /// 创建带缓存的渲染器
    pub fn new(cache: &'a mut HashMap<u64, ID2D1PathGeometry>) -> Self {
        Self { cache }
    }

    /// 渲染元素（使用缓存）
    pub fn render(&mut self, element: &DrawingElement, ctx: &mut RenderContext) -> RenderResult {
        if element.points.len() < 2 {
            return Ok(());
        }

        let brush = ctx
            .get_brush(element.color)
            .ok_or_else(|| RenderError::ResourceCreation("Failed to create brush".into()))?
            .clone();

        // 尝试从缓存获取，或创建新的
        let geometry = if let Some(cached) = self.cache.get(&element.id) {
            cached.clone()
        } else {
            let new_geom =
                PenRenderer::create_path_geometry(ctx, &element.points).ok_or_else(|| {
                    RenderError::ResourceCreation("Failed to create path geometry".into())
                })?;
            self.cache.insert(element.id, new_geom.clone());
            new_geom
        };

        unsafe {
            ctx.render_target
                .DrawGeometry(&geometry, &brush, element.thickness, None);
        }

        Ok(())
    }
}

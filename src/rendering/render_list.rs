//! 渲染列表模块
//!
//! 各模块提交渲染图元到统一的列表中，由渲染器统一排序和批处理绘制。
//! 这比各模块直接调用绘图 API 更高效且通用。

use crate::platform::{BitmapId, Color, DrawStyle, PlatformError, PlatformRenderer, Point, Rectangle, TextStyle};

/// 渲染图元
/// 
/// 表示一个需要绘制的图形元素，包含位置、样式和 Z-Order 信息。
#[derive(Debug, Clone)]
pub enum RenderItem {
    /// 位图
    Bitmap {
        id: BitmapId,
        dest_rect: Rectangle,
        src_rect: Option<Rectangle>,
        opacity: f32,
        z_order: i32,
    },
    
    /// 矩形
    Rectangle {
        rect: Rectangle,
        style: DrawStyle,
        z_order: i32,
    },
    
    /// 圆角矩形
    RoundedRectangle {
        rect: Rectangle,
        radius: f32,
        style: DrawStyle,
        z_order: i32,
    },
    
    /// 圆形/椭圆
    Circle {
        center: Point,
        radius: f32,
        style: DrawStyle,
        z_order: i32,
    },
    
    /// 线条
    Line {
        start: Point,
        end: Point,
        style: DrawStyle,
        z_order: i32,
    },
    
    /// 文本
    Text {
        text: String,
        position: Point,
        style: TextStyle,
        z_order: i32,
    },
    
    /// 虚线矩形
    DashedRectangle {
        rect: Rectangle,
        style: DrawStyle,
        dash_pattern: Vec<f32>,
        z_order: i32,
    },
    
    /// 选择区域遮罩
    SelectionMask {
        screen_rect: Rectangle,
        selection_rect: Rectangle,
        mask_color: Color,
        z_order: i32,
    },
    
    /// 选择区域边框
    SelectionBorder {
        rect: Rectangle,
        color: Color,
        width: f32,
        dash_pattern: Option<Vec<f32>>,
        z_order: i32,
    },
    
    /// 选择区域手柄
    SelectionHandles {
        rect: Rectangle,
        handle_size: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
        z_order: i32,
    },
    
    /// 元素手柄
    ElementHandles {
        rect: Rectangle,
        handle_radius: f32,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
        z_order: i32,
    },
    
    /// 裁剪区域开始
    PushClipRect {
        rect: Rectangle,
        z_order: i32,
    },
    
    /// 裁剪区域结束
    PopClipRect {
        z_order: i32,
    },
}

impl RenderItem {
    /// 获取图元的 Z-Order
    pub fn z_order(&self) -> i32 {
        match self {
            RenderItem::Bitmap { z_order, .. } => *z_order,
            RenderItem::Rectangle { z_order, .. } => *z_order,
            RenderItem::RoundedRectangle { z_order, .. } => *z_order,
            RenderItem::Circle { z_order, .. } => *z_order,
            RenderItem::Line { z_order, .. } => *z_order,
            RenderItem::Text { z_order, .. } => *z_order,
            RenderItem::DashedRectangle { z_order, .. } => *z_order,
            RenderItem::SelectionMask { z_order, .. } => *z_order,
            RenderItem::SelectionBorder { z_order, .. } => *z_order,
            RenderItem::SelectionHandles { z_order, .. } => *z_order,
            RenderItem::ElementHandles { z_order, .. } => *z_order,
            RenderItem::PushClipRect { z_order, .. } => *z_order,
            RenderItem::PopClipRect { z_order, .. } => *z_order,
        }
    }
}

/// Z-Order 层级常量
pub mod z_order {
    /// 背景层（截图背景）
    pub const BACKGROUND: i32 = 0;
    /// 遮罩层
    pub const MASK: i32 = 100;
    /// 静态绘图元素层
    pub const STATIC_ELEMENTS: i32 = 200;
    /// 选择框边框层
    pub const SELECTION_BORDER: i32 = 300;
    /// 当前绘制元素层
    pub const CURRENT_ELEMENT: i32 = 400;
    /// 选择框手柄层
    pub const SELECTION_HANDLES: i32 = 500;
    /// 元素手柄层
    pub const ELEMENT_HANDLES: i32 = 600;
    /// 工具栏层
    pub const TOOLBAR: i32 = 700;
    /// 光标层
    pub const CURSOR: i32 = 800;
}

/// 渲染列表
/// 
/// 收集所有待渲染的图元，按 Z-Order 排序后统一绘制。
#[derive(Debug, Default)]
pub struct RenderList {
    items: Vec<RenderItem>,
}

impl RenderList {
    /// 创建新的渲染列表
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// 创建带预分配容量的渲染列表
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    /// 提交渲染图元
    pub fn submit(&mut self, item: RenderItem) {
        self.items.push(item);
    }

    /// 批量提交渲染图元
    pub fn submit_batch(&mut self, items: impl IntoIterator<Item = RenderItem>) {
        self.items.extend(items);
    }

    /// 按 Z-Order 排序
    pub fn sort_by_z_order(&mut self) {
        self.items.sort_by_key(|item| item.z_order());
    }

    /// 清空渲染列表
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// 获取图元数量
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 执行渲染
    /// 
    /// 按 Z-Order 顺序将所有图元绘制到渲染器。
    pub fn execute<R: PlatformRenderer<Error = PlatformError> + ?Sized>(
        &mut self,
        renderer: &mut R,
    ) -> Result<(), PlatformError> {
        // 先排序
        self.sort_by_z_order();

        // 按顺序绘制
        for item in &self.items {
            Self::render_item(renderer, item)?;
        }

        Ok(())
    }

    /// 渲染单个图元
    fn render_item<R: PlatformRenderer<Error = PlatformError> + ?Sized>(
        renderer: &mut R,
        item: &RenderItem,
    ) -> Result<(), PlatformError> {
        match item {
            RenderItem::Rectangle { rect, style, .. } => {
                renderer.draw_rectangle(*rect, style)?;
            }
            
            RenderItem::RoundedRectangle { rect, radius, style, .. } => {
                renderer.draw_rounded_rectangle(*rect, *radius, style)?;
            }
            
            RenderItem::Circle { center, radius, style, .. } => {
                renderer.draw_circle(*center, *radius, style)?;
            }
            
            RenderItem::Line { start, end, style, .. } => {
                renderer.draw_line(*start, *end, style)?;
            }
            
            RenderItem::Text { text, position, style, .. } => {
                renderer.draw_text(text, *position, style)?;
            }
            
            RenderItem::DashedRectangle { rect, style, dash_pattern, .. } => {
                renderer.draw_dashed_rectangle(*rect, style, dash_pattern)?;
            }
            
            RenderItem::SelectionMask { screen_rect, selection_rect, mask_color, .. } => {
                renderer.draw_selection_mask(*screen_rect, *selection_rect, *mask_color)?;
            }
            
            RenderItem::SelectionBorder { rect, color, width, dash_pattern, .. } => {
                renderer.draw_selection_border(*rect, *color, *width, dash_pattern.as_deref())?;
            }
            
            RenderItem::SelectionHandles { rect, handle_size, fill_color, border_color, border_width, .. } => {
                renderer.draw_selection_handles(*rect, *handle_size, *fill_color, *border_color, *border_width)?;
            }
            
            RenderItem::ElementHandles { rect, handle_radius, fill_color, border_color, border_width, .. } => {
                renderer.draw_element_handles(*rect, *handle_radius, *fill_color, *border_color, *border_width)?;
            }
            
            RenderItem::PushClipRect { rect, .. } => {
                renderer.push_clip_rect(*rect)?;
            }
            
            RenderItem::PopClipRect { .. } => {
                renderer.pop_clip_rect()?;
            }
            
            // Bitmap 需要特殊处理，因为 PlatformRenderer trait 没有直接的 draw_bitmap 方法
            // 这里暂时跳过，后续可以扩展 PlatformRenderer trait
            RenderItem::Bitmap { .. } => {
                // TODO: 实现位图绘制
            }
        }

        Ok(())
    }
}

/// RenderList 的便捷构建器
pub struct RenderListBuilder {
    list: RenderList,
    default_z_order: i32,
}

impl RenderListBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            list: RenderList::new(),
            default_z_order: 0,
        }
    }

    /// 设置默认 Z-Order
    pub fn with_z_order(mut self, z_order: i32) -> Self {
        self.default_z_order = z_order;
        self
    }

    /// 添加矩形
    pub fn rectangle(mut self, rect: Rectangle, style: DrawStyle) -> Self {
        self.list.submit(RenderItem::Rectangle {
            rect,
            style,
            z_order: self.default_z_order,
        });
        self
    }

    /// 添加圆形
    pub fn circle(mut self, center: Point, radius: f32, style: DrawStyle) -> Self {
        self.list.submit(RenderItem::Circle {
            center,
            radius,
            style,
            z_order: self.default_z_order,
        });
        self
    }

    /// 添加线条
    pub fn line(mut self, start: Point, end: Point, style: DrawStyle) -> Self {
        self.list.submit(RenderItem::Line {
            start,
            end,
            style,
            z_order: self.default_z_order,
        });
        self
    }

    /// 添加文本
    pub fn text(mut self, text: impl Into<String>, position: Point, style: TextStyle) -> Self {
        self.list.submit(RenderItem::Text {
            text: text.into(),
            position,
            style,
            z_order: self.default_z_order,
        });
        self
    }

    /// 构建渲染列表
    pub fn build(self) -> RenderList {
        self.list
    }
}

impl Default for RenderListBuilder {
    fn default() -> Self {
        Self::new()
    }
}

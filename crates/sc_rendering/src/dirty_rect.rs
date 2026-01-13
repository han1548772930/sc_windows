use crate::types::Rectangle;

/// 脏矩形追踪器
///
/// 收集所有需要重绘的区域，并提供合并后的脏矩形用于剪裁渲染。
#[derive(Debug, Default)]
pub struct DirtyRectTracker {
    /// 脏区域列表
    dirty_regions: Vec<Rectangle>,
    /// 是否需要全屏重绘
    full_redraw: bool,
    /// 屏幕尺寸（用于全屏重绘时返回）
    screen_size: (f32, f32),
}

impl DirtyRectTracker {
    /// 创建新的脏矩形追踪器
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            dirty_regions: Vec::new(),
            full_redraw: false,
            screen_size: (screen_width, screen_height),
        }
    }

    /// 设置屏幕尺寸
    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.screen_size = (width, height);
    }

    /// 标记区域为脏
    pub fn mark_dirty(&mut self, rect: Rectangle) {
        // 如果已经需要全屏重绘，不需要再添加
        if self.full_redraw {
            return;
        }

        // 检查是否与现有区域重叠，如果重叠则合并
        for existing in &mut self.dirty_regions {
            if existing.intersects(&rect) {
                *existing = existing.union(&rect);
                return;
            }
        }

        self.dirty_regions.push(rect);
    }

    /// 标记需要全屏重绘
    pub fn mark_full_redraw(&mut self) {
        self.full_redraw = true;
        self.dirty_regions.clear();
    }

    /// 检查是否需要全屏重绘
    pub fn needs_full_redraw(&self) -> bool {
        self.full_redraw
    }

    /// 获取合并后的脏矩形
    ///
    /// 返回 None 表示没有脏区域，返回 Some 表示需要重绘的区域。
    /// 如果是全屏重绘，返回整个屏幕区域。
    pub fn get_combined_dirty_rect(&self) -> Option<Rectangle> {
        if self.full_redraw {
            return Some(Rectangle {
                x: 0.0,
                y: 0.0,
                width: self.screen_size.0,
                height: self.screen_size.1,
            });
        }

        if self.dirty_regions.is_empty() {
            return None;
        }

        // 合并所有脏区域
        let mut combined = self.dirty_regions[0];
        for rect in self.dirty_regions.iter().skip(1) {
            combined = combined.union(rect);
        }

        Some(combined)
    }

    /// 获取所有脏区域（未合并）
    pub fn get_dirty_regions(&self) -> &[Rectangle] {
        &self.dirty_regions
    }

    /// 清空脏区域追踪
    pub fn clear(&mut self) {
        self.dirty_regions.clear();
        self.full_redraw = false;
    }

    /// 检查是否有脏区域
    pub fn is_dirty(&self) -> bool {
        self.full_redraw || !self.dirty_regions.is_empty()
    }

    /// 裁剪矩形到屏幕范围内
    pub fn clip_to_screen(&self, rect: Rectangle) -> Rectangle {
        let left = rect.x.max(0.0);
        let top = rect.y.max(0.0);
        let right = rect.right().min(self.screen_size.0);
        let bottom = rect.bottom().min(self.screen_size.1);

        Rectangle {
            x: left,
            y: top,
            width: (right - left).max(0.0),
            height: (bottom - top).max(0.0),
        }
    }
}

/// 脏矩形类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyType {
    /// 全屏重绘
    Full,
    /// 局部重绘
    Partial,
    /// 无需重绘
    None,
}

impl DirtyRectTracker {
    /// 获取脏区域类型
    pub fn dirty_type(&self) -> DirtyType {
        if self.full_redraw {
            DirtyType::Full
        } else if !self.dirty_regions.is_empty() {
            DirtyType::Partial
        } else {
            DirtyType::None
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_mark_dirty() {
        let mut tracker = super::DirtyRectTracker::new(1920.0, 1080.0);

        tracker.mark_dirty(super::Rectangle::new(10.0, 10.0, 100.0, 100.0));
        assert!(tracker.is_dirty());
        assert_eq!(tracker.dirty_type(), super::DirtyType::Partial);
    }

    #[test]
    fn test_mark_full_redraw() {
        let mut tracker = super::DirtyRectTracker::new(1920.0, 1080.0);

        tracker.mark_full_redraw();
        assert!(tracker.needs_full_redraw());
        assert_eq!(tracker.dirty_type(), super::DirtyType::Full);

        let combined = tracker.get_combined_dirty_rect().unwrap();
        assert_eq!(combined.x, 0.0);
        assert_eq!(combined.y, 0.0);
        assert_eq!(combined.width, 1920.0);
        assert_eq!(combined.height, 1080.0);
    }

    #[test]
    fn test_rect_union() {
        let a = super::Rectangle::new(10.0, 10.0, 100.0, 100.0);
        let b = super::Rectangle::new(50.0, 50.0, 100.0, 100.0);

        let union = a.union(&b);

        assert_eq!(union.x, 10.0);
        assert_eq!(union.y, 10.0);
        assert_eq!(union.width, 140.0);
        assert_eq!(union.height, 140.0);
    }

    #[test]
    fn test_clear() {
        let mut tracker = super::DirtyRectTracker::new(1920.0, 1080.0);

        tracker.mark_full_redraw();
        tracker.clear();

        assert!(!tracker.is_dirty());
        assert_eq!(tracker.dirty_type(), super::DirtyType::None);
    }
}

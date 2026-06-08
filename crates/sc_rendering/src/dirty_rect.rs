use crate::types::Rectangle;

#[derive(Debug, Default)]
pub struct DirtyRectTracker {
    dirty_regions: Vec<Rectangle>,
    full_redraw: bool,
    screen_size: (f32, f32),
}

impl DirtyRectTracker {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            dirty_regions: Vec::new(),
            full_redraw: false,
            screen_size: (screen_width, screen_height),
        }
    }

    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.screen_size = (width, height);
    }

    pub fn mark_dirty(&mut self, rect: Rectangle) {
        if self.full_redraw {
            return;
        }

        for existing in &mut self.dirty_regions {
            if existing.intersects(&rect) {
                *existing = existing.union(&rect);
                return;
            }
        }

        self.dirty_regions.push(rect);
    }

    pub fn mark_full_redraw(&mut self) {
        self.full_redraw = true;
        self.dirty_regions.clear();
    }

    pub fn needs_full_redraw(&self) -> bool {
        self.full_redraw
    }

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

        let mut combined = self.dirty_regions[0];
        for rect in self.dirty_regions.iter().skip(1) {
            combined = combined.union(rect);
        }

        Some(combined)
    }

    pub fn get_dirty_regions(&self) -> &[Rectangle] {
        &self.dirty_regions
    }

    pub fn clear(&mut self) {
        self.dirty_regions.clear();
        self.full_redraw = false;
    }

    pub fn is_dirty(&self) -> bool {
        self.full_redraw || !self.dirty_regions.is_empty()
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyType {
    Full,
    Partial,
    None,
}

impl DirtyRectTracker {
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

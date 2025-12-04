//! 图层缓存模块
//!
//! 将静态内容（如截图背景、未选中的绘图元素）渲染到离屏位图缓存，
//! 在渲染循环中只需贴上缓存位图再绘制动态内容，大幅降低 GPU/CPU 占用。

use crate::platform::BitmapId;

/// 图层类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerType {
    /// 背景层（截图背景）
    Background,
    /// 静态元素层（未选中的绘图元素）
    StaticElements,
    /// 选择区域遮罩层
    SelectionMask,
}

/// 图层状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerState {
    /// 有效（可直接使用缓存）
    Valid,
    /// 无效（需要重建）
    Invalid,
    /// 未创建
    NotCreated,
}

/// 单个图层的缓存信息
#[derive(Debug)]
pub struct CachedLayer {
    /// 图层类型
    pub layer_type: LayerType,
    /// 缓存位图 ID
    pub bitmap_id: Option<BitmapId>,
    /// 图层状态
    pub state: LayerState,
    /// 图层尺寸
    pub size: (u32, u32),
    /// 最后更新时间戳
    pub last_updated: u64,
}

impl CachedLayer {
    /// 创建新的图层
    pub fn new(layer_type: LayerType) -> Self {
        Self {
            layer_type,
            bitmap_id: None,
            state: LayerState::NotCreated,
            size: (0, 0),
            last_updated: 0,
        }
    }

    /// 标记为无效
    pub fn invalidate(&mut self) {
        if self.state == LayerState::Valid {
            self.state = LayerState::Invalid;
        }
    }

    /// 标记为有效
    pub fn mark_valid(&mut self, bitmap_id: BitmapId, size: (u32, u32), timestamp: u64) {
        self.bitmap_id = Some(bitmap_id);
        self.state = LayerState::Valid;
        self.size = size;
        self.last_updated = timestamp;
    }

    /// 检查是否需要重建
    pub fn needs_rebuild(&self) -> bool {
        self.state != LayerState::Valid
    }
}

/// 图层缓存管理器
///
/// 管理多个缓存图层，提供统一的失效和重建接口。
#[derive(Debug)]
pub struct LayerCache {
    /// 背景层缓存
    background: CachedLayer,
    /// 静态元素层缓存
    static_elements: CachedLayer,
    /// 选择区域遮罩层缓存
    selection_mask: CachedLayer,
    /// 全局时间戳计数器
    timestamp: u64,
    /// 屏幕尺寸
    screen_size: (u32, u32),
}

impl LayerCache {
    /// 创建新的图层缓存管理器
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            background: CachedLayer::new(LayerType::Background),
            static_elements: CachedLayer::new(LayerType::StaticElements),
            selection_mask: CachedLayer::new(LayerType::SelectionMask),
            timestamp: 0,
            screen_size: (screen_width, screen_height),
        }
    }

    /// 设置屏幕尺寸（尺寸变化时会使所有缓存失效）
    pub fn set_screen_size(&mut self, width: u32, height: u32) {
        if self.screen_size != (width, height) {
            self.screen_size = (width, height);
            self.invalidate_all();
        }
    }

    /// 获取指定类型的图层
    pub fn get_layer(&self, layer_type: LayerType) -> &CachedLayer {
        match layer_type {
            LayerType::Background => &self.background,
            LayerType::StaticElements => &self.static_elements,
            LayerType::SelectionMask => &self.selection_mask,
        }
    }

    /// 获取指定类型的图层（可变）
    pub fn get_layer_mut(&mut self, layer_type: LayerType) -> &mut CachedLayer {
        match layer_type {
            LayerType::Background => &mut self.background,
            LayerType::StaticElements => &mut self.static_elements,
            LayerType::SelectionMask => &mut self.selection_mask,
        }
    }

    /// 使指定图层失效
    pub fn invalidate(&mut self, layer_type: LayerType) {
        self.get_layer_mut(layer_type).invalidate();
    }

    /// 使所有图层失效
    pub fn invalidate_all(&mut self) {
        self.background.invalidate();
        self.static_elements.invalidate();
        self.selection_mask.invalidate();
    }

    /// 检查指定图层是否需要重建
    pub fn needs_rebuild(&self, layer_type: LayerType) -> bool {
        self.get_layer(layer_type).needs_rebuild()
    }

    /// 检查是否有任何图层需要重建
    pub fn any_needs_rebuild(&self) -> bool {
        self.background.needs_rebuild()
            || self.static_elements.needs_rebuild()
            || self.selection_mask.needs_rebuild()
    }

    /// 标记图层为已重建
    pub fn mark_rebuilt(&mut self, layer_type: LayerType, bitmap_id: BitmapId) {
        self.timestamp += 1;
        let screen_size = self.screen_size;
        let timestamp = self.timestamp;
        self.get_layer_mut(layer_type)
            .mark_valid(bitmap_id, screen_size, timestamp);
    }

    /// 获取背景层位图 ID
    pub fn get_background_bitmap(&self) -> Option<BitmapId> {
        if self.background.state == LayerState::Valid {
            self.background.bitmap_id
        } else {
            None
        }
    }

    /// 获取静态元素层位图 ID
    pub fn get_static_elements_bitmap(&self) -> Option<BitmapId> {
        if self.static_elements.state == LayerState::Valid {
            self.static_elements.bitmap_id
        } else {
            None
        }
    }

    /// 获取选择区域遮罩层位图 ID
    pub fn get_selection_mask_bitmap(&self) -> Option<BitmapId> {
        if self.selection_mask.state == LayerState::Valid {
            self.selection_mask.bitmap_id
        } else {
            None
        }
    }

    /// 获取当前时间戳
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// 获取屏幕尺寸
    pub fn screen_size(&self) -> (u32, u32) {
        self.screen_size
    }
}

impl Default for LayerCache {
    fn default() -> Self {
        Self::new(1920, 1080)
    }
}

/// 图层缓存构建器
/// 
/// 提供便捷的方式来构建和更新图层缓存。
pub struct LayerCacheBuilder<'a> {
    cache: &'a mut LayerCache,
}

impl<'a> LayerCacheBuilder<'a> {
    /// 创建新的构建器
    pub fn new(cache: &'a mut LayerCache) -> Self {
        Self { cache }
    }

    /// 设置背景层位图
    pub fn with_background(self, bitmap_id: BitmapId) -> Self {
        self.cache.mark_rebuilt(LayerType::Background, bitmap_id);
        self
    }

    /// 设置静态元素层位图
    pub fn with_static_elements(self, bitmap_id: BitmapId) -> Self {
        self.cache.mark_rebuilt(LayerType::StaticElements, bitmap_id);
        self
    }

    /// 设置选择区域遮罩层位图
    pub fn with_selection_mask(self, bitmap_id: BitmapId) -> Self {
        self.cache.mark_rebuilt(LayerType::SelectionMask, bitmap_id);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_cache_creation() {
        let cache = LayerCache::new(1920, 1080);
        
        assert!(cache.background.needs_rebuild());
        assert!(cache.static_elements.needs_rebuild());
        assert!(cache.selection_mask.needs_rebuild());
    }

    #[test]
    fn test_layer_invalidation() {
        let mut cache = LayerCache::new(1920, 1080);
        
        cache.mark_rebuilt(LayerType::Background, 1);
        assert!(!cache.background.needs_rebuild());
        
        cache.invalidate(LayerType::Background);
        assert!(cache.background.needs_rebuild());
    }

    #[test]
    fn test_screen_size_change_invalidates() {
        let mut cache = LayerCache::new(1920, 1080);
        
        cache.mark_rebuilt(LayerType::Background, 1);
        cache.mark_rebuilt(LayerType::StaticElements, 2);
        
        cache.set_screen_size(2560, 1440);
        
        assert!(cache.background.needs_rebuild());
        assert!(cache.static_elements.needs_rebuild());
    }

    #[test]
    fn test_timestamp_increments() {
        let mut cache = LayerCache::new(1920, 1080);
        
        assert_eq!(cache.timestamp(), 0);
        
        cache.mark_rebuilt(LayerType::Background, 1);
        assert_eq!(cache.timestamp(), 1);
        
        cache.mark_rebuilt(LayerType::StaticElements, 2);
        assert_eq!(cache.timestamp(), 2);
    }
}

use crate::types::BitmapId;

/// 缓存层类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheLayer {
    /// 背景图层（截图）
    Background,
    /// 遮罩层
    Overlay,
    /// 静态元素层（已完成的绘画元素）
    StaticElements,
    /// 动态元素层（正在绘制的元素）
    DynamicElements,
    /// UI 层
    Ui,
}

impl CacheLayer {
    /// 获取层的渲染顺序（越小越先渲染）
    pub fn z_order(&self) -> u32 {
        match self {
            CacheLayer::Background => 0,
            CacheLayer::Overlay => 1,
            CacheLayer::StaticElements => 2,
            CacheLayer::DynamicElements => 3,
            CacheLayer::Ui => 4,
        }
    }

    /// 获取所有层，按渲染顺序排列
    pub fn all_layers() -> &'static [CacheLayer] {
        &[
            CacheLayer::Background,
            CacheLayer::Overlay,
            CacheLayer::StaticElements,
            CacheLayer::DynamicElements,
            CacheLayer::Ui,
        ]
    }
}

/// 缓存条目状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheState {
    /// 缓存有效
    Valid,
    /// 缓存无效，需要重绘
    Invalid,
    /// 缓存不存在
    Missing,
}

/// 图层缓存管理器
///
/// 管理多个渲染图层的位图缓存。
/// 每个图层可以独立失效和重绘，减少不必要的重复渲染。
#[derive(Debug, Default)]
pub struct LayerCache {
    /// 背景层位图ID
    background: Option<BitmapId>,
    /// 背景层状态
    background_valid: bool,

    /// 遮罩层位图ID
    overlay: Option<BitmapId>,
    /// 遮罩层状态
    overlay_valid: bool,

    /// 静态元素层位图ID
    static_elements: Option<BitmapId>,
    /// 静态元素层状态
    static_elements_valid: bool,

    /// 动态元素层位图ID
    dynamic_elements: Option<BitmapId>,
    /// 动态元素层状态
    dynamic_elements_valid: bool,

    /// UI层位图ID
    ui: Option<BitmapId>,
    /// UI层状态
    ui_valid: bool,

    /// 缓存尺寸
    size: (u32, u32),
}

impl LayerCache {
    /// 创建新的图层缓存管理器
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置缓存尺寸
    ///
    /// 尺寸变化会使所有缓存失效
    pub fn set_size(&mut self, width: u32, height: u32) {
        if self.size != (width, height) {
            self.size = (width, height);
            self.invalidate_all();
        }
    }

    /// 获取缓存尺寸
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// 获取指定层的位图ID
    pub fn get_bitmap(&self, layer: CacheLayer) -> Option<BitmapId> {
        match layer {
            CacheLayer::Background => self.background,
            CacheLayer::Overlay => self.overlay,
            CacheLayer::StaticElements => self.static_elements,
            CacheLayer::DynamicElements => self.dynamic_elements,
            CacheLayer::Ui => self.ui,
        }
    }

    /// 设置指定层的位图ID
    pub fn set_bitmap(&mut self, layer: CacheLayer, bitmap_id: BitmapId) {
        match layer {
            CacheLayer::Background => {
                self.background = Some(bitmap_id);
                self.background_valid = true;
            }
            CacheLayer::Overlay => {
                self.overlay = Some(bitmap_id);
                self.overlay_valid = true;
            }
            CacheLayer::StaticElements => {
                self.static_elements = Some(bitmap_id);
                self.static_elements_valid = true;
            }
            CacheLayer::DynamicElements => {
                self.dynamic_elements = Some(bitmap_id);
                self.dynamic_elements_valid = true;
            }
            CacheLayer::Ui => {
                self.ui = Some(bitmap_id);
                self.ui_valid = true;
            }
        }
    }

    /// 获取指定层的缓存状态
    pub fn get_state(&self, layer: CacheLayer) -> CacheState {
        let (bitmap, valid) = match layer {
            CacheLayer::Background => (self.background, self.background_valid),
            CacheLayer::Overlay => (self.overlay, self.overlay_valid),
            CacheLayer::StaticElements => (self.static_elements, self.static_elements_valid),
            CacheLayer::DynamicElements => (self.dynamic_elements, self.dynamic_elements_valid),
            CacheLayer::Ui => (self.ui, self.ui_valid),
        };

        match (bitmap, valid) {
            (Some(_), true) => CacheState::Valid,
            (Some(_), false) => CacheState::Invalid,
            (None, _) => CacheState::Missing,
        }
    }

    /// 检查指定层是否有效
    pub fn is_valid(&self, layer: CacheLayer) -> bool {
        self.get_state(layer) == CacheState::Valid
    }

    /// 使指定层失效
    pub fn invalidate(&mut self, layer: CacheLayer) {
        match layer {
            CacheLayer::Background => self.background_valid = false,
            CacheLayer::Overlay => self.overlay_valid = false,
            CacheLayer::StaticElements => self.static_elements_valid = false,
            CacheLayer::DynamicElements => self.dynamic_elements_valid = false,
            CacheLayer::Ui => self.ui_valid = false,
        }
    }

    /// 使所有层失效
    pub fn invalidate_all(&mut self) {
        self.background_valid = false;
        self.overlay_valid = false;
        self.static_elements_valid = false;
        self.dynamic_elements_valid = false;
        self.ui_valid = false;
    }

    /// 标记指定层为有效
    pub fn validate(&mut self, layer: CacheLayer) {
        match layer {
            CacheLayer::Background => self.background_valid = true,
            CacheLayer::Overlay => self.overlay_valid = true,
            CacheLayer::StaticElements => self.static_elements_valid = true,
            CacheLayer::DynamicElements => self.dynamic_elements_valid = true,
            CacheLayer::Ui => self.ui_valid = true,
        }
    }

    /// 清除指定层的缓存
    pub fn clear(&mut self, layer: CacheLayer) {
        match layer {
            CacheLayer::Background => {
                self.background = None;
                self.background_valid = false;
            }
            CacheLayer::Overlay => {
                self.overlay = None;
                self.overlay_valid = false;
            }
            CacheLayer::StaticElements => {
                self.static_elements = None;
                self.static_elements_valid = false;
            }
            CacheLayer::DynamicElements => {
                self.dynamic_elements = None;
                self.dynamic_elements_valid = false;
            }
            CacheLayer::Ui => {
                self.ui = None;
                self.ui_valid = false;
            }
        }
    }

    /// 清除所有缓存
    pub fn clear_all(&mut self) {
        self.background = None;
        self.background_valid = false;
        self.overlay = None;
        self.overlay_valid = false;
        self.static_elements = None;
        self.static_elements_valid = false;
        self.dynamic_elements = None;
        self.dynamic_elements_valid = false;
        self.ui = None;
        self.ui_valid = false;
    }

    /// 获取所有无效的层
    pub fn get_invalid_layers(&self) -> Vec<CacheLayer> {
        CacheLayer::all_layers()
            .iter()
            .filter(|&&layer| !self.is_valid(layer))
            .copied()
            .collect()
    }

    /// 获取所有有效的层
    pub fn get_valid_layers(&self) -> Vec<CacheLayer> {
        CacheLayer::all_layers()
            .iter()
            .filter(|&&layer| self.is_valid(layer))
            .copied()
            .collect()
    }

    /// 获取需要渲染的位图ID列表（按z-order排序）
    pub fn get_render_order(&self) -> Vec<(CacheLayer, BitmapId)> {
        let mut result = Vec::new();

        for layer in CacheLayer::all_layers() {
            if let Some(bitmap_id) = self.get_bitmap(*layer)
                && self.is_valid(*layer)
            {
                result.push((*layer, bitmap_id));
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_layer_z_order() {
        assert!(super::CacheLayer::Background.z_order() < super::CacheLayer::Overlay.z_order());
        assert!(super::CacheLayer::Overlay.z_order() < super::CacheLayer::StaticElements.z_order());
        assert!(
            super::CacheLayer::StaticElements.z_order()
                < super::CacheLayer::DynamicElements.z_order()
        );
        assert!(super::CacheLayer::DynamicElements.z_order() < super::CacheLayer::Ui.z_order());
    }

    #[test]
    fn test_cache_state() {
        let mut cache = super::LayerCache::new();

        // 初始状态为 Missing
        assert_eq!(
            cache.get_state(super::CacheLayer::Background),
            super::CacheState::Missing
        );

        // 设置位图后变为 Valid
        cache.set_bitmap(super::CacheLayer::Background, 1);
        assert_eq!(
            cache.get_state(super::CacheLayer::Background),
            super::CacheState::Valid
        );

        // 失效后变为 Invalid
        cache.invalidate(super::CacheLayer::Background);
        assert_eq!(
            cache.get_state(super::CacheLayer::Background),
            super::CacheState::Invalid
        );

        // 验证后变回 Valid
        cache.validate(super::CacheLayer::Background);
        assert_eq!(
            cache.get_state(super::CacheLayer::Background),
            super::CacheState::Valid
        );
    }

    #[test]
    fn test_size_change_invalidates_all() {
        let mut cache = super::LayerCache::new();
        cache.set_size(1920, 1080);

        cache.set_bitmap(super::CacheLayer::Background, 1);
        cache.set_bitmap(super::CacheLayer::StaticElements, 2);

        assert!(cache.is_valid(super::CacheLayer::Background));
        assert!(cache.is_valid(super::CacheLayer::StaticElements));

        // 改变尺寸应该使所有缓存失效
        cache.set_size(1280, 720);

        assert!(!cache.is_valid(super::CacheLayer::Background));
        assert!(!cache.is_valid(super::CacheLayer::StaticElements));
    }

    #[test]
    fn test_get_invalid_layers() {
        let mut cache = super::LayerCache::new();

        cache.set_bitmap(super::CacheLayer::Background, 1);
        cache.set_bitmap(super::CacheLayer::StaticElements, 2);
        cache.invalidate(super::CacheLayer::StaticElements);

        let invalid = cache.get_invalid_layers();

        assert!(!invalid.contains(&super::CacheLayer::Background));
        assert!(invalid.contains(&super::CacheLayer::StaticElements));
        assert!(invalid.contains(&super::CacheLayer::Overlay)); // Missing 也算无效
    }

    #[test]
    fn test_render_order() {
        let mut cache = super::LayerCache::new();

        cache.set_bitmap(super::CacheLayer::Ui, 3);
        cache.set_bitmap(super::CacheLayer::Background, 1);
        cache.set_bitmap(super::CacheLayer::StaticElements, 2);

        let order = cache.get_render_order();

        // 应该按 z-order 排序
        assert_eq!(order.len(), 3);
        assert_eq!(order[0].0, super::CacheLayer::Background);
        assert_eq!(order[1].0, super::CacheLayer::StaticElements);
        assert_eq!(order[2].0, super::CacheLayer::Ui);
    }
}

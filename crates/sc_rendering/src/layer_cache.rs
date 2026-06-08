use crate::types::BitmapId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheLayer {
    Background,
    Overlay,
    StaticElements,
    DynamicElements,
    Ui,
}

impl CacheLayer {
    pub fn z_order(&self) -> u32 {
        match self {
            CacheLayer::Background => 0,
            CacheLayer::Overlay => 1,
            CacheLayer::StaticElements => 2,
            CacheLayer::DynamicElements => 3,
            CacheLayer::Ui => 4,
        }
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheState {
    Valid,
    Invalid,
    Missing,
}

#[derive(Debug, Default)]
pub struct LayerCache {
    background: Option<BitmapId>,
    background_valid: bool,

    overlay: Option<BitmapId>,
    overlay_valid: bool,

    static_elements: Option<BitmapId>,
    static_elements_valid: bool,

    dynamic_elements: Option<BitmapId>,
    dynamic_elements_valid: bool,

    ui: Option<BitmapId>,
    ui_valid: bool,

    size: (u32, u32),
}

impl LayerCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_size(&mut self, width: u32, height: u32) {
        if self.size != (width, height) {
            self.size = (width, height);
            self.invalidate_all();
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn get_bitmap(&self, layer: CacheLayer) -> Option<BitmapId> {
        match layer {
            CacheLayer::Background => self.background,
            CacheLayer::Overlay => self.overlay,
            CacheLayer::StaticElements => self.static_elements,
            CacheLayer::DynamicElements => self.dynamic_elements,
            CacheLayer::Ui => self.ui,
        }
    }

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

    pub fn is_valid(&self, layer: CacheLayer) -> bool {
        self.get_state(layer) == CacheState::Valid
    }

    pub fn invalidate(&mut self, layer: CacheLayer) {
        match layer {
            CacheLayer::Background => self.background_valid = false,
            CacheLayer::Overlay => self.overlay_valid = false,
            CacheLayer::StaticElements => self.static_elements_valid = false,
            CacheLayer::DynamicElements => self.dynamic_elements_valid = false,
            CacheLayer::Ui => self.ui_valid = false,
        }
    }

    pub fn invalidate_all(&mut self) {
        self.background_valid = false;
        self.overlay_valid = false;
        self.static_elements_valid = false;
        self.dynamic_elements_valid = false;
        self.ui_valid = false;
    }

    pub fn validate(&mut self, layer: CacheLayer) {
        match layer {
            CacheLayer::Background => self.background_valid = true,
            CacheLayer::Overlay => self.overlay_valid = true,
            CacheLayer::StaticElements => self.static_elements_valid = true,
            CacheLayer::DynamicElements => self.dynamic_elements_valid = true,
            CacheLayer::Ui => self.ui_valid = true,
        }
    }

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

    pub fn get_invalid_layers(&self) -> Vec<CacheLayer> {
        CacheLayer::all_layers()
            .iter()
            .filter(|&&layer| !self.is_valid(layer))
            .copied()
            .collect()
    }

    pub fn get_valid_layers(&self) -> Vec<CacheLayer> {
        CacheLayer::all_layers()
            .iter()
            .filter(|&&layer| self.is_valid(layer))
            .copied()
            .collect()
    }

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

        assert_eq!(
            cache.get_state(super::CacheLayer::Background),
            super::CacheState::Missing
        );

        cache.set_bitmap(super::CacheLayer::Background, 1);
        assert_eq!(
            cache.get_state(super::CacheLayer::Background),
            super::CacheState::Valid
        );

        cache.invalidate(super::CacheLayer::Background);
        assert_eq!(
            cache.get_state(super::CacheLayer::Background),
            super::CacheState::Invalid
        );

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
        assert!(invalid.contains(&super::CacheLayer::Overlay));
    }

    #[test]
    fn test_render_order() {
        let mut cache = super::LayerCache::new();

        cache.set_bitmap(super::CacheLayer::Ui, 3);
        cache.set_bitmap(super::CacheLayer::Background, 1);
        cache.set_bitmap(super::CacheLayer::StaticElements, 2);

        let order = cache.get_render_order();

        assert_eq!(order.len(), 3);
        assert_eq!(order[0].0, super::CacheLayer::Background);
        assert_eq!(order[1].0, super::CacheLayer::StaticElements);
        assert_eq!(order[2].0, super::CacheLayer::Ui);
    }
}

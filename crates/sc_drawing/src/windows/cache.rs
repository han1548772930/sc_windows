use std::collections::{HashMap, HashSet};
use windows::Win32::Graphics::Direct2D::ID2D1PathGeometry;
use windows::Win32::Graphics::DirectWrite::IDWriteTextLayout;

/// 元素唯一标识符
///
/// 使用 `DrawingElement::id`（u64）作为 key，避免与元素索引混淆。
pub type ElementId = u64;

/// 几何体缓存管理器
///
/// 集中管理所有绘图元素的几何体缓存，提供以下优势：
/// - 统一的缓存失效策略
/// - LRU淘汰机制（可选）
/// - 内存使用监控
pub struct GeometryCache {
    /// 路径几何体缓存
    path_cache: HashMap<ElementId, ID2D1PathGeometry>,
    /// 文本布局缓存
    text_cache: HashMap<ElementId, IDWriteTextLayout>,
    /// 脏标记集合（需要重建缓存的元素）
    dirty_flags: HashSet<ElementId>,
    /// 最大缓存条目数
    max_entries: usize,
    /// 缓存命中计数
    hit_count: u64,
    /// 缓存未命中计数
    miss_count: u64,
}

impl Default for GeometryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl GeometryCache {
    /// 默认最大缓存条目数
    pub const DEFAULT_MAX_ENTRIES: usize = 500;

    /// 创建新的几何体缓存管理器
    pub fn new() -> Self {
        Self {
            path_cache: HashMap::new(),
            text_cache: HashMap::new(),
            dirty_flags: HashSet::new(),
            max_entries: Self::DEFAULT_MAX_ENTRIES,
            hit_count: 0,
            miss_count: 0,
        }
    }

    /// 创建带自定义容量的缓存管理器
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            path_cache: HashMap::with_capacity(max_entries / 2),
            text_cache: HashMap::with_capacity(max_entries / 2),
            dirty_flags: HashSet::new(),
            max_entries,
            hit_count: 0,
            miss_count: 0,
        }
    }

    /// 获取或创建路径几何体
    pub fn get_or_create_path<F>(&mut self, id: ElementId, creator: F) -> Option<&ID2D1PathGeometry>
    where
        F: FnOnce() -> Option<ID2D1PathGeometry>,
    {
        if self.dirty_flags.contains(&id) {
            self.path_cache.remove(&id);
            self.dirty_flags.remove(&id);
        }

        if self.path_cache.contains_key(&id) {
            self.hit_count += 1;
            return self.path_cache.get(&id);
        }

        self.miss_count += 1;
        if let Some(geometry) = creator() {
            self.enforce_capacity_limit();
            self.path_cache.insert(id, geometry);
            return self.path_cache.get(&id);
        }

        None
    }

    /// 获取或创建文本布局
    pub fn get_or_create_text<F>(&mut self, id: ElementId, creator: F) -> Option<&IDWriteTextLayout>
    where
        F: FnOnce() -> Option<IDWriteTextLayout>,
    {
        if self.dirty_flags.contains(&id) {
            self.text_cache.remove(&id);
            self.dirty_flags.remove(&id);
        }

        if self.text_cache.contains_key(&id) {
            self.hit_count += 1;
            return self.text_cache.get(&id);
        }

        self.miss_count += 1;
        if let Some(layout) = creator() {
            self.enforce_capacity_limit();
            self.text_cache.insert(id, layout);
            return self.text_cache.get(&id);
        }

        None
    }

    /// 标记元素为脏（需要重建缓存）
    pub fn mark_dirty(&mut self, id: ElementId) {
        self.dirty_flags.insert(id);
    }

    /// 标记多个元素为脏
    pub fn mark_dirty_batch(&mut self, ids: &[ElementId]) {
        for &id in ids {
            self.dirty_flags.insert(id);
        }
    }

    /// 使所有缓存失效
    pub fn invalidate_all(&mut self) {
        self.path_cache.clear();
        self.text_cache.clear();
        self.dirty_flags.clear();
    }

    /// 移除指定元素的缓存
    pub fn remove(&mut self, id: ElementId) {
        self.path_cache.remove(&id);
        self.text_cache.remove(&id);
        self.dirty_flags.remove(&id);
    }

    /// 批量移除缓存
    pub fn remove_batch(&mut self, ids: &[ElementId]) {
        for &id in ids {
            self.remove(id);
        }
    }

    /// 获取缓存统计信息
    pub fn get_stats(&self) -> CacheStats {
        CacheStats {
            path_count: self.path_cache.len(),
            text_count: self.text_cache.len(),
            dirty_count: self.dirty_flags.len(),
            hit_count: self.hit_count,
            miss_count: self.miss_count,
            hit_rate: if self.hit_count + self.miss_count > 0 {
                self.hit_count as f64 / (self.hit_count + self.miss_count) as f64
            } else {
                0.0
            },
        }
    }

    /// 重置统计计数器
    pub fn reset_stats(&mut self) {
        self.hit_count = 0;
        self.miss_count = 0;
    }

    /// 强制执行容量限制
    fn enforce_capacity_limit(&mut self) {
        let total = self.path_cache.len() + self.text_cache.len();
        if total >= self.max_entries {
            let to_remove = total / 2;

            let path_keys: Vec<_> = self
                .path_cache
                .keys()
                .take(to_remove / 2)
                .copied()
                .collect();
            for key in path_keys {
                self.path_cache.remove(&key);
            }

            let text_keys: Vec<_> = self
                .text_cache
                .keys()
                .take(to_remove / 2)
                .copied()
                .collect();
            for key in text_keys {
                self.text_cache.remove(&key);
            }
        }
    }

    /// 检查路径缓存是否存在
    pub fn has_path(&self, id: ElementId) -> bool {
        self.path_cache.contains_key(&id) && !self.dirty_flags.contains(&id)
    }

    /// 检查文本缓存是否存在
    pub fn has_text(&self, id: ElementId) -> bool {
        self.text_cache.contains_key(&id) && !self.dirty_flags.contains(&id)
    }

    /// 获取已缓存的路径几何体（只读）
    pub fn get_path(&self, id: ElementId) -> Option<&ID2D1PathGeometry> {
        if self.dirty_flags.contains(&id) {
            return None;
        }
        self.path_cache.get(&id)
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub path_count: usize,
    pub text_count: usize,
    pub dirty_count: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub hit_rate: f64,
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GeometryCache: paths={}, texts={}, dirty={}, hits={}, misses={}, rate={:.1}%",
            self.path_count,
            self.text_count,
            self.dirty_count,
            self.hit_count,
            self.miss_count,
            self.hit_rate * 100.0
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_cache_new() {
        let cache = super::GeometryCache::new();
        let stats = cache.get_stats();
        assert_eq!(stats.path_count, 0);
        assert_eq!(stats.text_count, 0);
        assert_eq!(stats.dirty_count, 0);
        assert_eq!(stats.hit_count, 0);
        assert_eq!(stats.miss_count, 0);
    }

    #[test]
    fn test_cache_mark_dirty() {
        let mut cache = super::GeometryCache::new();
        cache.mark_dirty(1);
        cache.mark_dirty(2);
        assert_eq!(cache.get_stats().dirty_count, 2);
    }

    #[test]
    fn test_cache_mark_dirty_batch() {
        let mut cache = super::GeometryCache::new();
        cache.mark_dirty_batch(&[1, 2, 3]);
        assert_eq!(cache.get_stats().dirty_count, 3);
    }

    #[test]
    fn test_cache_invalidate_all() {
        let mut cache = super::GeometryCache::new();
        cache.mark_dirty(1);
        cache.invalidate_all();
        let stats = cache.get_stats();
        assert_eq!(stats.dirty_count, 0);
        assert_eq!(stats.path_count, 0);
        assert_eq!(stats.text_count, 0);
    }

    #[test]
    fn test_cache_has_path() {
        let mut cache = super::GeometryCache::new();
        assert!(!cache.has_path(1));

        cache.mark_dirty(1);
        assert!(!cache.has_path(1));
    }

    #[test]
    fn test_cache_remove_clears_dirty() {
        let mut cache = super::GeometryCache::new();
        cache.mark_dirty(1);
        assert_eq!(cache.get_stats().dirty_count, 1);
        cache.remove(1);
        assert_eq!(cache.get_stats().dirty_count, 0);
    }
}

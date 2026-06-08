use std::collections::{HashMap, HashSet};
use windows::Win32::Graphics::Direct2D::ID2D1PathGeometry;
use windows::Win32::Graphics::DirectWrite::IDWriteTextLayout;

pub type ElementId = u64;

pub struct GeometryCache {
    path_cache: HashMap<ElementId, ID2D1PathGeometry>,
    text_cache: HashMap<ElementId, IDWriteTextLayout>,
    dirty_flags: HashSet<ElementId>,
    max_entries: usize,
    hit_count: u64,
    miss_count: u64,
}

impl Default for GeometryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl GeometryCache {
    pub const DEFAULT_MAX_ENTRIES: usize = 500;

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

    pub fn mark_dirty(&mut self, id: ElementId) {
        self.dirty_flags.insert(id);
    }

    pub fn mark_dirty_batch(&mut self, ids: &[ElementId]) {
        for &id in ids {
            self.dirty_flags.insert(id);
        }
    }

    pub fn invalidate_all(&mut self) {
        self.path_cache.clear();
        self.text_cache.clear();
        self.dirty_flags.clear();
    }

    pub fn remove(&mut self, id: ElementId) {
        self.path_cache.remove(&id);
        self.text_cache.remove(&id);
        self.dirty_flags.remove(&id);
    }

    pub fn remove_batch(&mut self, ids: &[ElementId]) {
        for &id in ids {
            self.remove(id);
        }
    }

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

    pub fn reset_stats(&mut self) {
        self.hit_count = 0;
        self.miss_count = 0;
    }

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

    pub fn has_path(&self, id: ElementId) -> bool {
        self.path_cache.contains_key(&id) && !self.dirty_flags.contains(&id)
    }

    pub fn has_text(&self, id: ElementId) -> bool {
        self.text_cache.contains_key(&id) && !self.dirty_flags.contains(&id)
    }

    pub fn get_path(&self, id: ElementId) -> Option<&ID2D1PathGeometry> {
        if self.dirty_flags.contains(&id) {
            return None;
        }
        self.path_cache.get(&id)
    }
}

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

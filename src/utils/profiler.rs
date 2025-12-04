//! 性能监控模块
//!
//! 提供简单的性能监控工具，用于测量代码段的执行时间。
//!
//! ## 使用示例
//! ```no_run
//! use sc_windows::utils::profiler::Profiler;
//!
//! let mut profiler = Profiler::new();
//!
//! profiler.time("capture_screen", || {
//!     // 执行截屏操作
//! });
//!
//! profiler.report();
//! ```

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 性能监控器
pub struct Profiler {
    spans: HashMap<String, Vec<Duration>>,
    enabled: bool,
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler {
    /// 创建新的性能监控器
    pub fn new() -> Self {
        Self {
            spans: HashMap::new(),
            enabled: cfg!(debug_assertions), // 仅在debug模式下默认启用
        }
    }

    /// 创建一个始终启用的性能监控器
    pub fn new_enabled() -> Self {
        Self {
            spans: HashMap::new(),
            enabled: true,
        }
    }

    /// 启用或禁用性能监控
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 测量函数执行时间
    pub fn time<F, R>(&mut self, name: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        if !self.enabled {
            return f();
        }

        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();

        self.spans
            .entry(name.to_string())
            .or_default()
            .push(duration);

        result
    }

    /// 开始一个计时区间，返回一个Guard对象
    pub fn start_span(&mut self, name: &str) -> SpanGuard<'_> {
        SpanGuard {
            profiler: self,
            name: name.to_string(),
            start: Instant::now(),
        }
    }

    /// 记录一个耗时
    pub fn record(&mut self, name: &str, duration: Duration) {
        if self.enabled {
            self.spans
                .entry(name.to_string())
                .or_default()
                .push(duration);
        }
    }

    /// 获取指定span的统计信息
    pub fn get_stats(&self, name: &str) -> Option<SpanStats> {
        self.spans.get(name).map(|durations| {
            let count = durations.len();
            let total: Duration = durations.iter().sum();
            let avg = if count > 0 {
                total / count as u32
            } else {
                Duration::ZERO
            };
            let min = durations.iter().min().copied().unwrap_or(Duration::ZERO);
            let max = durations.iter().max().copied().unwrap_or(Duration::ZERO);

            SpanStats {
                count,
                total,
                avg,
                min,
                max,
            }
        })
    }

    /// 打印性能报告
    pub fn report(&self) {
        if !self.enabled || self.spans.is_empty() {
            return;
        }

        eprintln!("=== 性能报告 ===");
        
        let mut entries: Vec<_> = self.spans.iter().collect();
        entries.sort_by_key(|(name, _)| name.as_str());

        for (name, _durations) in entries {
            if let Some(stats) = self.get_stats(name) {
                eprintln!(
                    "⏱️ {}: 调用{}次, 平均{:?}, 总计{:?} (最小{:?}, 最大{:?})",
                    name, stats.count, stats.avg, stats.total, stats.min, stats.max
                );
            }
        }
        
        eprintln!("================");
    }

    /// 清除所有记录
    pub fn clear(&mut self) {
        self.spans.clear();
    }

    /// 获取所有span名称
    pub fn span_names(&self) -> Vec<&str> {
        self.spans.keys().map(|s| s.as_str()).collect()
    }
}

/// Span统计信息
#[derive(Debug, Clone)]
pub struct SpanStats {
    pub count: usize,
    pub total: Duration,
    pub avg: Duration,
    pub min: Duration,
    pub max: Duration,
}

/// RAII风格的计时Guard
pub struct SpanGuard<'a> {
    profiler: &'a mut Profiler,
    name: String,
    start: Instant,
}

impl Drop for SpanGuard<'_> {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        if self.profiler.enabled {
            self.profiler
                .spans
                .entry(self.name.clone())
                .or_default()
                .push(duration);
        }
    }
}

/// 简单的计时宏
#[macro_export]
macro_rules! time_it {
    ($name:expr, $body:expr) => {{
        let start = std::time::Instant::now();
        let result = $body;
        let duration = start.elapsed();
        if cfg!(debug_assertions) {
            eprintln!("⏱️ {}: {:?}", $name, duration);
        }
        result
    }};
}

// ==================== 单元测试 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_profiler_basic() {
        let mut profiler = Profiler::new_enabled();

        profiler.time("test_op", || {
            thread::sleep(Duration::from_millis(10));
        });

        let stats = profiler.get_stats("test_op");
        assert!(stats.is_some());
        
        let stats = stats.unwrap();
        assert_eq!(stats.count, 1);
        assert!(stats.avg >= Duration::from_millis(10));
    }

    #[test]
    fn test_profiler_multiple_calls() {
        let mut profiler = Profiler::new_enabled();

        for _ in 0..3 {
            profiler.time("repeated_op", || {
                thread::sleep(Duration::from_millis(5));
            });
        }

        let stats = profiler.get_stats("repeated_op").unwrap();
        assert_eq!(stats.count, 3);
    }

    #[test]
    fn test_profiler_disabled() {
        let mut profiler = Profiler::new_enabled();
        profiler.set_enabled(false);

        profiler.time("disabled_op", || {
            thread::sleep(Duration::from_millis(5));
        });

        assert!(profiler.get_stats("disabled_op").is_none());
    }

    #[test]
    fn test_profiler_clear() {
        let mut profiler = Profiler::new_enabled();

        profiler.time("to_clear", || {});

        assert!(profiler.get_stats("to_clear").is_some());

        profiler.clear();

        assert!(profiler.get_stats("to_clear").is_none());
    }
}

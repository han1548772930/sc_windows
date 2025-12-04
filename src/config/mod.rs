//! 配置管理模块
//!
//! 提供统一的配置管理功能，包括:
//! - 配置加载和缓存
//! - 配置变更通知机制
//! - 线程安全的配置访问

mod manager;

pub use manager::ConfigManager;

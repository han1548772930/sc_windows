use std::sync::OnceLock;

use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::System::Com::*;
use windows::core::*;

/// 全局共享的 Factory 实例
static SHARED_FACTORIES: OnceLock<SharedFactories> = OnceLock::new();

/// D2D 和 DWrite Factory 的共享容器
///
/// 这些 Factory 对象是重量级资源，应该在应用生命周期内全局共享。
/// 使用 OnceLock 确保线程安全的单例初始化。
pub struct SharedFactories {
    d2d_factory: ID2D1Factory,
    dwrite_factory: IDWriteFactory,
}

// SAFETY: ID2D1Factory 和 IDWriteFactory 是 COM 对象，
// 它们的线程安全性由 COM 的单元模型保证。
// 我们使用 COINIT_APARTMENTTHREADED 初始化，
// 且这些 Factory 在创建后是只读的。
unsafe impl Send for SharedFactories {}
unsafe impl Sync for SharedFactories {}

impl SharedFactories {
    /// 获取全局共享的 Factory 实例
    ///
    /// 首次调用时会初始化 COM 和创建 Factory 对象。
    /// 如果初始化失败，返回 None。
    pub fn get() -> Option<&'static SharedFactories> {
        SHARED_FACTORIES.get_or_init(|| {
            Self::create().unwrap_or_else(|e| {
                eprintln!("Failed to create SharedFactories: {:?}", e);
                panic!("SharedFactories initialization failed");
            })
        });
        SHARED_FACTORIES.get()
    }

    /// 尝试获取全局共享的 Factory 实例（不 panic）
    pub fn try_get() -> Option<&'static SharedFactories> {
        if let Some(factories) = SHARED_FACTORIES.get() {
            return Some(factories);
        }

        // 尝试初始化
        match Self::create() {
            Ok(factories) => {
                let _ = SHARED_FACTORIES.set(factories);
                SHARED_FACTORIES.get()
            }
            Err(e) => {
                eprintln!("Failed to create SharedFactories: {:?}", e);
                None
            }
        }
    }

    /// 创建新的 Factory 实例
    fn create() -> Result<Self> {
        // SAFETY: COM 初始化是安全的操作
        // 如果已经初始化会返回 RPC_E_CHANGED_MODE (0x80010106)，这是可接受的
        unsafe {
            let result = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            // RPC_E_CHANGED_MODE = 0x80010106
            const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x80010106_u32 as i32);
            if result.is_err() && result != RPC_E_CHANGED_MODE {
                return Err(Error::new(E_FAIL, "COM initialization failed"));
            }
        }

        // SAFETY: D2D1CreateFactory 是 Windows API 的安全封装
        let d2d_factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };

        // SAFETY: DWriteCreateFactory 是 Windows API 的安全封装
        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };

        Ok(Self {
            d2d_factory,
            dwrite_factory,
        })
    }

    /// 获取 D2D Factory 引用
    #[inline]
    pub fn d2d_factory(&self) -> &ID2D1Factory {
        &self.d2d_factory
    }

    /// 获取 DWrite Factory 引用
    #[inline]
    pub fn dwrite_factory(&self) -> &IDWriteFactory {
        &self.dwrite_factory
    }

    /// 克隆 D2D Factory（COM 引用计数）
    #[inline]
    pub fn d2d_factory_clone(&self) -> ID2D1Factory {
        self.d2d_factory.clone()
    }

    /// 克隆 DWrite Factory（COM 引用计数）
    #[inline]
    pub fn dwrite_factory_clone(&self) -> IDWriteFactory {
        self.dwrite_factory.clone()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_shared_factories_singleton() {
        let f1 = super::SharedFactories::try_get();
        let f2 = super::SharedFactories::try_get();

        // 应该是同一个实例
        if let (Some(f1), Some(f2)) = (f1, f2) {
            assert!(std::ptr::eq(f1, f2));
        }
    }
}

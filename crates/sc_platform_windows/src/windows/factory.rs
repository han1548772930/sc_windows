use std::sync::OnceLock;

use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::System::Com::*;
use windows::core::*;

static SHARED_FACTORIES: OnceLock<SharedFactories> = OnceLock::new();

pub struct SharedFactories {
    d2d_factory: ID2D1Factory,
    dwrite_factory: IDWriteFactory,
}

unsafe impl Send for SharedFactories {}
unsafe impl Sync for SharedFactories {}

impl SharedFactories {
    pub fn get() -> Option<&'static SharedFactories> {
        SHARED_FACTORIES.get_or_init(|| {
            Self::create().unwrap_or_else(|e| {
                eprintln!("Failed to create SharedFactories: {:?}", e);
                panic!("SharedFactories initialization failed");
            })
        });
        SHARED_FACTORIES.get()
    }

    pub fn try_get() -> Option<&'static SharedFactories> {
        if let Some(factories) = SHARED_FACTORIES.get() {
            return Some(factories);
        }

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

    fn create() -> Result<Self> {
        unsafe {
            let result = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            // RPC_E_CHANGED_MODE = 0x80010106
            const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x80010106_u32 as i32);
            if result.is_err() && result != RPC_E_CHANGED_MODE {
                return Err(Error::new(E_FAIL, "COM initialization failed"));
            }
        }

        let d2d_factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_MULTI_THREADED, None)? };

        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };

        Ok(Self {
            d2d_factory,
            dwrite_factory,
        })
    }

    #[inline]
    pub fn d2d_factory(&self) -> &ID2D1Factory {
        &self.d2d_factory
    }

    #[inline]
    pub fn dwrite_factory(&self) -> &IDWriteFactory {
        &self.dwrite_factory
    }

    #[inline]
    pub fn d2d_factory_clone(&self) -> ID2D1Factory {
        self.d2d_factory.clone()
    }

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

        if let (Some(f1), Some(f2)) = (f1, f2) {
            assert!(std::ptr::eq(f1, f2));
        }
    }
}

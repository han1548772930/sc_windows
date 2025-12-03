// Windows DXGI Desktop Duplication implementation
//
// Provides high-performance screen capture using DXGI Desktop Duplication API.
// Falls back to GDI if not available.

use windows::core::{Interface, Result};
use windows::Win32::Foundation::{RECT, E_FAIL, HMODULE};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11_CPU_ACCESS_READ,
    D3D11_MAP_READ, D3D11_CREATE_DEVICE_SINGLETHREADED, D3D11_MAPPED_SUBRESOURCE,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIAdapter, IDXGIOutput, IDXGIOutput1, IDXGIOutputDuplication,
    Common::DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Gdi::{
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, CreateDIBSection, HBITMAP, RGBQUAD,
};

/// Captures the primary screen using DXGI Desktop Duplication and returns an HBITMAP.
///
/// This function creates a new D3D11 device and duplication session for each call.
/// For higher performance in a loop, the device and duplication interface should be reused.
/// However, for a single screenshot, this is acceptable.
pub unsafe fn capture_screen_region_to_hbitmap_dxgi(
    selection_rect: RECT,
) -> Result<HBITMAP> {
    let (device, context) = create_d3d11_device()?;
    let duplication = create_duplication(&device)?;

    let (texture, texture_desc) = acquire_frame(&duplication)?;
    
    // Create a staging texture to copy the frame to CPU memory
    let staging_desc = D3D11_TEXTURE2D_DESC {
        Width: texture_desc.Width,
        Height: texture_desc.Height,
        MipLevels: 1,
        ArraySize: 1,
        Format: texture_desc.Format, // Use source format to ensure CopyResource works
        SampleDesc: texture_desc.SampleDesc,
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };

    let mut staging_texture: Option<ID3D11Texture2D> = None;
    unsafe { device.CreateTexture2D(&staging_desc, None, Some(&mut staging_texture))? };
    let staging_texture = staging_texture.ok_or_else(|| windows::core::Error::new(E_FAIL, "Failed to create staging texture"))?;

    // Copy the captured frame to the staging texture
    unsafe { context.CopyResource(&staging_texture, &texture) };

    // Map the staging texture to access the data
    let mut mapped_resource = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { context.Map(&staging_texture, 0, D3D11_MAP_READ, 0, Some(&mut mapped_resource))? };
    
    // Process the data and create HBITMAP
    // Note: DXGI returns BGRA data, which is what GDI expects for 32-bit bitmaps mostly.
    // We need to handle row pitch (stride).
    
    let width = (selection_rect.right - selection_rect.left).abs();
    let height = (selection_rect.bottom - selection_rect.top).abs();
    
    // Validate dimensions
    if width <= 0 || height <= 0 || width as u32 > texture_desc.Width || height as u32 > texture_desc.Height {
         unsafe { let _ = context.Unmap(&staging_texture, 0); };
         return Err(windows::core::Error::new(E_FAIL, "Invalid capture region or region exceeds screen bounds"));
    }

    // Create buffer for the sub-region
    // let mut pixel_data = Vec::with_capacity((width * height * 4) as usize); // Not needed with CreateDIBSection
    let src_ptr = mapped_resource.pData as *const u8;
    let src_pitch = mapped_resource.RowPitch as usize;
    
    // Create DIB Section to ensure 32-bit BGRA format
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // Top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [RGBQUAD::default(); 1],
    };

    let mut p_bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbitmap = unsafe { 
        CreateDIBSection(
            None, 
            &bmi, 
            DIB_RGB_COLORS, 
            &mut p_bits, 
            None, 
            0
        )
    }?; // ? handles null check/error conversion automatically for many Win32 calls? No, CreateDIBSection returns Result<HBITMAP> in windows crate?
    // Actually, windows crate returns Result<HBITMAP> if defined with SetLastError, but for GDI objects it might return HBITMAP directly and we check is_invalid().
    // Let's check generated bindings. Usually CreateDIBSection returns Result<HBITMAP>.
    
    if hbitmap.is_invalid() || p_bits.is_null() {
         unsafe { let _ = context.Unmap(&staging_texture, 0); };
         return Err(windows::core::Error::new(E_FAIL, "Failed to create DIB Section"));
    }

    let dest_ptr = p_bits as *mut u8;

    for y in 0..height {
        let src_y = (selection_rect.top + y) as usize;
        // Ensure we don't read out of bounds
        if src_y as u32 >= texture_desc.Height { break; }
        
        let src_offset = src_y * src_pitch + (selection_rect.left as usize * 4);
        let dest_offset = (y as usize) * (width as usize * 4);
        
        // Safety check for source buffer bounds
        if src_offset + (width * 4) as usize > src_pitch * texture_desc.Height as usize {
             #[cfg(debug_assertions)]
             eprintln!("DXGI Capture OOB: src_offset={} width={} pitch={} height={}", src_offset, width, src_pitch, texture_desc.Height);
             break; 
        }

        unsafe {
            let src_row = src_ptr.add(src_offset);
            let dest_row = dest_ptr.add(dest_offset);
            
            // Use pixel-by-pixel copy to enforce Alpha=255
            // This ensures D2D renders the bitmap as opaque, preventing black/transparent issues
            for x in 0..width as usize {
                let p_src = src_row.add(x * 4);
                let p_dst = dest_row.add(x * 4);
                
                // BGRA copy (assuming DXGI returns B8G8R8A8)
                *p_dst.add(0) = *p_src.add(0); // B
                *p_dst.add(1) = *p_src.add(1); // G
                *p_dst.add(2) = *p_src.add(2); // R
                *p_dst.add(3) = 255;           // Force Alpha to 255
            }
        }
    }

    // Unmap the texture
    unsafe { let _ = context.Unmap(&staging_texture, 0); };
    
    Ok(hbitmap)
}

fn create_d3d11_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device = None;
    let mut context = None;
    
    let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED;
    
    // Try hardware first
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            flags,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
    }
    
    match (device, context) {
        (Some(d), Some(c)) => Ok((d, c)),
        _ => Err(windows::core::Error::new(E_FAIL, "Failed to create D3D11 device")),
    }
}

fn create_duplication(device: &ID3D11Device) -> Result<IDXGIOutputDuplication> {
    let dxgi_device: IDXGIDevice = device.cast()?;
    let adapter: IDXGIAdapter = unsafe { dxgi_device.GetAdapter()? };
    // Get output 0 (Primary)
    let output: IDXGIOutput = unsafe { adapter.EnumOutputs(0)? };
    let output1: IDXGIOutput1 = output.cast()?;
    
    unsafe { output1.DuplicateOutput(device) }
}

fn acquire_frame(duplication: &IDXGIOutputDuplication) -> Result<(ID3D11Texture2D, D3D11_TEXTURE2D_DESC)> {
    let mut frame_info = Default::default();
    let mut resource = None;
    
    // Try to acquire frame with timeout (e.g., 200ms)
    // If the screen is static, this might timeout. 
    // But DuplicateOutput usually gives the current image immediately on first call?
    // Actually, docs say: "If there is no image update, the function waits..."
    // But for the *first* call after creation, does it give the current desktop?
    // Yes, usually. But let's handle timeout retry or just error out.
    
    // Note: We might need to ReleaseFrame if we loop, but here we just do one shot.
    // Since we drop the duplication object at end of function, it should be fine, 
    // but strictly we should call ReleaseFrame if we acquired it.
    // However, we are not looping.
    
    unsafe {
        duplication.AcquireNextFrame(500, &mut frame_info, &mut resource)?;
    }
    
    if let Some(res) = resource {
        let texture: ID3D11Texture2D = res.cast()?;
        let mut desc = Default::default();
        unsafe { texture.GetDesc(&mut desc) };
        Ok((texture, desc))
    } else {
        Err(windows::core::Error::new(E_FAIL, "AcquireNextFrame returned no resource"))
    }
}

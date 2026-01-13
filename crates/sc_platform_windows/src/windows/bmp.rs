use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, GetDIBits, HBITMAP, HDC, RGBQUAD,
};
use windows::core::{Error, HRESULT, Result};

fn err(msg: &str) -> Error {
    Error::new(HRESULT(-1), msg)
}

/// 从原图中提取指定区域的 BMP 图像数据。
///
/// 输入与输出都是完整的 BMP 文件字节（包含 14-byte 文件头 + 40-byte 信息头）。
pub fn crop_bmp(original_image_data: &[u8], crop_rect: &sc_drawing::Rect) -> Result<Vec<u8>> {
    // BMP header is 14 bytes, DIB header starts at offset 14. We rely on a 40-byte BITMAPINFOHEADER.
    if original_image_data.len() < 54 {
        return Err(err("BMP data too small"));
    }

    // Read width/height from BITMAPINFOHEADER.
    let width = i32::from_le_bytes([
        original_image_data[18],
        original_image_data[19],
        original_image_data[20],
        original_image_data[21],
    ]);
    let height = i32::from_le_bytes([
        original_image_data[22],
        original_image_data[23],
        original_image_data[24],
        original_image_data[25],
    ])
    .abs();

    let bits_per_pixel = u16::from_le_bytes([original_image_data[28], original_image_data[29]]);

    // Row size is 4-byte aligned.
    let bytes_per_pixel = (bits_per_pixel / 8) as i32;
    let row_size = ((width * bytes_per_pixel + 3) / 4) * 4;

    // Compute crop region.
    let crop_x = crop_rect.left.max(0).min(width - 1);
    let crop_y = crop_rect.top.max(0).min(height - 1);
    let crop_width = (crop_rect.right - crop_rect.left)
        .max(1)
        .min(width - crop_x);
    let crop_height = (crop_rect.bottom - crop_rect.top)
        .max(1)
        .min(height - crop_y);

    // If crop region is invalid, return the original image.
    if crop_width <= 0 || crop_height <= 0 {
        return Ok(original_image_data.to_vec());
    }

    // Build a new BMP header.
    let new_row_size = ((crop_width * bytes_per_pixel + 3) / 4) * 4;
    let new_image_size = new_row_size * crop_height;
    let new_file_size = 54 + new_image_size;

    let mut new_bmp = Vec::with_capacity(new_file_size as usize);

    // Copy and patch header.
    new_bmp.extend_from_slice(&original_image_data[0..18]);
    new_bmp.extend_from_slice(&crop_width.to_le_bytes());
    new_bmp.extend_from_slice(&(-crop_height).to_le_bytes());
    new_bmp.extend_from_slice(&original_image_data[26..54]);

    // Patch file size.
    new_bmp[2..6].copy_from_slice(&(new_file_size as u32).to_le_bytes());
    // Patch image size.
    new_bmp[34..38].copy_from_slice(&(new_image_size as u32).to_le_bytes());

    // Copy pixel data.
    let pixel_data_offset = 54;
    for y in 0..crop_height {
        let src_y = crop_y + y;
        let src_row_start = pixel_data_offset + (src_y * row_size) as usize;
        let src_pixel_start = src_row_start + (crop_x * bytes_per_pixel) as usize;
        let src_pixel_end = src_pixel_start + (crop_width * bytes_per_pixel) as usize;

        if src_pixel_end <= original_image_data.len() {
            new_bmp.extend_from_slice(&original_image_data[src_pixel_start..src_pixel_end]);

            // Row padding.
            let padding = (new_row_size - crop_width * bytes_per_pixel) as usize;
            new_bmp.resize(new_bmp.len() + padding, 0);
        }
    }

    Ok(new_bmp)
}

/// Read pixels from a Win32 `HBITMAP` and return BMP file bytes.
///
/// The returned bytes include a BMP file header + BITMAPINFOHEADER + pixel data (32bpp BGRA).
pub fn bitmap_to_bmp_data(
    mem_dc: HDC,
    bitmap: HBITMAP,
    width: i32,
    height: i32,
) -> Result<Vec<u8>> {
    unsafe {
        // Bitmap info.
        let mut bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // Top-down.
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

        let data_size = (width * height * 4) as usize;
        let mut pixel_data = vec![0u8; data_size];

        let result = GetDIBits(
            mem_dc,
            bitmap,
            0,
            height as u32,
            Some(pixel_data.as_mut_ptr() as *mut _),
            &mut bitmap_info,
            DIB_RGB_COLORS,
        );

        if result == 0 {
            return Err(err("GetDIBits failed"));
        }

        // Build BMP file header.
        let file_size = 54 + data_size as u32;
        let mut bmp_data = Vec::with_capacity(file_size as usize);

        // BMP file header (14 bytes).
        bmp_data.extend_from_slice(b"BM");
        bmp_data.extend_from_slice(&file_size.to_le_bytes());
        bmp_data.extend_from_slice(&[0u8; 4]);
        bmp_data.extend_from_slice(&54u32.to_le_bytes());

        // BITMAPINFOHEADER (40 bytes).
        bmp_data.extend_from_slice(&40u32.to_le_bytes());
        bmp_data.extend_from_slice(&width.to_le_bytes());
        bmp_data.extend_from_slice(&(-height).to_le_bytes());
        bmp_data.extend_from_slice(&1u16.to_le_bytes());
        bmp_data.extend_from_slice(&32u16.to_le_bytes());
        bmp_data.extend_from_slice(&[0u8; 24]);

        bmp_data.extend_from_slice(&pixel_data);
        Ok(bmp_data)
    }
}

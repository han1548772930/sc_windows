//! 图像处理工具函数
//!
//! 提供通用的图像裁剪、格式转换等功能。
//! 这些函数与具体业务（如 OCR）解耦，可被多个模块复用。

use anyhow::Result;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::*;

/// 从原图中提取指定区域的图片数据
///
/// # 参数
/// - `original_image_data`: 原始 BMP 图像数据
/// - `crop_rect`: 裁剪区域
///
/// # 返回
/// 裁剪后的 BMP 图像数据
pub fn crop_bmp(original_image_data: &[u8], crop_rect: &RECT) -> Result<Vec<u8>> {
    // 解析 BMP 头部信息
    if original_image_data.len() < 54 {
        return Err(anyhow::anyhow!("BMP 数据太小"));
    }

    // 读取 BMP 头部信息
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

    // 计算每行的字节数（需要4字节对齐）
    let bytes_per_pixel = (bits_per_pixel / 8) as i32;
    let row_size = ((width * bytes_per_pixel + 3) / 4) * 4;

    // 计算裁剪区域
    let crop_x = crop_rect.left.max(0).min(width - 1);
    let crop_y = crop_rect.top.max(0).min(height - 1);
    let crop_width = (crop_rect.right - crop_rect.left)
        .max(1)
        .min(width - crop_x);
    let crop_height = (crop_rect.bottom - crop_rect.top)
        .max(1)
        .min(height - crop_y);

    // 如果裁剪区域无效，返回原图
    if crop_width <= 0 || crop_height <= 0 {
        return Ok(original_image_data.to_vec());
    }

    // 创建新的 BMP 头部
    let new_row_size = ((crop_width * bytes_per_pixel + 3) / 4) * 4;
    let new_image_size = new_row_size * crop_height;
    let new_file_size = 54 + new_image_size;

    let mut new_bmp = Vec::with_capacity(new_file_size as usize);

    // 复制并修改 BMP 头部
    new_bmp.extend_from_slice(&original_image_data[0..18]); // 文件头
    new_bmp.extend_from_slice(&crop_width.to_le_bytes()); // 新宽度
    new_bmp.extend_from_slice(&(-crop_height).to_le_bytes()); // 新高度 (保持负值，Top-Down)
    new_bmp.extend_from_slice(&original_image_data[26..54]); // 其余头部信息

    // 修改文件大小
    new_bmp[2..6].copy_from_slice(&(new_file_size as u32).to_le_bytes());
    // 修改图像数据大小
    new_bmp[34..38].copy_from_slice(&(new_image_size as u32).to_le_bytes());

    // 复制像素数据
    let pixel_data_offset = 54;

    for y in 0..crop_height {
        let src_y = crop_y + y;
        let src_row_start = pixel_data_offset + (src_y * row_size) as usize;
        let src_pixel_start = src_row_start + (crop_x * bytes_per_pixel) as usize;
        let src_pixel_end = src_pixel_start + (crop_width * bytes_per_pixel) as usize;

        if src_pixel_end <= original_image_data.len() {
            new_bmp.extend_from_slice(&original_image_data[src_pixel_start..src_pixel_end]);

            // 添加行填充
            let padding = (new_row_size - crop_width * bytes_per_pixel) as usize;
            new_bmp.resize(new_bmp.len() + padding, 0);
        }
    }

    Ok(new_bmp)
}

/// 将位图转换为 BMP 数据
///
/// # 参数
/// - `mem_dc`: 内存设备上下文
/// - `bitmap`: 位图句柄
/// - `width`: 图像宽度
/// - `height`: 图像高度
///
/// # 返回
/// BMP 格式的图像数据
pub fn bitmap_to_bmp_data(
    mem_dc: HDC,
    bitmap: HBITMAP,
    width: i32,
    height: i32,
) -> Result<Vec<u8>> {
    unsafe {
        // 获取位图信息
        let mut bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // 负值表示自顶向下
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

        // 计算图像数据大小
        let data_size = (width * height * 4) as usize;
        let mut pixel_data = vec![0u8; data_size];

        // 获取位图数据
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
            return Err(anyhow::anyhow!("获取位图数据失败"));
        }

        // 将 BGRA 数据转换为简单的 BMP 格式
        // 创建 BMP 文件头
        let file_size = 54 + data_size as u32; // BMP 头部 + 数据
        let mut bmp_data = Vec::with_capacity(file_size as usize);

        // BMP 文件头 (14 字节)
        bmp_data.extend_from_slice(b"BM"); // 签名
        bmp_data.extend_from_slice(&file_size.to_le_bytes()); // 文件大小
        bmp_data.extend_from_slice(&[0u8; 4]); // 保留字段
        bmp_data.extend_from_slice(&54u32.to_le_bytes()); // 数据偏移

        // BMP 信息头 (40 字节)
        bmp_data.extend_from_slice(&40u32.to_le_bytes()); // 信息头大小
        bmp_data.extend_from_slice(&width.to_le_bytes()); // 宽度
        bmp_data.extend_from_slice(&(-height).to_le_bytes()); // 高度（负值，表示自顶向下，与 GetDIBits 一致）
        bmp_data.extend_from_slice(&1u16.to_le_bytes()); // 平面数
        bmp_data.extend_from_slice(&32u16.to_le_bytes()); // 位深度
        bmp_data.extend_from_slice(&[0u8; 24]); // 其他字段填充为 0

        // 添加像素数据
        bmp_data.extend_from_slice(&pixel_data);

        Ok(bmp_data)
    }
}

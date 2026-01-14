use anyhow::Result;

use super::window::PreviewWindowState;

impl PreviewWindowState {
    pub(super) fn parse_bmp_data(bmp_data: &[u8]) -> Result<(Vec<u8>, i32, i32)> {
        if bmp_data.len() < 54 {
            return Err(anyhow::anyhow!("BMP 数据太小"));
        }
        if bmp_data[0] != b'B' || bmp_data[1] != b'M' {
            return Err(anyhow::anyhow!("不是有效的BMP文件"));
        }

        let data_offset =
            u32::from_le_bytes([bmp_data[10], bmp_data[11], bmp_data[12], bmp_data[13]]) as usize;
        let width = i32::from_le_bytes([bmp_data[18], bmp_data[19], bmp_data[20], bmp_data[21]]);
        let height_raw =
            i32::from_le_bytes([bmp_data[22], bmp_data[23], bmp_data[24], bmp_data[25]]);
        let height = height_raw.abs();
        let is_top_down = height_raw < 0;
        let bit_count = u16::from_le_bytes([bmp_data[28], bmp_data[29]]);

        if data_offset >= bmp_data.len() {
            return Err(anyhow::anyhow!("BMP数据偏移量无效"));
        }

        let pixel_data = &bmp_data[data_offset..];
        let bytes_per_pixel = (bit_count / 8) as usize;
        let row_size = (width as usize * bytes_per_pixel).div_ceil(4) * 4;

        let mut rgba_pixels = vec![0u8; (width * height * 4) as usize];

        for y in 0..height {
            let src_y = if is_top_down { y } else { height - 1 - y };
            let src_row_start = src_y as usize * row_size;
            for x in 0..width {
                let src_idx = src_row_start + x as usize * bytes_per_pixel;
                let dst_idx = (y * width + x) as usize * 4;
                if src_idx + bytes_per_pixel <= pixel_data.len() && dst_idx + 3 < rgba_pixels.len()
                {
                    match bit_count {
                        24 => {
                            rgba_pixels[dst_idx] = pixel_data[src_idx + 2];
                            rgba_pixels[dst_idx + 1] = pixel_data[src_idx + 1];
                            rgba_pixels[dst_idx + 2] = pixel_data[src_idx];
                            rgba_pixels[dst_idx + 3] = 255;
                        }
                        32 => {
                            rgba_pixels[dst_idx] = pixel_data[src_idx + 2];
                            rgba_pixels[dst_idx + 1] = pixel_data[src_idx + 1];
                            rgba_pixels[dst_idx + 2] = pixel_data[src_idx];
                            rgba_pixels[dst_idx + 3] = 255;
                        }
                        _ => {
                            rgba_pixels[dst_idx] = 255;
                            rgba_pixels[dst_idx + 1] = 255;
                            rgba_pixels[dst_idx + 2] = 255;
                            rgba_pixels[dst_idx + 3] = 255;
                        }
                    }
                }
            }
        }

        Ok((rgba_pixels, width, height))
    }
}

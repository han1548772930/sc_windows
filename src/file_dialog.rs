use windows::{
    Win32::{Foundation::*, UI::Controls::Dialogs::*},
    core::*,
};

use crate::utils::to_wide_chars;

/// 显示保存文件对话框
pub fn show_save_file_dialog(hwnd: HWND, title: &str, default_filename: &str) -> Option<String> {
    unsafe {
        // 准备文件名缓冲区
        let mut file_name = [0u16; 260]; // MAX_PATH

        // 设置默认文件名
        if !default_filename.is_empty() {
            let default_wide = to_wide_chars(default_filename);
            let copy_len = (default_wide.len() - 1).min(file_name.len() - 1); // -1 for null terminator
            file_name[..copy_len].copy_from_slice(&default_wide[..copy_len]);
        }

        // 准备过滤器字符串
        let filter_str =
            "PNG 图片\0*.png\0JPEG 图片\0*.jpg;*.jpeg\0BMP 图片\0*.bmp\0所有文件\0*.*\0\0";
        let filter_wide = to_wide_chars(filter_str);

        // 准备标题
        let title_wide = to_wide_chars(title);

        // 设置OPENFILENAME结构
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: hwnd,
            lpstrFilter: PCWSTR(filter_wide.as_ptr()),
            lpstrFile: PWSTR(file_name.as_mut_ptr()),
            nMaxFile: file_name.len() as u32,
            lpstrTitle: PCWSTR(title_wide.as_ptr()),
            Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY,
            lpstrDefExt: PCWSTR(to_wide_chars("png").as_ptr()),
            nFilterIndex: 1, // 默认选择第一个过滤器（PNG）
            ..Default::default()
        };

        // 显示保存文件对话框
        if GetSaveFileNameW(&mut ofn).as_bool() {
            // 用户选择了文件，转换为Rust字符串
            let file_path = PWSTR(file_name.as_mut_ptr()).to_string().ok()?;
            Some(file_path)
        } else {
            // 用户取消了对话框
            None
        }
    }
}

/// 显示图片保存对话框的便捷函数
pub fn show_image_save_dialog(hwnd: HWND, default_filename: &str) -> Option<String> {
    show_save_file_dialog(hwnd, "保存图片", default_filename)
}

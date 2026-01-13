use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM},
        Graphics::Gdi::LOGFONTW,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            CoCreateInstance, CoInitializeEx, CoUninitialize,
        },
        UI::{
            Controls::Dialogs::*,
            Shell::{
                FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog,
                SIGDN_FILESYSPATH,
            },
        },
    },
    core::{PCWSTR, PWSTR},
};

use crate::win_api::to_wide_chars;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderPickerOutcome {
    Selected(String),
    /// Dialog was shown but the user cancelled, or the dialog failed after creation.
    NotSelected,
    /// Dialog could not be created (host may choose to fall back to another UI).
    Unavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontDialogSelection {
    pub font_size: f32,
    pub font_weight: i32,
    pub font_italic: bool,
    pub font_underline: bool,
    pub font_strikeout: bool,
    pub font_name: String,
    pub font_color: (u8, u8, u8),
}

/// Show a folder picker dialog.
///
/// Notes:
/// - `Unavailable` matches the host's legacy behavior where we only fall back when the COM dialog
///   cannot be created.
pub fn show_folder_picker_dialog(hwnd: HWND, title: &str) -> FolderPickerOutcome {
    unsafe {
        // Best-effort COM init: if it fails (e.g. different apartment already set), we still try to
        // create/show the dialog. Only call CoUninitialize if initialization succeeded.
        let mut com_inited = false;
        if CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).is_ok() {
            com_inited = true;
        }

        let outcome = (|| {
            let Ok(folder_dialog) =
                CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            else {
                return FolderPickerOutcome::Unavailable;
            };

            let _ = folder_dialog.SetOptions(FOS_PICKFOLDERS | FOS_PATHMUSTEXIST);

            let title_wide = to_wide_chars(title);
            let _ = folder_dialog.SetTitle(PCWSTR(title_wide.as_ptr()));

            if folder_dialog.Show(Some(hwnd)).is_ok()
                && let Ok(result) = folder_dialog.GetResult()
                && let Ok(path) = result.GetDisplayName(SIGDN_FILESYSPATH)
                && let Ok(path_str) = path.to_string()
            {
                FolderPickerOutcome::Selected(path_str)
            } else {
                FolderPickerOutcome::NotSelected
            }
        })();

        if com_inited {
            CoUninitialize();
        }

        outcome
    }
}

/// Show a Win32 ChooseFont dialog.
///
/// Returns `Some(selection)` when the user confirms; `None` when cancelled.
pub fn show_font_dialog(
    hwnd: HWND,
    font_size: f32,
    font_weight: i32,
    font_italic: bool,
    font_underline: bool,
    font_strikeout: bool,
    font_name: &str,
    font_color: (u8, u8, u8),
) -> Option<FontDialogSelection> {
    unsafe {
        // Create LOGFONTW
        let mut log_font = LOGFONTW::default();

        log_font.lfHeight = -(font_size as i32);
        log_font.lfWeight = font_weight;
        log_font.lfItalic = if font_italic { 1 } else { 0 };
        log_font.lfUnderline = if font_underline { 1 } else { 0 };
        log_font.lfStrikeOut = if font_strikeout { 1 } else { 0 };

        // Copy face name (keep legacy semantics to avoid behavior change).
        let font_name_wide = to_wide_chars(font_name);
        let copy_len = std::cmp::min(font_name_wide.len(), 31); // LF_FACESIZE - 1
        for i in 0..copy_len {
            log_font.lfFaceName[i] = font_name_wide[i];
        }

        // CHOOSEFONTW
        let mut choose_font = CHOOSEFONTW::default();
        choose_font.lStructSize = std::mem::size_of::<CHOOSEFONTW>() as u32;
        choose_font.hwndOwner = hwnd;
        choose_font.lpLogFont = &mut log_font;
        choose_font.Flags = CF_EFFECTS | CF_SCREENFONTS | CF_INITTOLOGFONTSTRUCT;
        choose_font.rgbColors = COLORREF(
            (font_color.0 as u32) | ((font_color.1 as u32) << 8) | ((font_color.2 as u32) << 16),
        );

        if !ChooseFontW(&mut choose_font).as_bool() {
            return None;
        }

        // Selected font color
        let color_value = choose_font.rgbColors.0;
        let selected_color = (
            (color_value & 0xFF) as u8,
            ((color_value >> 8) & 0xFF) as u8,
            ((color_value >> 16) & 0xFF) as u8,
        );

        // Selected face name (keep legacy semantics to avoid behavior change).
        let mut selected_name = String::new();
        for &ch in &log_font.lfFaceName {
            if ch == 0 {
                break;
            }
            selected_name.push(char::from_u32(ch as u32).unwrap_or('?'));
        }

        Some(FontDialogSelection {
            font_size: (-log_font.lfHeight) as f32,
            font_weight: log_font.lfWeight,
            font_italic: log_font.lfItalic != 0,
            font_underline: log_font.lfUnderline != 0,
            font_strikeout: log_font.lfStrikeOut != 0,
            font_name: selected_name,
            font_color: selected_color,
        })
    }
}

/// Show a Win32 ChooseColor dialog.
///
/// Returns `Some((r,g,b))` when the user confirms; `None` when cancelled.
pub fn show_color_dialog(hwnd: HWND, initial_color: (u8, u8, u8)) -> Option<(u8, u8, u8)> {
    unsafe {
        // Custom colors array must stay alive for the duration of the call.
        let mut custom_colors = [COLORREF(0); 16];

        let mut cc = CHOOSECOLORW {
            lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
            hwndOwner: hwnd,
            hInstance: HWND::default(),
            rgbResult: COLORREF(
                (initial_color.0 as u32)
                    | ((initial_color.1 as u32) << 8)
                    | ((initial_color.2 as u32) << 16),
            ),
            lpCustColors: custom_colors.as_mut_ptr(),
            Flags: CC_FULLOPEN | CC_RGBINIT,
            lCustData: LPARAM(0),
            lpfnHook: None,
            lpTemplateName: PCWSTR::null(),
        };

        if !ChooseColorW(&mut cc).as_bool() {
            return None;
        }

        let color = cc.rgbResult.0;
        Some((
            (color & 0xFF) as u8,
            ((color >> 8) & 0xFF) as u8,
            ((color >> 16) & 0xFF) as u8,
        ))
    }
}

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

        // 默认扩展名（必须保持内存有效直到 API 调用结束）
        let def_ext_wide = to_wide_chars("png");

        // 设置OPENFILENAME结构
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: hwnd,
            lpstrFilter: PCWSTR(filter_wide.as_ptr()),
            lpstrFile: PWSTR(file_name.as_mut_ptr()),
            nMaxFile: file_name.len() as u32,
            lpstrTitle: PCWSTR(title_wide.as_ptr()),
            Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY,
            lpstrDefExt: PCWSTR(def_ext_wide.as_ptr()),
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

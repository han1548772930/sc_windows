use crate::*;
use windows::Win32::{Foundation::*, Graphics::Gdi::*, System::DataExchange::*};
use windows::core::*;

// 绘图和元素管理
impl WindowState {
    pub fn save_history(&mut self) {
        let state = HistoryState {
            drawing_elements: self.drawing_elements.clone(),
            selected_element: self.selected_element,
        };

        self.history.push(state);

        // 限制历史记录数量，避免内存过多占用
        const MAX_HISTORY: usize = 20;
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn undo(&mut self) {
        if !self.can_undo() {
            return; // 没有可撤销的内容
        }

        if let Some(state) = self.history.pop() {
            self.drawing_elements = state.drawing_elements;
            self.selected_element = state.selected_element;

            // 更新选中状态
            for element in &mut self.drawing_elements {
                element.selected = false;
            }

            if let Some(index) = self.selected_element {
                if index < self.drawing_elements.len() {
                    self.drawing_elements[index].selected = true;
                }
            }
        }
    }
    pub fn get_element_at_position(&self, x: i32, y: i32) -> Option<usize> {
        if x < self.selection_rect.left
            || x > self.selection_rect.right
            || y < self.selection_rect.top
            || y > self.selection_rect.bottom
        {
            return None;
        }

        if x < 0 || x >= self.screen_width || y < 0 || y >= self.screen_height {
            return None;
        }

        for (index, element) in self.drawing_elements.iter().enumerate().rev() {
            if self.is_element_visible(element) && element.contains_point(x, y) {
                return Some(index);
            }
        }
        None
    }
    pub fn is_element_visible(&self, element: &DrawingElement) -> bool {
        let element_rect = element.get_bounding_rect();

        // 检查元素是否与选择框有交集
        let intersects_selection = !(element_rect.right < self.selection_rect.left
            || element_rect.left > self.selection_rect.right
            || element_rect.bottom < self.selection_rect.top
            || element_rect.top > self.selection_rect.bottom);

        // 检查元素是否在屏幕范围内
        let within_screen = !(element_rect.right < 0
            || element_rect.left > self.screen_width
            || element_rect.bottom < 0
            || element_rect.top > self.screen_height);

        // 只要有交集且在屏幕内就认为可见（绘制时会被裁剪）
        intersects_selection && within_screen
    }
    pub fn is_element_visible_in_selection(&self, element: &DrawingElement) -> bool {
        let element_rect = element.get_bounding_rect();

        // 检查元素是否与选择框有交集
        !(element_rect.right < self.selection_rect.left
            || element_rect.left > self.selection_rect.right
            || element_rect.bottom < self.selection_rect.top
            || element_rect.top > self.selection_rect.bottom)
    }
    pub fn end_drag(&mut self) {
        if self.drag_mode == DragMode::DrawingShape {
            if let Some(mut element) = self.current_element.take() {
                // 根据不同工具类型判断是否保存
                let should_save = match element.tool {
                    DrawingTool::Pen => {
                        // 手绘工具：至少要有2个点
                        element.points.len() > 1
                    }
                    DrawingTool::Rectangle | DrawingTool::Circle | DrawingTool::Arrow => {
                        // 形状工具：检查尺寸
                        if element.points.len() >= 2 {
                            let dx = (element.points[1].x - element.points[0].x).abs();
                            let dy = (element.points[1].y - element.points[0].y).abs();
                            dx > 5 || dy > 5 // 至少有一个方向大于5像素
                        } else {
                            false
                        }
                    }
                    DrawingTool::Text => {
                        // 文本工具：有位置点就保存
                        !element.points.is_empty()
                    }
                    _ => false,
                };

                if should_save {
                    // 关键：保存前更新边界矩形
                    element.update_bounding_rect();
                    self.drawing_elements.push(element);
                }
            }
        } else if self.drag_mode == DragMode::Drawing {
            let width = self.selection_rect.right - self.selection_rect.left;
            let height = self.selection_rect.bottom - self.selection_rect.top;

            if width < MIN_BOX_SIZE || height < MIN_BOX_SIZE {
                self.has_selection = false;
                self.toolbar.hide();
            } else {
                self.toolbar.update_position(
                    &self.selection_rect,
                    self.screen_width,
                    self.screen_height,
                );
            }
        }

        self.mouse_pressed = false;
        self.drag_mode = DragMode::None;
    }

    /// 临时隐藏UI元素进行截图
    pub fn hide_ui_for_capture(&mut self, hwnd: HWND) {
        self.hide_ui_for_capture = true;
        unsafe {
            // 强制重绘以隐藏UI元素
            let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
            let _ = UpdateWindow(hwnd);
            // 等待重绘完成
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    /// 恢复UI元素显示
    pub fn show_ui_after_capture(&mut self, hwnd: HWND) {
        self.hide_ui_for_capture = false;
        unsafe {
            // 强制重绘以显示UI元素
            let _ = InvalidateRect(Some(hwnd), None, FALSE.into());
            let _ = UpdateWindow(hwnd);
        }
    }

    pub fn save_selection(&mut self, hwnd: HWND) -> Result<()> {
        // 临时隐藏UI元素
        self.hide_ui_for_capture(hwnd);

        let result = unsafe {
            let width = self.selection_rect.right - self.selection_rect.left;
            let height = self.selection_rect.bottom - self.selection_rect.top;

            if width <= 0 || height <= 0 {
                return Ok(());
            }

            // 截取屏幕的完整选择区域（包含所有内容但不包含UI元素）
            let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            let old_bitmap = SelectObject(mem_dc, bitmap.into());

            // 从屏幕复制选择区域
            let _ = BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                Some(screen_dc),
                self.selection_rect.left,
                self.selection_rect.top,
                SRCCOPY,
            );

            // 复制到剪贴板
            if OpenClipboard(Some(HWND(std::ptr::null_mut()))).is_ok() {
                let _ = EmptyClipboard();
                let _ = SetClipboardData(2, Some(HANDLE(bitmap.0 as *mut std::ffi::c_void)));
                let _ = CloseClipboard();
            } else {
                let _ = DeleteObject(bitmap.into());
            }

            // 清理资源
            SelectObject(mem_dc, old_bitmap);
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            let _ = DeleteDC(mem_dc);

            Ok(())
        };

        // 恢复UI元素显示
        self.show_ui_after_capture(hwnd);

        result
    }

    // 新增：保存选择区域到文件（让用户选择保存路径）
    pub fn save_selection_to_file(&mut self, hwnd: HWND) -> Result<bool> {
        let width = self.selection_rect.right - self.selection_rect.left;
        let height = self.selection_rect.bottom - self.selection_rect.top;

        if width <= 0 || height <= 0 {
            return Ok(false);
        }

        // 显示文件保存对话框
        let file_path = match crate::file_dialog::show_image_save_dialog(hwnd, "screenshot.png") {
            Some(path) => path,
            None => return Ok(false), // 用户取消了对话框
        };

        // 临时隐藏UI元素
        self.hide_ui_for_capture(hwnd);

        let result = unsafe {
            // 截取屏幕选择区域
            let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            let old_bitmap = SelectObject(mem_dc, bitmap.into());

            // 从屏幕复制选择区域
            let _ = BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                Some(screen_dc),
                self.selection_rect.left,
                self.selection_rect.top,
                SRCCOPY,
            );

            // 保存位图到文件
            let save_result = self.save_bitmap_to_file(bitmap, &file_path, width, height);

            // 清理资源
            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteDC(mem_dc);
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
            let _ = DeleteObject(bitmap.into());

            save_result.map(|_| true)
        };

        // 恢复UI元素显示
        self.show_ui_after_capture(hwnd);

        result
    }

    /// 保存位图到文件
    fn save_bitmap_to_file(
        &self,
        bitmap: HBITMAP,
        file_path: &str,
        width: i32,
        height: i32,
    ) -> Result<()> {
        unsafe {
            // 获取位图信息
            let mut bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // 负值表示自上而下的位图
                    biPlanes: 1,
                    biBitCount: 24, // 24位RGB
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };

            // 计算位图数据大小
            let bytes_per_line = ((width * 3 + 3) / 4) * 4; // 4字节对齐
            let data_size = bytes_per_line * height;
            let mut bitmap_data = vec![0u8; data_size as usize];

            // 获取屏幕DC
            let screen_dc = GetDC(Some(HWND(std::ptr::null_mut())));

            // 获取位图数据
            GetDIBits(
                screen_dc,
                bitmap,
                0,
                height as u32,
                Some(bitmap_data.as_mut_ptr() as *mut _),
                &mut bitmap_info,
                DIB_RGB_COLORS,
            );

            // 创建BMP文件头
            let file_header = BITMAPFILEHEADER {
                bfType: 0x4D42, // "BM"
                bfSize: (std::mem::size_of::<BITMAPFILEHEADER>()
                    + std::mem::size_of::<BITMAPINFOHEADER>()
                    + data_size as usize) as u32,
                bfReserved1: 0,
                bfReserved2: 0,
                bfOffBits: (std::mem::size_of::<BITMAPFILEHEADER>()
                    + std::mem::size_of::<BITMAPINFOHEADER>()) as u32,
            };

            // 写入文件
            use std::fs::File;
            use std::io::Write;

            let mut file = File::create(file_path)?;

            // 写入文件头
            let file_header_bytes = std::slice::from_raw_parts(
                &file_header as *const _ as *const u8,
                std::mem::size_of::<BITMAPFILEHEADER>(),
            );
            file.write_all(file_header_bytes)?;

            // 写入信息头
            let info_header_bytes = std::slice::from_raw_parts(
                &bitmap_info.bmiHeader as *const _ as *const u8,
                std::mem::size_of::<BITMAPINFOHEADER>(),
            );
            file.write_all(info_header_bytes)?;

            // 写入位图数据
            file.write_all(&bitmap_data)?;

            // 清理资源
            ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc);
        }

        Ok(())
    }
}

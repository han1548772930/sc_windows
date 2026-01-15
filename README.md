# sc_windows

Windows 原生截图与标注工具（Rust + Direct2D），支持矩形/圆形/箭头/画笔/文字标注、OCR 文字识别、保存/复制与固钉预览。
<img width="1566" height="564" alt="ScreenShot_2026-01-15_110419_517" src="https://github.com/user-attachments/assets/847a8596-fe4f-4f2a-b972-ababa84127a3" />
<img width="1961" height="608" alt="ScreenShot_2026-01-15_110456_147" src="https://github.com/user-attachments/assets/95a1edc6-ccae-4d34-b198-01161ba5838c" />
![screenshot](https://github.com/user-attachments/assets/b6417c1c-4abe-4e4b-943d-1d6b1b2bc7d6)

## 功能
- **截图**：框选区域、智能窗口检测与高亮、实时尺寸预览
- **标注**：矩形、圆形、箭头、画笔、文字，支持颜色与粗细调节，撤销/重做
- **OCR**：基于 PaddleOCR 模型（MNN 推理），支持多语言文字识别
- **输出**：保存到文件、复制到剪贴板、固钉悬浮窗口
- **系统集成**：系统托盘、全局热键（默认 Ctrl+Alt+S）

## 快速开始
### 环境要求
- Windows 10/11
- Rust toolchain（建议通过 rustup 安装）

### 构建运行
```bash
cargo run -p sc_windows --release
```

### OCR 模型
OCR 使用 `models/` 目录下的模型文件。可在设置窗口选择识别语言（中/英/日/韩等）。

## 架构（简要）
代码按 core + platform 抽象 + 平台实现拆分：
- `crates/sc_app`：Core（平台无关状态机 / Action-Effect / reducers）
- `crates/sc_platform`：Host-facing 平台抽象（`HostPlatform`、输入事件等）
- `crates/sc_platform_windows`：Windows 平台实现（Win32/D2D/clipboard/dialog/tray/hotkeys 等）
- `crates/sc_host_windows`：Windows 宿主（composition root，把平台事件连接到 core + UI）
- `crates/sc_ui` / `crates/sc_ui_windows`：UI（平台无关逻辑 + Windows 窗口/控件）
- `crates/sc_drawing` / `crates/sc_drawing_host`：标注/绘图 core + host 组件
- `crates/sc_ocr`：OCR 引擎与识别封装
- `crates/sc_host_protocol`：Host 命令/消息协议
- `apps/sc_windows`：对外 crate 名保持 `sc_windows` 的 thin wrapper（入口 + 兼容 re-export）

## 许可
MIT

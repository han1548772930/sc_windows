# sc_windows

Windows 原生截图与标注工具（Rust + Direct2D），支持矩形/圆形/箭头/画笔/文字标注、OCR 文字识别、保存/复制与固钉预览。
<img width="1566" height="564" alt="ScreenShot_2026-01-15_110419_517" src="https://github.com/user-attachments/assets/847a8596-fe4f-4f2a-b972-ababa84127a3" />
<img width="1961" height="608" alt="ScreenShot_2026-01-15_110456_147" src="https://github.com/user-attachments/assets/95a1edc6-ccae-4d34-b198-01161ba5838c" />
![screenshot](https://github.com/user-attachments/assets/b6417c1c-4abe-4e4b-943d-1d6b1b2bc7d6)

## 功能
- **截图**：框选区域、智能窗口检测与高亮、实时尺寸预览
- **滚动截图**：后台逐帧拼接、实时长图预览、支持反向滚动与向已有范围外继续扩展
- **标注**：矩形、圆形、箭头、画笔、文字，支持颜色与粗细调节，撤销/重做
- **OCR**：基于 PaddleOCR 模型（MNN 推理），支持多语言文字识别
- **输出**：保存到文件、复制到剪贴板、固钉悬浮窗口
- **系统集成**：系统托盘、全局热键（默认 Ctrl+Alt+S）

## 快速开始
### 环境要求
- Windows 10/11
- Rust toolchain（建议通过 rustup 安装）
- LLVM/Clang（OCR 原生依赖在 Windows 上生成绑定时需要，并确保 LLVM `bin` 目录可被构建脚本找到）

### 构建运行
```bash
cargo run -p sc_windows --release
```

### OCR 模型
OCR 使用 `models/` 目录下的模型文件。可在设置窗口选择识别语言（中/英/日/韩等）。

## 滚动截图说明

- 滚动截图期间，选区内的物理滚轮输入会进入 FIFO 队列，并以固定的 `10ms` 节拍分片输出。
- 每次最多输出 `40 delta`，累计滚动量和正反方向顺序保持不变；快速输入可能产生短暂队列延迟。
- 拼接首先使用多区域垂直重叠匹配，失败时再使用 `imageproc` 并行模板匹配进行严格复核。
- 候选位移必须通过多锚点一致性、唯一峰值和原始 RGB 像素验证。无法可靠确定位置时会停止，不会跳过失败帧或把猜测结果写入长图。
- 页面动画、视频、重复内容或完全无重叠的画面仍可能无法得到唯一解，此时控制台会输出方向、最后成功位移和选区尺寸。

## 更新记录

### 2026-07-22

- 增加滚动截图后台 FIFO 处理与实时预览。
- 支持滚动方向切换、边界回弹，以及返回后继续向已捕获范围外扩展。
- 增加匀速滚轮队列，降低滚动速度突变造成的帧间大位移。
- 增加基于 `imageproc 0.27` 的并行模板匹配和严格 RGB 复核。
- 增加重复聊天内容、窄文字区域、边界回弹和 `94px` 位移回归测试。
- 将工作区直接依赖及兼容的传递依赖升级到当前稳定版本。

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

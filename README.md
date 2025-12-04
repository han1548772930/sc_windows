# sc_windows

Windows 原生截图与标注工具（Rust + Direct2D），支持矩形/圆形/箭头/画笔/文字标注、OCR 识别、保存/复制与固钉。

## 功能
- 截图：框选区域、窗口高亮、实时预览
- 标注：矩形、圆形、箭头、画笔、文字，颜色与粗细可调，撤销/重做
- OCR：PaddleOCR-json 集成，多语言识别（可在设置中切换）
- 系统集成：托盘、全局热键（默认 Ctrl+Alt+S）、置顶显示

## 架构
- 渲染管线（rendering/）
  - Direct2D 后端：platform/windows/d2d.rs
  - LayerCache（静态层）+ 动态层，RenderList + DirtyRect 减少重绘
- 绘图子系统（drawing/）
  - ElementManager 管理元素；HistoryManager 采用命令模式（DrawingAction，undo_action/redo_action）
  - 选择/手柄命中与交互（selection/、interaction/），几何缓存（cache/）
- 平台抽象（platform/）
  - traits + events 抽象输入与渲染；Windows 实现基于 D2D/DWrite/GDI；共享工厂（factory/）
- 消息与状态
  - Message/Command 解耦流程；状态机：Idle → Selecting → Editing → Processing
- 设置与 OCR
  - ConfigManager 精简为手动重载；PaddleOCR-json 异步启动与状态反映

## 快速开始
- 要求：Windows 10+，Rust 稳定版，Direct2D 可用
- 构建运行：
  - cargo build --release
  - target/release/sc_windows.exe
- OCR（可选）：将 PaddleOCR-json_v1.4.1 放在可执行文件同目录

## 目录
- src/drawing：元素、交互、历史、几何缓存
- src/rendering：LayerCache、RenderList、DirtyRect
- src/platform：traits/events 与 Windows 实现（d2d/gdi/factory/...）
- src/screenshot：选择与捕获
- src/ui：工具栏、光标与预览
- src/settings：配置与默认值
- src/system：托盘、热键、窗口检测
- benches：性能基准（渲染/选择/缓存）

## 常用命令
- 运行：cargo run --release
- 基准：cargo bench

## 许可
MIT（见 LICENSE）。

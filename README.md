# sc_windows

Windows 原生截图与标注工具（Rust + Direct2D），支持矩形/圆形/箭头/画笔/文字标注、OCR 文字识别、保存/复制与固钉预览。

## 功能
- **截图**：框选区域、智能窗口检测与高亮、实时尺寸预览
- **标注**：矩形、圆形、箭头、画笔、文字，支持颜色与粗细调节，撤销/重做
- **OCR**：PaddleOCR 集成，支持中英日韩等多语言文字识别
- **输出**：保存到文件、复制到剪贴板、固钉悬浮窗口
- **系统集成**：系统托盘、全局热键（默认 Ctrl+Alt+S）

## 架构
```
┌─────────────────────────────────────────────────────────┐
│                      App (app.rs)                       │
│              状态机 + 消息分发 + 命令执行                 │
├─────────────────────────────────────────────────────────┤
│  State Machine        Message/Command       Rendering   │
│  ┌─────────────┐     ┌──────────────┐    ┌───────────┐ │
│  │ Idle        │     │ Message      │    │ D2D       │ │
│  │ Selecting   │◄───►│ Command      │◄──►│ Renderer  │ │
│  │ Editing     │     │ DrawingMsg   │    │ LayerCache│ │
│  │ Processing  │     └──────────────┘    └───────────┘ │
│  └─────────────┘                                       │
├─────────────────────────────────────────────────────────┤
│  Drawing           Screenshot         UI               │
│  ┌───────────┐    ┌───────────┐    ┌───────────┐      │
│  │ Elements  │    │ Selection │    │ Toolbar   │      │
│  │ History   │    │ Capture   │    │ Preview   │      │
│  │ Tools     │    │ Save      │    │ Cursor    │      │
│  └───────────┘    └───────────┘    └───────────┘      │
├─────────────────────────────────────────────────────────┤
│  Platform (Windows)           System                   │
│  ┌────────────────────┐      ┌───────────────┐        │
│  │ Direct2D + DWrite  │      │ Tray + Hotkey │        │
│  │ GDI (截图捕获)      │      │ Window Detect │        │
│  └────────────────────┘      └───────────────┘        │
└─────────────────────────────────────────────────────────┘
```

### 核心模块
- **渲染管线** (`rendering/`): LayerCache 分层缓存 + RenderList 批量渲染 + DirtyRect 局部重绘
- **绘图系统** (`drawing/`): ElementManager 元素管理 + HistoryManager 命令模式撤销/重做 + GeometryCache 几何缓存
- **状态机** (`state/`): Idle → Selecting → Editing → Processing 状态流转
- **平台层** (`platform/`): Direct2D 渲染器、GDI 截图、DWrite 文本、共享工厂

## 快速开始

### 环境要求
- Windows 10/11
- Rust 2024 Edition (nightly)
- Direct2D 支持（系统自带）

### 构建运行
```bash
cargo build --release
./target/release/sc_windows.exe
```

### OCR 支持（可选）
将 [PaddleOCR-json](https://github.com/hiroi-sora/PaddleOCR-json) 放在可执行文件同目录的 `PaddleOCR-json_v1.4.1/` 文件夹中。

## 目录结构
```
src/
├── app.rs              # 应用主体，状态机与消息循环
├── command_executor.rs # 命令执行器
├── drawing/            # 绘图子系统
│   ├── elements.rs     # 元素管理
│   ├── history.rs      # 撤销/重做（命令模式）
│   ├── types.rs        # DrawingElement, DrawingTool
│   └── cache.rs        # 几何缓存
├── rendering/          # 渲染管线
│   ├── layer_cache.rs  # 分层缓存
│   ├── render_list.rs  # 渲染命令列表
│   └── dirty_rect.rs   # 脏矩形优化
├── platform/           # 平台抽象
│   └── windows/        # Windows 实现
│       ├── d2d.rs      # Direct2D 渲染器
│       ├── gdi.rs      # GDI 截图
│       └── factory.rs  # D2D/DWrite 工厂
├── screenshot/         # 截图功能
├── ui/                 # 用户界面
│   ├── toolbar.rs      # 工具栏
│   └── preview/        # 固钉预览窗口
├── settings/           # 配置管理
├── system/             # 系统集成
│   ├── tray.rs         # 系统托盘
│   ├── hotkeys.rs      # 全局热键
│   └── window_detection.rs  # 窗口检测
├── ocr/                # OCR 模块
└── state/              # 状态机

benches/                # 性能基准测试
tests/                  # 单元测试
```

## 常用命令
```bash
cargo run --release     # 运行
cargo test              # 测试
cargo bench             # 性能基准
cargo check             # 快速检查
```

## 许可
MIT

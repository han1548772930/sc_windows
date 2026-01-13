# sc_windows

Windows 原生截图与标注工具（Rust + Direct2D），支持矩形/圆形/箭头/画笔/文字标注、OCR 文字识别、保存/复制与固钉预览。

## 功能
- **截图**：框选区域、智能窗口检测与高亮、实时尺寸预览
- **标注**：矩形、圆形、箭头、画笔、文字，支持颜色与粗细调节，撤销/重做
- **OCR**：内置 PaddleOCR + MNN 推理，支持中英日韩等多语言文字识别
- **输出**：保存到文件、复制到剪贴板、固钉悬浮窗口
- **系统集成**：系统托盘、全局热键（默认 Ctrl+Alt+S）

## 架构

本项目已完成 **core/host split**（保持功能不变），并将 Win32 副作用尽量收口到 platform backend：

- **Core（平台无关）**：`crates/sc_app`
  负责状态（model）、Action/Effect、reducer（例如 selection）。
- **Host（Windows 宿主 / composition root）**：`crates/sc_host_windows`
  负责输入事件桥接、执行 core effects，驱动 UI/渲染/系统集成。
- **Host-facing Platform API**：`crates/sc_platform` 的 `HostPlatform`
  host 通过它请求窗口/定时器/剪贴板/对话框等副作用；Windows 实现在 `crates/sc_platform_windows::windows::WindowsHostPlatform`。
  公共 API 使用 `WindowId`（opaque）避免暴露 `HWND`。
- **对外 crate 名保持 `sc_windows`**：`apps/sc_windows`
  薄封装：binary 入口 + 对 `sc_host_windows` 的 re-export（保持历史 crate 名）。

依赖关系（概念图）：

```text
apps/sc_windows (crate: sc_windows)
        │  main.rs (entry) + lib.rs (re-export)
        ▼
crates/sc_host_windows (Host)
        │  Win32 message loop / UI / system integration / rendering backend
        │  executes core effects
        ▼
crates/sc_app (Core)
        │  Action/Effect + reducers (platform-neutral)
        ▼
shared crates:
  sc_ui / sc_rendering / sc_drawing / sc_platform
windows impl:
  sc_platform_windows
```

### 关键 crates
Core / platform-neutral：
- `crates/sc_app`：core model + Action/Effect + reducers
- `crates/sc_host_protocol`：Host 命令/消息协议（Command / UIMessage / DrawingMessage）
- `crates/sc_rendering`：RenderList / dirty rect / 渲染基础类型
- `crates/sc_ui`：平台无关 UI（layout/hit-test/RenderList builders）
- `crates/sc_drawing`：绘图 core（含 windows feature 的渲染/适配）
- `crates/sc_settings`：Settings 持久化 + ConfigManager
- `crates/sc_platform`：平台抽象（InputEvent / WindowMessageHandler / HostPlatform 等）

Windows backend / host components：
- `crates/sc_platform_windows`：Win32 backend（D2D/GDI/clipboard/dialog/tray/hotkeys/win_api 等）
- `crates/sc_highlight`：窗口/控件命中与 auto-highlight
- `crates/sc_ocr`：OCR helpers（引擎创建、模型检测、识别等）
- `crates/sc_drawing_host`：绘图编辑器 host 组件（DrawingManager 等）
- `crates/sc_ui_windows`：Windows UI 组件（toolbar/preview/settings/cursor/icons）
- `crates/sc_host_windows`：composition root（连接 core + platform + system integration）

Wrapper：
- `apps/sc_windows`：入口 + re-export（对外 crate 名保持 `sc_windows`）

## 代码风格
为了保持一致性，仓库采用以下风格约定：
- 文件开头不保留大块头部注释（例如 `//! ...`）。
- 所有 `use` import 统一放在文件顶部（不在函数块/测试模块里写 `use ...`）。
- 尽量避免在代码里写内联绝对路径（`crate::...` / `sc_*::...` / `windows::...`）；优先在顶部 `use` 引入再使用短名。
- host/UI 侧优先通过 `HostPlatform` 请求平台副作用；窗口句柄对外使用 `WindowId`（opaque）。

## 快速开始

### 环境要求
- Windows 10/11
- Rust（Edition 2024）
- Direct2D 支持（系统自带）

### 构建运行
```bash
# workspace 下建议显式指定 package
cargo run -p sc_windows --release

# 或仅构建：
cargo build -p sc_windows --release
```

### OCR 支持
OCR 已内置，使用 MNN 推理引擎 + PaddleOCR 模型。模型文件位于 `models/` 目录，支持多语言（中文、英文、日文、韩文、阿拉伯文等）。

## 目录结构

```text
sc_windows/
├── apps/
│   └── sc_windows/                  # 薄封装：入口 + re-export（对外 crate 名保持 sc_windows）
│       ├── src/                     # main.rs + lib.rs
│       ├── icons/                   # SVG/ICO 资源（编译期嵌入）
│       ├── benches/                 # 性能基准
│       └── tests/
├── crates/
│   ├── sc_app/                      # Core：Action/Effect + reducers（平台无关）
│   ├── sc_host_protocol/            # Host 命令/消息协议
│   ├── sc_rendering/                # RenderList / dirty rect / types
│   ├── sc_ui/                       # 平台无关 UI builders
│   ├── sc_drawing/                  # 绘图 core（含 windows feature 的渲染/适配）
│   ├── sc_settings/                 # Settings 持久化 + ConfigManager
│   ├── sc_platform/                 # 平台抽象（InputEvent / HostPlatform 等）
│   ├── sc_platform_windows/         # Windows backend（D2D/GDI/dialog/tray/hotkeys/clipboard/win_api 等）
│   ├── sc_highlight/                # 窗口/控件命中与 auto-highlight
│   ├── sc_ocr/                      # OCR helpers
│   ├── sc_drawing_host/             # 绘图编辑器 host 组件
│   ├── sc_ui_windows/               # Windows UI 组件（toolbar/preview/settings）
│   └── sc_host_windows/             # composition root（连接 core + platform + system integration）
├── models/                          # OCR 模型与运行时资源
├── Cargo.toml                       # Workspace
└── README.md
```

更详细的架构/边界说明与重构进度记录请查看根目录的状态文档。

## 常用命令
```bash
cargo fmt --all                      # 格式化
cargo check --workspace --all-targets # 全 workspace 编译检查
cargo test --workspace                # 全 workspace 测试
cargo clippy --workspace --all-targets# clippy（清理/静态检查）

cargo run -p sc_windows --release     # 运行
cargo test -p sc_host_windows         # 只测 host
cargo test -p sc_app                  # 只测 core
cargo check -p sc_windows --benches   # benches 编译检查
```

## 许可
MIT

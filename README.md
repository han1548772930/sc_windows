# sc_windows

Windows 原生截图与标注工具（Rust + Direct2D），支持矩形/圆形/箭头/画笔/文字标注、OCR 文字识别、保存/复制与固钉预览。

## 功能
- **截图**：框选区域、智能窗口检测与高亮、实时尺寸预览
- **标注**：矩形、圆形、箭头、画笔、文字，支持颜色与粗细调节，撤销/重做
- **OCR**：内置 PaddleOCR + MNN 推理，支持中英日韩等多语言文字识别
- **输出**：保存到文件、复制到剪贴板、固钉悬浮窗口
- **系统集成**：系统托盘、全局热键（默认 Ctrl+Alt+S）

## 架构

本项目正在重构为类似 Zed/gpui 的 **core/host split**（保持功能不变）：

- **Core（平台无关）**：`crates/sc_app`
  负责状态（model）、Action/Effect、reducer（例如 selection）。
- **Host（Windows 宿主）**：`crates/sc_host_windows`
  负责 Win32 事件泵、系统集成（托盘/热键/剪贴板/对话框等）、渲染后端、执行副作用（effects）。
- **对外 crate 名保持 `sc_windows`**：`apps/sc_windows`
  只是薄封装：binary 入口 + 对 `sc_host_windows` 的 re-export。

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
- `crates/sc_app`：Core（platform-neutral）Action/Effect + reducers
- `crates/sc_host_windows`：Host（Win32）输入事件 -> core Action，执行 effects，驱动 UI/渲染/系统集成
- `crates/sc_platform_windows`：Windows 平台实现（D2D/GDI/system/dialog/tray/hotkeys/clipboard/win_api 等）
- `crates/sc_ui`：平台无关 UI（ViewState -> RenderList + hit-test 数据）
- `crates/sc_rendering`：平台无关渲染类型/RenderList/脏矩形等
- `crates/sc_drawing`：绘图 core（含 windows feature 的渲染/适配）
- `crates/sc_platform`：平台抽象（输入事件等）

## 代码风格（Zed 风格）
为了保持一致性，仓库采用以下风格约定：
- 文件开头不保留大块头部注释（例如 `//! ...`）。
- 所有 `use` import 统一放在文件顶部（不在函数块/测试模块里写 `use ...`）。
- 尽量避免在代码里写内联绝对路径（`crate::...` / `sc_*::...` / `windows::...`）；优先在顶部 `use` 引入再使用短名。

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
│       └── benches/                 # 性能基准（如果存在）
├── crates/
│   ├── sc_host_windows/             # Host：Win32 事件泵 / 执行副作用 / UI / 系统集成 / 渲染后端
│   ├── sc_app/                      # Core：Action/Effect + reducers（平台无关）
│   ├── sc_ui/                       # 平台无关 UI（ViewState -> RenderList + hit-test）
│   ├── sc_rendering/                # RenderList / 脏矩形等（平台无关）
│   ├── sc_drawing/                  # 绘图 core（含 windows feature 的渲染/适配）
│   ├── sc_platform/                 # 平台抽象（输入事件等）
│   ├── sc_platform_windows/         # Windows 平台实现（D2D/GDI/system/dialog 等）
│   └── sc_highlight/                # 窗口命中/auto-highlight 相关逻辑
├── models/                          # OCR 模型与运行时资源
├── Cargo.toml                       # Workspace
└── ZED_GPUI_REFACTOR_STATUS.md      # 重构进度记录
```

## 常用命令
```bash
cargo run -p sc_windows --release   # 运行
cargo test                          # 测试（整个 workspace）
cargo test -p sc_host_windows       # 只测 host
cargo test -p sc_app                # 只测 core
cargo check -p sc_windows --benches # benches 编译检查
```

## 许可
MIT

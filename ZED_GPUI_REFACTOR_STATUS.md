# Zed/gpui 重构状态（截至 2026-01-13）

目标：参考 Zed 的 `gpui` 分层，把工程拆成「纯 core + platform 抽象 + 平台实现」，并让 `sc_host_windows` 只承担 composition root / 接线（尽量不承载可复用逻辑）。

## 0) 不变量（边界规则）
- `crates/sc_app`（core）不依赖任何 Windows/host crate。
- host 不维护自己的业务状态机；行为主要由 core model 派生。
- Windows-only 的系统副作用优先集中在 `sc_platform_windows` 与 `*_windows` crates；`sc_host_windows` 只保留必要的 composition root / message loop / glue。
- 兼容 re-export 仅允许存在于 wrapper crate（`apps/sc_windows`）对外暴露；实现层代码不得依赖 compat 模块。

## 1) 当前 workspace（已落地）
### Zed/gpui 对照（心智模型）
- Zed：`crates/gpui/src/platform.rs` 定义 `Platform` 抽象 + `current_platform(...)` 选择 backend；每个平台实现放在 `crates/gpui/src/platform/<os>/*`。
- 本项目：
  - `crates/sc_platform_windows/src/windows/*` ≈ `gpui/src/platform/windows/*`（Win32/Direct2D/clipboard/hotkeys/tray/dialogs 等）
  - `crates/sc_platform` 目前主要是输入事件类型 + 最小窗口消息边界（`WindowMessageHandler`）+ 一些平台无关 traits；后续会逐步补齐更接近 gpui 的 platform 抽象（见 TODO）。
  - `crates/sc_host_windows` 承担 Windows 侧 composition root：把平台 backend 提供的事件/系统集成连接到 `sc_app`/`sc_ui`/`sc_*`。

### apps
- `apps/sc_windows`：thin wrapper（binary 入口 + 对外 re-export，保持 crate 名 `sc_windows`）

### core / platform-neutral
- `crates/sc_app`：core model + Action/Effect + reducers
- `crates/sc_host_protocol`：Host 命令/消息协议（`Command` / `UIMessage` / `DrawingMessage`）
- `crates/sc_rendering`：RenderList / Dirty rect / 渲染数据结构
- `crates/sc_ui`：平台无关 UI（view/layout -> RenderList builders）
- `crates/sc_drawing`：绘图 core（几何/元素；Windows 相关通过 feature）
- `crates/sc_platform`：平台抽象（InputEvent / WindowMessageHandler / traits）
- `crates/sc_settings`：平台无关 Settings 持久化 + `ConfigManager`

### Windows 实现 / host components
- `crates/sc_platform_windows`：Win32/Direct2D/GDI/clipboard/hotkeys/tray/dialogs/win_api 等（逐步收敛为 Windows backend）
- `crates/sc_highlight`：窗口/控件检测（Windows）
- `crates/sc_ocr`：OCR helpers（引擎创建、模型检测、分行识别等）
- `crates/sc_drawing_host`：绘图编辑器的 host 组件（`DrawingManager`），依赖 `sc_drawing` + Windows renderer
- `crates/sc_ui_windows`：Windows UI host 组件（toolbar/cursor/preview/pin window/icons）
- `crates/sc_host_windows`：composition root + screenshot/selection + system orchestration（message loop/window proc 已迁至 platform runner）

## 2) DONE（本轮已经完成）
- 从 `sc_host_windows` 抽出独立 crates：
  - protocol → `sc_host_protocol`
  - OCR → `sc_ocr`
  - drawing editor → `sc_drawing_host`
  - Windows UI（toolbar/preview/cursor/icons）→ `sc_ui_windows`
- 新增平台无关 settings：
  - 新增 `crates/sc_settings`（Settings 持久化 + `ConfigManager`）
  - Settings UI（Win32 窗口）移动到 `sc_ui_windows::SettingsWindow`
  - host 通过命令链处理 settings：`ShowSettings -> (bool) -> ReloadSettings`，不再依赖自定义 `WM_RELOAD_SETTINGS`
  - 删除旧的 `crates/sc_host_windows/src/settings/*`
- `sc_host_windows` 目录瘦身：
  - 删除 `crates/sc_host_windows/src/host/` 与 `src/shared/`，改为扁平 `src/*` + 少量子模块目录
  - 移除已废弃的旧实现目录与重复文件（`crates/` + `apps/` 下不再存在相同的 `.rs` 源码副本）
  - 删除未使用的 `utils`（host 内不再保存通用几何/计时工具）
  - tray/hotkeys 的 stateful manager 下沉到 `crates/sc_platform_windows/src/windows/*`（host 不再保留 `system/hotkeys.rs` 与 `system/tray.rs`）
  - MessageBox UI 下沉到 `crates/sc_platform_windows/src/windows/message_box.rs`（统一通过 helper；`MessageBoxW` 仅保留在该模块）
  - screen capture + D2D 位图创建路径下沉到 `crates/sc_platform_windows`（host 不再直接处理 HBITMAP / GDI DC 选择逻辑）
- window bootstrap/message loop 下沉到 `crates/sc_platform_windows/src/win_api.rs`（DPI aware、RegisterClass/CreateWindow、user data、DefWindowProc、GetMessage loop 等）
- 初步引入更接近 gpui 的 platform 抽象入口：
  - `crates/sc_platform/src/host.rs` 新增 `WindowMessageHandler`（window proc/message loop 的边界）
  - `crates/sc_platform_windows/src/windows/app_runner.rs` 新增通用 Win32 runner：
    - `run_toolwindow_app`
    - `run_fullscreen_toolwindow_app`
    -（window proc + message loop + GWLP_USERDATA 挂载 app）
  - `crates/sc_platform_windows/src/windows/app_runner.rs` 负责把 Win32 输入消息转换为 `sc_platform::InputEvent` 并调用 `WindowMessageHandler::handle_input_event`
  - `crates/sc_platform/src/events.rs` 新增 `InputEvent::Hotkey`（把 `WM_HOTKEY` 抽象为平台无关事件）
  - `crates/sc_host_windows/src/run.rs` 进一步瘦身：删除自定义 `window_proc`，改为提供 app factory 并调用 `run_fullscreen_toolwindow_app`
  - `crates/sc_host_windows/src/app.rs`：
    - 不再依赖 `EventConverter/WPARAM/LPARAM/LRESULT`；窗口消息边界改为 `msg + wparam + lparam` 原始值
    - 热键与截屏延迟 timer 通过 `handle_input_event` 统一处理（而不是 raw `WM_HOTKEY/WM_TIMER` 分支）
    - OCR completion/availability 使用 window-thread user events（`HostEvent` + `handle_user_event`），不再依赖自定义 raw Win32 消息
- `sc_host_windows` 不再直接依赖 `windows` crate（通过 `sc_platform_windows::win32` re-export 获取 Win32 类型/常量）
  - `crates/sc_host_windows/src/constants.rs` 只保留 host 仍需要的 Win32/热键/timer 常量（UI/绘图常量已迁走）
- 依赖方向收口：
  - `sc_host_windows` 内部实现直接依赖 `sc_host_protocol` / `sc_ocr` / `sc_drawing_host` / `sc_ui_windows` / `sc_settings`
  - `DrawingConfig` 由 host 从 Settings 构造并注入（`sc_drawing_host` / `sc_ui_windows` 不再依赖 Settings）
- OCR 相关调用统一走 `sc_ocr::{create_engine, models_exist, recognize_text_by_lines}`
- HostPlatform 继续演进（2026-01-13）：
  - `HostPlatform` 新增 `request_close` / `request_redraw_erase` / `request_redraw_rect_erase`
  - `HostPlatform` 新增窗口状态接口：`minimize_window` / `maximize_window` / `restore_window` / `bring_window_to_top` / `set_window_topmost_flag`
  - `sc_ui_windows` 的 Settings/Preview 不再直接调用 `win_api`/`cursor`/`message_box`，统一走 `HostPlatform`
  - Settings/Preview 的最小化/最大化/还原/置顶 也统一走 `HostPlatform`（减少 `ShowWindow`/`SetWindowPos` 直接调用）
  - `sc_ui_windows::{PreviewWindow, SettingsWindow}` 对外改为 opaque wrapper（内部 state struct 才持有 `HWND`），进一步弱化 `HWND` 暴露
- 验证通过（2026-01-12）：
  - `cargo check --workspace --all-targets`
  - `cargo test --workspace`
  - `cargo fmt --all`
- 代码风格（Zed 风格，2026-01-13）：
  - 删除所有文件开头的大块注释（例如 `//!`）。
  - 所有 import 统一放到文件顶部（不允许在函数块/测试模块里写 `use ...`）。
  - 尽量避免在代码中写内联绝对路径（`crate::...` / `sc_*::...` / `windows::...`），改为顶层 `use` 引入。
- 验证通过（2026-01-13）：
  - `cargo fmt --all`
  - `cargo check --workspace --all-targets`
  - `cargo test --workspace`
- Theme/Style & 类型收口（2026-01-13）：
  - `crates/sc_platform::traits` 复用 `sc_rendering` 的基础类型（`Color/Point/Rectangle/TextStyle/DrawStyle/BitmapId`），避免重复定义。
  - `Direct2DRenderer` 的 `sc_rendering::RenderBackend` 实现不再做类型拷贝/转换（同一套几何/颜色类型贯穿）。

## 3) 兼容 re-export（仅 wrapper crate 保留）
位置：`apps/sc_windows/src/lib.rs`
- `sc_windows::drawing` → `sc_drawing_host`
- `sc_windows::ui` → `sc_ui_windows`
- `sc_windows::ocr` → `sc_ocr`
- `sc_windows::message` → `sc_host_protocol`
- `sc_windows::settings` → `sc_settings`

规则：
- `sc_host_windows` 内部新代码禁止通过 wrapper 的 compat 模块引用；必须直接引用真实 crate。
- `sc_windows` wrapper 保持 thin（仅入口 + 兼容 re-export + tests/benches）。

## 4) NEXT（后续可选 / 增强项）
本轮目标已完成；如果要继续更像 gpui，可以考虑：
1) Platform API（可选增强）
  - 把 tray/hotkeys 的注册/初始化也抽象进 `HostPlatform`（目前仍由 `sc_platform_windows::windows::{TrayManager, HotkeyManager}` 管理）。
  - 继续收口/类型化更多 window 相关 side-effects，进一步减少 Windows-only 细节扩散。
2) UI（可选增强）
  - 继续把 Preview/Pin 等窗口的 layout/hit-test/RenderList builder 下沉到 `crates/sc_ui`（目前已覆盖 selection overlay + toolbar）。
3) Theme/Style（可选增强）
  - 把 `crates/sc_ui_windows::constants` 与 Preview/Settings 的颜色/尺寸常量进一步收敛到统一 theme 模块。

use crate::types::{DrawingTool, ToolbarButton};
use windows::Win32::Foundation::RECT;

/// 全局消息枚举，用于组件间通信
#[derive(Debug, Clone)]
pub enum Message {
    /// 截图相关消息
    Screenshot(ScreenshotMessage),
    /// 绘图相关消息
    Drawing(DrawingMessage),
    /// UI相关消息
    UI(UIMessage),
    /// 系统相关消息
    System(SystemMessage),
}

/// 截图管理器消息
#[derive(Debug, Clone)]
pub enum ScreenshotMessage {
    /// 开始截图
    StartCapture,
    /// 开始选择区域（鼠标按下）
    StartSelection(i32, i32),
    /// 更新选择区域
    UpdateSelection(RECT),
    /// 确认选择
    ConfirmSelection,
    /// 取消截图
    CancelCapture,
}

/// 绘图管理器消息
#[derive(Debug, Clone, PartialEq)]
pub enum DrawingMessage {
    /// 选择工具
    SelectTool(DrawingTool),
    /// 开始绘制
    StartDrawing(i32, i32),
    /// 更新绘制
    UpdateDrawing(i32, i32),
    /// 完成绘制
    FinishDrawing,
    /// 添加元素
    AddElement(Box<crate::types::DrawingElement>),
    /// 撤销
    Undo,
    /// 重做
    Redo,
    /// 删除元素
    DeleteElement(usize),
    /// 选择元素
    SelectElement(Option<usize>),
    /// 检查元素点击
    CheckElementClick(i32, i32),
}

/// UI管理器消息
#[derive(Debug, Clone, PartialEq)]
pub enum UIMessage {
    /// 显示工具栏
    ShowToolbar(RECT),
    /// 隐藏工具栏
    HideToolbar,
    /// 更新工具栏位置
    UpdateToolbarPosition(RECT),
    /// 工具栏按钮点击
    ToolbarButtonClicked(ToolbarButton),
    /// 显示对话框
    ShowDialog(DialogType),
    /// 关闭对话框
    CloseDialog,
}

/// 系统管理器消息
#[derive(Debug, Clone)]
pub enum SystemMessage {
    /// 系统托盘消息
    TrayMessage(u32, u32),
    /// 热键触发
    HotkeyTriggered,
    /// 窗口检测
    WindowDetected(String),
    /// OCR状态更新
    OcrStatusUpdate(bool),
}

/// 对话框类型
#[derive(Debug, Clone, PartialEq)]
pub enum DialogType {
    /// 保存对话框
    Save,
    /// 设置对话框
    Settings,
    /// 关于对话框
    About,
}

/// 命令枚举，用于指示需要执行的操作
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// 请求重绘
    RequestRedraw,
    /// 显示覆盖层
    ShowOverlay,
    /// 隐藏覆盖层
    HideOverlay,
    /// 更新工具栏
    UpdateToolbar,
    /// 显示保存对话框
    ShowSaveDialog,
    /// 显示设置窗口
    ShowSettings,
    /// 重新加载设置
    ReloadSettings,
    /// 执行截图
    TakeScreenshot,
    /// 复制到剪贴板
    CopyToClipboard,
    /// 选择绘图工具
    SelectDrawingTool(DrawingTool),
    /// 保存选择区域到文件
    SaveSelectionToFile,
    /// 保存选择区域到剪贴板
    SaveSelectionToClipboard,
    /// 固定选择区域
    PinSelection,
    /// 提取文本
    ExtractText,
    /// 重置到初始状态
    ResetToInitialState,
    /// 隐藏窗口
    HideWindow,
    /// 退出应用
    Quit,
    /// 显示错误消息
    ShowError(String),
    /// 启动定时器
    StartTimer(u32, u32), // (timer_id, interval_ms)
    /// 停止定时器
    StopTimer(u32), // timer_id
    /// UI相关命令
    UI(UIMessage),
    /// 绘图相关命令
    Drawing(DrawingMessage),
    /// 无操作
    None,
}

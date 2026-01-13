use sc_app::selection::RectI32;
use sc_drawing::{DrawingElement, DrawingTool};

/// Drawing manager messages.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawingMessage {
    /// Select tool.
    SelectTool(DrawingTool),
    /// Start drawing.
    StartDrawing(i32, i32),
    /// Update drawing.
    UpdateDrawing(i32, i32),
    /// Finish drawing.
    FinishDrawing,
    /// Add element.
    AddElement(Box<DrawingElement>),
    /// Undo.
    Undo,
    /// Redo.
    Redo,
    /// Delete element.
    DeleteElement(usize),
    /// Select element.
    SelectElement(Option<usize>),
    /// Check element click.
    CheckElementClick(i32, i32),
}

/// UI manager messages.
#[derive(Debug, Clone, PartialEq)]
pub enum UIMessage {
    /// Show toolbar.
    ShowToolbar(RectI32),
    /// Hide toolbar.
    HideToolbar,
    /// Update toolbar position.
    UpdateToolbarPosition(RectI32),
}

/// Host command queue items.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Core actions (platform-neutral). Executed by the host.
    Core(sc_app::Action),

    /// Request redraw (full window).
    RequestRedraw,
    /// Request redraw for a dirty rect.
    RequestRedrawRect(RectI32),

    /// Update toolbar.
    UpdateToolbar,

    /// Show settings window.
    ShowSettings,
    /// Reload settings.
    ReloadSettings,

    /// Take screenshot.
    TakeScreenshot,

    /// Select drawing tool.
    SelectDrawingTool(DrawingTool),

    /// Save selection to file.
    SaveSelectionToFile,
    /// Save selection to clipboard.
    SaveSelectionToClipboard,

    /// Pin selection.
    PinSelection,

    /// Extract text.
    ExtractText,

    /// Show OCR preview (data is provided by host cache).
    ShowOcrPreview,

    /// Copy text to clipboard.
    CopyTextToClipboard(String),

    /// OCR no-text / failed message.
    ShowOcrNoTextMessage,

    /// Stop OCR engine (async).
    StopOcrEngine,

    /// Reset to initial state.
    ResetToInitialState,

    /// Hide window.
    HideWindow,

    /// Quit the app (destroy the main window).
    QuitApp,

    /// Show error.
    ShowError(String),

    /// Start timer.
    StartTimer(u32, u32),
    /// Stop timer.
    StopTimer(u32),

    /// UI commands.
    UI(UIMessage),

    /// Drawing commands.
    Drawing(DrawingMessage),

    /// No-op.
    None,
}

use sc_app::{AppModel, Effect, selection as core_selection};
use sc_platform_windows::win32::RECT;

use sc_host_protocol::{Command, DrawingMessage, UIMessage};

#[inline]
pub fn rect_to_core(rect: RECT) -> core_selection::RectI32 {
    core_selection::RectI32 {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}

#[inline]
pub fn rect_from_core(rect: core_selection::RectI32) -> RECT {
    RECT {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}

pub fn command_from_effect(effect: Effect) -> Option<Command> {
    match effect {
        Effect::Selection(sel) => match sel {
            core_selection::Effect::ShowToolbar { selection } => {
                Some(Command::UI(UIMessage::ShowToolbar(selection)))
            }
            core_selection::Effect::UpdateToolbarPosition { selection } => {
                Some(Command::UI(UIMessage::UpdateToolbarPosition(selection)))
            }
        },

        Effect::SelectDrawingTool(tool) => Some(Command::SelectDrawingTool(tool)),
        Effect::SaveSelectionToFile => Some(Command::SaveSelectionToFile),
        Effect::SaveSelectionToClipboard => Some(Command::SaveSelectionToClipboard),
        Effect::Undo => Some(Command::Drawing(DrawingMessage::Undo)),
        Effect::ExtractText => Some(Command::ExtractText),
        Effect::ShowOcrPreview => Some(Command::ShowOcrPreview),
        Effect::CopyTextToClipboard { text } => Some(Command::CopyTextToClipboard(text)),
        Effect::ShowOcrNoTextMessage => Some(Command::ShowOcrNoTextMessage),
        Effect::StopOcrEngine => Some(Command::StopOcrEngine),
        Effect::PinSelection => Some(Command::PinSelection),
        Effect::ResetToInitialState => Some(Command::ResetToInitialState),
        Effect::HideWindow => Some(Command::HideWindow),
    }
}

pub fn commands_from_effects(effects: impl IntoIterator<Item = Effect>) -> Vec<Command> {
    effects
        .into_iter()
        .filter_map(command_from_effect)
        .collect()
}

pub fn dispatch(core: &mut AppModel, action: sc_app::Action) -> Vec<Command> {
    commands_from_effects(core.reduce(action))
}

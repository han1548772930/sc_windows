use sc_drawing::DrawingTool;

pub mod ocr;
pub mod selection;

/// Top-level application actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Selection(selection::Action),
    /// User selected a drawing tool.
    SelectDrawingTool(DrawingTool),
    /// Save current selection to a file.
    SaveSelectionToFile,
    /// Save current selection to clipboard.
    SaveSelectionToClipboard,
    /// Undo the last drawing action.
    Undo,
    /// Extract text (OCR) from the current selection.
    ExtractText,
    /// OCR job completed.
    OcrCompleted {
        has_results: bool,
        is_failed: bool,
        text: String,
    },
    /// OCR job cancelled or aborted.
    OcrCancelled,
    /// Pin the current selection.
    PinSelection,
    /// Cancel the current flow (e.g. ESC).
    Cancel,
}

#[cfg(test)]
mod tests {
    #[test]
    fn extract_text_sets_running_and_emits_effect() {
        let mut m = super::AppModel::new();
        assert_eq!(m.ocr().phase(), super::ocr::Phase::Idle);

        let eff = m.reduce(super::Action::ExtractText);
        assert_eq!(m.ocr().phase(), super::ocr::Phase::Running);
        assert_eq!(eff, vec![super::Effect::ExtractText]);

        // Re-entrant requests are ignored while running.
        let eff2 = m.reduce(super::Action::ExtractText);
        assert_eq!(m.ocr().phase(), super::ocr::Phase::Running);
        assert!(eff2.is_empty());
    }

    #[test]
    fn ocr_completed_success_copies_text_and_resets() {
        let mut m = super::AppModel::new();
        let _ = m.reduce(super::Action::ExtractText);

        let eff = m.reduce(super::Action::OcrCompleted {
            has_results: true,
            is_failed: false,
            text: "hello".to_string(),
        });

        assert_eq!(m.ocr().phase(), super::ocr::Phase::Idle);
        assert_eq!(
            eff,
            vec![
                super::Effect::ShowOcrPreview,
                super::Effect::CopyTextToClipboard {
                    text: "hello".to_string(),
                },
                super::Effect::StopOcrEngine,
                super::Effect::HideWindow,
                super::Effect::ResetToInitialState,
            ]
        );
    }

    #[test]
    fn ocr_completed_no_results_shows_message_and_resets() {
        let mut m = super::AppModel::new();
        let _ = m.reduce(super::Action::ExtractText);

        let eff = m.reduce(super::Action::OcrCompleted {
            has_results: false,
            is_failed: true,
            text: String::new(),
        });

        assert_eq!(m.ocr().phase(), super::ocr::Phase::Idle);
        assert_eq!(
            eff,
            vec![
                super::Effect::ShowOcrPreview,
                super::Effect::ShowOcrNoTextMessage,
                super::Effect::StopOcrEngine,
                super::Effect::HideWindow,
                super::Effect::ResetToInitialState,
            ]
        );
    }

    #[test]
    fn ocr_cancelled_requests_cleanup_and_resets() {
        let mut m = super::AppModel::new();
        let _ = m.reduce(super::Action::ExtractText);

        let eff = m.reduce(super::Action::OcrCancelled);
        assert_eq!(m.ocr().phase(), super::ocr::Phase::Idle);
        assert_eq!(
            eff,
            vec![
                super::Effect::StopOcrEngine,
                super::Effect::HideWindow,
                super::Effect::ResetToInitialState
            ]
        );
    }
}

/// Top-level application effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Selection(selection::Effect),
    /// Apply the selected drawing tool in the host.
    SelectDrawingTool(DrawingTool),
    /// Save current selection to a file.
    SaveSelectionToFile,
    /// Save current selection to clipboard.
    SaveSelectionToClipboard,
    /// Undo the last drawing action.
    Undo,
    /// Extract text (OCR) from the current selection.
    ExtractText,
    /// Show the OCR result preview window (host uses cached completion data).
    ShowOcrPreview,
    /// Copy OCR text to clipboard.
    CopyTextToClipboard {
        text: String,
    },
    /// Show an informational message when OCR returned no text or failed.
    ShowOcrNoTextMessage,
    /// Stop the OCR engine (host-specific cleanup).
    StopOcrEngine,
    /// Pin the current selection.
    PinSelection,
    /// Reset the host back to its initial state.
    ResetToInitialState,
    /// Hide the host window.
    HideWindow,
}

/// Core app model.
///
/// This will gradually absorb more of the host app's state machine.
#[derive(Debug, Default)]
pub struct AppModel {
    selection: selection::Model,
    drawing_tool: DrawingTool,
    ocr: ocr::Model,
}

impl AppModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn selection(&self) -> &selection::Model {
        &self.selection
    }

    pub fn drawing_tool(&self) -> DrawingTool {
        self.drawing_tool
    }

    pub fn ocr(&self) -> &ocr::Model {
        &self.ocr
    }

    pub fn reduce(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Selection(a) => self
                .selection
                .reduce(a)
                .into_iter()
                .map(Effect::Selection)
                .collect(),

            Action::SelectDrawingTool(tool) => {
                self.drawing_tool = tool;
                vec![Effect::SelectDrawingTool(tool)]
            }

            Action::SaveSelectionToFile => vec![Effect::SaveSelectionToFile],

            Action::SaveSelectionToClipboard => vec![Effect::SaveSelectionToClipboard],

            Action::Undo => vec![Effect::Undo],

            Action::ExtractText => {
                // Ignore re-entrant OCR requests while a job is already running.
                if self.ocr.is_running() {
                    return Vec::new();
                }

                self.ocr.start();
                vec![Effect::ExtractText]
            }

            Action::OcrCompleted {
                has_results,
                is_failed,
                text,
            } => {
                self.ocr.finish();

                let mut effects = Vec::new();
                effects.push(Effect::ShowOcrPreview);

                if has_results {
                    effects.push(Effect::CopyTextToClipboard { text });
                }

                if !has_results || is_failed {
                    effects.push(Effect::ShowOcrNoTextMessage);
                }

                effects.push(Effect::StopOcrEngine);
                effects.push(Effect::HideWindow);
                effects.push(Effect::ResetToInitialState);
                effects
            }

            Action::OcrCancelled => {
                self.ocr.cancel();
                vec![
                    Effect::StopOcrEngine,
                    Effect::HideWindow,
                    Effect::ResetToInitialState,
                ]
            }

            Action::PinSelection => vec![Effect::PinSelection],

            Action::Cancel => {
                // Keep selection core state consistent with the host reset.
                let _ = self.selection.reduce(selection::Action::ResetToIdle);

                vec![Effect::ResetToInitialState, Effect::HideWindow]
            }
        }
    }
}

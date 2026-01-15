use sc_app::selection::RectI32;

use crate::{ChildControlInfo, Result, WindowDetectionManager, WindowInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightKind {
    Window,
    Control,
}

#[derive(Debug, Clone, Copy)]
pub struct HighlightTarget {
    pub rect: RectI32,
    pub kind: HighlightKind,
}

#[derive(Debug, Clone, Copy)]
pub enum AutoHighlightMoveAction {
    /// No changes.
    None,
    /// Set/update the current auto-highlight target.
    SetHighlight(HighlightTarget),
    /// Clear the current auto-highlight target.
    ClearHighlight,
}

#[derive(Debug, Clone, Copy)]
pub struct AutoHighlightMoveArgs {
    pub x: i32,
    pub y: i32,

    pub screen_width: i32,
    pub screen_height: i32,
    /// Current hover-highlight rect from core (if any).
    pub current_highlight: Option<RectI32>,

    /// Whether a manual selection is being created.
    pub selecting: bool,
    /// Whether an existing selection is being interacted with (move/resize).
    pub interacting: bool,
}

/// Auto-highlight controller (Windows-only, because it relies on window/control hit-testing).
pub struct AutoHighlighter {
    detector: WindowDetectionManager,

    /// When disabled, hover no longer updates the selection/highlight.
    enabled: bool,
}

impl Default for AutoHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl AutoHighlighter {
    pub fn new() -> Self {
        Self {
            detector: WindowDetectionManager::new(),
            enabled: true,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }


    /// Reset to the default capture state: enable auto-highlight and clear any active highlight.
    pub fn reset(&mut self) {
        self.enabled = true;
    }

    pub fn start_detection(&mut self) -> Result<()> {
        self.detector.start_detection()
    }

    pub fn refresh_windows(&mut self) -> Result<()> {
        self.detector.refresh_windows()
    }

    pub fn handle_mouse_move(&mut self, args: AutoHighlightMoveArgs) -> AutoHighlightMoveAction {
        // If we're in manual selection/interactions, do not interfere.
        if args.selecting || args.interacting {
            return AutoHighlightMoveAction::None;
        }

        // Hover highlight.
        if !self.enabled {
            return AutoHighlightMoveAction::None;
        }

        let (window_info, control_info) = self.detector.detect_at_point(args.x, args.y);
        let target = pick_target(window_info, control_info).map(|mut t| {
            t.rect = clamp_rect_to_screen(t.rect, args.screen_width, args.screen_height);
            t
        });

        match (args.current_highlight, target) {
            (None, None) => AutoHighlightMoveAction::None,

            (Some(_), None) => {
                AutoHighlightMoveAction::ClearHighlight
            }

            (None, Some(t)) => {
                AutoHighlightMoveAction::SetHighlight(t)
            }

            (Some(prev), Some(next)) => {
                if rect_eq(&prev, &next.rect) {
                    // No visual change.
                    AutoHighlightMoveAction::None
                } else {
                    AutoHighlightMoveAction::SetHighlight(next)
                }
            }
        }
    }

    /// Update auto-highlight state after mouse up.
    ///
    /// Returns `true` if this changes whether we have an active highlight (i.e. a redraw is needed).
    pub fn handle_mouse_up(
        &mut self,
        is_click: bool,
        selection_has_selection: bool,
        had_active_highlight: bool,
    ) -> bool {
        // If a selection exists after mouse-up, auto-highlight should be disabled.
        self.enabled = !selection_has_selection;

        // If a click confirms an auto-highlighted selection, we should stop treating it as auto-highlight.
        if is_click && selection_has_selection && had_active_highlight {
            return true;
        }

        false
    }
}

fn pick_target(
    window_info: Option<WindowInfo>,
    control_info: Option<ChildControlInfo>,
) -> Option<HighlightTarget> {
    if let Some(control) = control_info {
        return Some(HighlightTarget {
            rect: control.rect,
            kind: HighlightKind::Control,
        });
    }

    window_info.map(|w| HighlightTarget {
        rect: w.rect,
        kind: HighlightKind::Window,
    })
}


#[inline]
fn clamp_rect_to_screen(rect: RectI32, screen_width: i32, screen_height: i32) -> RectI32 {
    RectI32 {
        left: rect.left.max(0),
        top: rect.top.max(0),
        right: rect.right.min(screen_width),
        bottom: rect.bottom.min(screen_height),
    }
}

#[inline]
fn rect_eq(a: &RectI32, b: &RectI32) -> bool {
    a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom
}

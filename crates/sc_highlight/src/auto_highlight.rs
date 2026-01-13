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
    /// Transition: disable auto-highlight and begin manual selection at the mouse-down position.
    BeginManualSelection { start_x: i32, start_y: i32 },
}

#[derive(Debug, Clone, Copy)]
pub struct AutoHighlightMoveArgs {
    pub x: i32,
    pub y: i32,

    pub screen_width: i32,
    pub screen_height: i32,

    /// Whether the mouse button is currently pressed.
    pub mouse_pressed: bool,
    /// Mouse-down position (used for drag threshold).
    pub mouse_down_x: i32,
    pub mouse_down_y: i32,
    /// Drag threshold in pixels.
    pub drag_threshold: i32,

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

    /// The current hover-highlighted target (if any).
    active: Option<HighlightTarget>,
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
            active: None,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// True when we are currently showing an auto-highlight selection (hover state).
    pub fn has_active_highlight(&self) -> bool {
        self.active.is_some()
    }

    pub fn active_rect(&self) -> Option<RectI32> {
        self.active.map(|t| t.rect)
    }

    /// Reset to the default capture state: enable auto-highlight and clear any active highlight.
    pub fn reset(&mut self) {
        self.enabled = true;
        self.active = None;
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

        // Transition: while enabled, a drag beyond threshold begins manual selection.
        if self.enabled && args.mouse_pressed {
            if is_drag_threshold_exceeded(
                args.mouse_down_x,
                args.mouse_down_y,
                args.x,
                args.y,
                args.drag_threshold,
            ) {
                self.enabled = false;
                self.active = None;
                return AutoHighlightMoveAction::BeginManualSelection {
                    start_x: args.mouse_down_x,
                    start_y: args.mouse_down_y,
                };
            }

            return AutoHighlightMoveAction::None;
        }

        // Hover highlight.
        if !self.enabled || args.mouse_pressed {
            return AutoHighlightMoveAction::None;
        }

        let (window_info, control_info) = self.detector.detect_at_point(args.x, args.y);
        let target = pick_target(window_info, control_info).map(|mut t| {
            t.rect = clamp_rect_to_screen(t.rect, args.screen_width, args.screen_height);
            t
        });

        match (self.active, target) {
            (None, None) => AutoHighlightMoveAction::None,

            (Some(_), None) => {
                self.active = None;
                AutoHighlightMoveAction::ClearHighlight
            }

            (None, Some(t)) => {
                self.active = Some(t);
                AutoHighlightMoveAction::SetHighlight(t)
            }

            (Some(prev), Some(next)) => {
                if rect_eq(&prev.rect, &next.rect) {
                    // No visual change.
                    self.active = Some(next);
                    AutoHighlightMoveAction::None
                } else {
                    self.active = Some(next);
                    AutoHighlightMoveAction::SetHighlight(next)
                }
            }
        }
    }

    /// Update auto-highlight state after mouse up.
    ///
    /// Returns `true` if this changes whether we have an active highlight (i.e. a redraw is needed).
    pub fn handle_mouse_up(&mut self, is_click: bool, selection_has_selection: bool) -> bool {
        // If a selection exists after mouse-up, auto-highlight should be disabled.
        self.enabled = !selection_has_selection;

        // If a click confirms an auto-highlighted selection, we should stop treating it as auto-highlight.
        if is_click && selection_has_selection && self.active.is_some() {
            self.active = None;
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
fn is_drag_threshold_exceeded(
    start_x: i32,
    start_y: i32,
    current_x: i32,
    current_y: i32,
    threshold: i32,
) -> bool {
    let dx = (current_x - start_x).abs();
    let dy = (current_y - start_y).abs();
    dx > threshold || dy > threshold
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

use sc_drawing::{
    DragMode, Rect as DrawingRect, is_rect_valid,
    update_rect_by_drag as update_drawing_rect_by_drag,
};

/// Platform-neutral integer rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectI32 {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl From<RectI32> for DrawingRect {
    #[inline]
    fn from(r: RectI32) -> Self {
        Self::new(r.left, r.top, r.right, r.bottom)
    }
}

impl From<DrawingRect> for RectI32 {
    #[inline]
    fn from(r: DrawingRect) -> Self {
        Self {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

/// Minimum selection size (in pixels).
///
/// This is a core interaction rule so that host-side selection creation and drag-resize stay
/// consistent and do not drift across platforms.
pub const MIN_BOX_SIZE: i32 = 50;

/// Drag/click threshold (in pixels) used to distinguish click vs drag for selection confirmation.
///
/// This is a core interaction rule so host behavior stays consistent.
pub const DRAG_THRESHOLD: i32 = 5;

/// True if the pointer moved far enough to be considered a drag (vs a click).
#[inline]
pub fn is_drag_threshold_exceeded(
    start_x: i32,
    start_y: i32,
    current_x: i32,
    current_y: i32,
) -> bool {
    let dx = (current_x - start_x).abs();
    let dy = (current_y - start_y).abs();
    dx > DRAG_THRESHOLD || dy > DRAG_THRESHOLD
}

/// Validate a selection rectangle against a minimum size.
///
/// This is intentionally centralized in core so that host-side selection creation and drag-resize
/// can share the same rule.
#[inline]
pub fn validate_min_size(rect: RectI32, min_size: i32) -> Option<RectI32> {
    let r: DrawingRect = rect.into();
    if is_rect_valid(&r, min_size) {
        Some(rect)
    } else {
        None
    }
}

/// Update a rectangle by applying a drag delta.
///
/// This uses the same drag semantics as `sc_drawing::update_rect_by_drag`, but returns a
/// `RectI32` for use by the core selection model.
#[inline]
pub fn update_rect_by_drag(drag_mode: DragMode, dx: i32, dy: i32, original: RectI32) -> RectI32 {
    let original: DrawingRect = original.into();
    update_drawing_rect_by_drag(drag_mode, dx, dy, original).into()
}

/// Update a rectangle by applying a drag delta, and reject the update if it violates `min_size`.
#[inline]
pub fn update_rect_by_drag_validated(
    drag_mode: DragMode,
    dx: i32,
    dy: i32,
    original: RectI32,
    min_size: i32,
) -> Option<RectI32> {
    let updated = update_rect_by_drag(drag_mode, dx, dy, original);
    validate_min_size(updated, min_size)
}

impl RectI32 {
    #[inline]
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    #[inline]
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    /// Construct a normalized rectangle from two points.
    #[inline]
    pub fn from_points(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self {
            left: x1.min(x2),
            top: y1.min(y2),
            right: x1.max(x2),
            bottom: y1.max(y2),
        }
    }

    /// True if both width and height are at least `min_size`.
    #[inline]
    pub fn is_valid_min_size(&self, min_size: i32) -> bool {
        self.width() >= min_size && self.height() >= min_size
    }
}

/// High-level selection phase.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    /// Selection overlay is active and the user may be dragging to create a new selection.
    ///
    /// This is kept in core so host/UI can gradually derive view purely from the core model.
    Selecting {
        selection: Option<RectI32>,
    },
    Editing {
        selection: RectI32,
    },
}

/// Input actions (pure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Host indicates whether the currently visible selection (if any) is an auto-highlight candidate.
    ///
    /// This is derived from platform-specific window/control hit-testing (host side).
    SetAutoHighlightActive { active: bool },

    /// Begin a move/resize interaction for the confirmed selection while editing.
    ///
    /// The host performs hit-testing and provides the drag mode (handle/moving), while the core
    /// applies the geometry rules and emits toolbar position updates.
    BeginEditDrag { drag_mode: DragMode, x: i32, y: i32 },

    /// Update the current edit drag (move/resize).
    EditDragMove { x: i32, y: i32 },

    /// End the current edit drag.
    EndEditDrag,

    /// Set or clear the current hover-highlight selection rect (auto-highlight target).
    ///
    /// This is used to derive the view (mask/border) even while the core phase is `Idle`.
    SetHoverSelection { selection: Option<RectI32> },

    /// Mouse down occurred. Used to determine click vs drag.
    MouseDown { x: i32, y: i32 },

    /// Mouse move occurred.
    ///
    /// When in `Phase::Selecting`, this updates the in-progress selection rectangle based on the
    /// stored mouse-down position.
    MouseMove { x: i32, y: i32 },

    /// Mouse up occurred.
    ///
    /// The core model decides whether to confirm a selection based on:
    /// - click vs drag (threshold)
    /// - manual drag-create geometry (mouse-down position + mouse-up position)
    /// - hover-selection (auto-highlight) when clicking
    MouseUp { x: i32, y: i32 },

    /// Host reset back to idle.
    ResetToIdle,
}

/// Effects requested by the core (executed by the host).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Show the selection toolbar for the given selection.
    ShowToolbar { selection: RectI32 },

    /// Update toolbar position to follow the current selection.
    ///
    /// The host UI layer may ignore this if the toolbar is not visible.
    UpdateToolbarPosition { selection: RectI32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EditInteraction {
    drag_mode: DragMode,
    start_x: i32,
    start_y: i32,
    start_selection: RectI32,
}

/// Selection state machine model.
#[derive(Debug, Default)]
pub struct Model {
    phase: Phase,
    mouse_down_pos: Option<(i32, i32)>,
    auto_highlight_active: bool,
    hover_selection: Option<RectI32>,

    edit_interaction: Option<EditInteraction>,
}

impl Model {
    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// True when the current visible selection (if any) is still an auto-highlight candidate.
    ///
    /// The host can use this to derive view styling (e.g. thicker border) without duplicating
    /// selection confirmation logic.
    pub fn has_auto_highlight(&self) -> bool {
        self.auto_highlight_active
    }

    /// Current hover-highlight rect (auto-highlight target), if any.
    pub fn hover_selection(&self) -> Option<RectI32> {
        self.hover_selection
    }

    /// Selection rect to be used for view derivation.
    ///
    /// This returns:
    /// - `Editing.selection` when editing
    /// - `Selecting.selection` when selecting (drag)
    /// - `hover_selection` when idle or selecting without a drag selection yet
    pub fn visible_selection(&self) -> Option<RectI32> {
        match &self.phase {
            Phase::Idle => self.hover_selection,
            Phase::Selecting { selection } => selection.or(self.hover_selection),
            Phase::Editing { selection } => Some(*selection),
        }
    }

    pub fn reduce(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::SetAutoHighlightActive { active } => {
                self.auto_highlight_active = active;
                Vec::new()
            }

            Action::BeginEditDrag { drag_mode, x, y } => {
                // Any explicit manipulation of the selection should treat it as confirmed.
                self.auto_highlight_active = false;
                self.hover_selection = None;

                if let Phase::Editing { selection } = self.phase {
                    self.edit_interaction = Some(EditInteraction {
                        drag_mode,
                        start_x: x,
                        start_y: y,
                        start_selection: selection,
                    });
                }

                Vec::new()
            }

            Action::EditDragMove { x, y } => {
                let Some(interaction) = self.edit_interaction else {
                    return Vec::new();
                };

                if let Phase::Editing { selection: current } = self.phase {
                    let dx = x - interaction.start_x;
                    let dy = y - interaction.start_y;

                    if let Some(updated) = update_rect_by_drag_validated(
                        interaction.drag_mode,
                        dx,
                        dy,
                        interaction.start_selection,
                        MIN_BOX_SIZE,
                    ) {
                        if updated != current {
                            self.phase = Phase::Editing { selection: updated };
                            return vec![Effect::UpdateToolbarPosition { selection: updated }];
                        }
                    }
                }

                Vec::new()
            }

            Action::EndEditDrag => {
                self.edit_interaction = None;
                Vec::new()
            }

            Action::SetHoverSelection { selection } => {
                self.hover_selection = selection;
                Vec::new()
            }

            Action::MouseDown { x, y } => {
                self.mouse_down_pos = Some((x, y));

                // Enter selecting on mouse-down when idle.
                //
                // This keeps the host simpler: it can route raw mouse input and the core model
                // derives phase transitions.
                if let Phase::Idle = self.phase {
                    self.phase = Phase::Selecting { selection: None };
                }

                Vec::new()
            }

            Action::MouseMove { x, y } => {
                // Only update in-progress selection while selecting.
                if let Phase::Selecting { selection } = &mut self.phase {
                    if let Some((sx, sy)) = self.mouse_down_pos {
                        // If we're currently in auto-highlight mode and the user hasn't started a
                        // drag selection yet, do not begin a drag selection until the drag
                        // threshold is exceeded. This prevents small jitter from showing a drag
                        // box when the intent was to click-confirm the hover selection.
                        if self.auto_highlight_active
                            && selection.is_none()
                            && !is_drag_threshold_exceeded(sx, sy, x, y)
                        {
                            return Vec::new();
                        }

                        // Drag selection has started (either auto-highlight threshold exceeded, or
                        // auto-highlight is not active). Clear hover-highlight state so view derives
                        // from the drag selection.
                        if self.auto_highlight_active && selection.is_none() {
                            self.auto_highlight_active = false;
                            self.hover_selection = None;
                        }

                        *selection = Some(RectI32::from_points(sx, sy, x, y));
                    }
                }

                Vec::new()
            }

            Action::ResetToIdle => {
                self.phase = Phase::Idle;
                self.mouse_down_pos = None;
                self.auto_highlight_active = false;
                self.hover_selection = None;
                self.edit_interaction = None;
                Vec::new()
            }

            Action::MouseUp { x, y } => {
                let was_selecting_phase = matches!(self.phase, Phase::Selecting { .. });

                // Only meaningful in Selecting; ignore otherwise (keeps model robust if host doesn't route).
                if !was_selecting_phase {
                    self.mouse_down_pos = None;
                    return Vec::new();
                }

                let mouse_down_pos = self.mouse_down_pos.take();
                let is_click = mouse_down_pos
                    .map_or(false, |(sx, sy)| !is_drag_threshold_exceeded(sx, sy, x, y));

                // Candidate selection:
                // - Click: confirm hover-selection (auto-highlight) if present (no min-size rule).
                // - Drag: confirm drag-created selection if it satisfies min-size.
                let selection = if is_click {
                    self.hover_selection
                } else {
                    mouse_down_pos
                        .map(|(sx, sy)| RectI32::from_points(sx, sy, x, y))
                        .and_then(|r| validate_min_size(r, MIN_BOX_SIZE))
                };

                // Any confirmation clears auto-highlight candidate state.
                self.auto_highlight_active = false;
                self.hover_selection = None;

                match selection {
                    Some(sel) => {
                        self.phase = Phase::Editing { selection: sel };
                        vec![Effect::ShowToolbar { selection: sel }]
                    }
                    None => {
                        self.phase = Phase::Idle;
                        Vec::new()
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn drag_selection_shows_toolbar_and_enters_editing() {
        let mut m = super::Model::default();
        m.reduce(super::Action::MouseDown { x: 0, y: 0 });

        m.reduce(super::Action::MouseMove { x: 200, y: 200 });

        let eff = m.reduce(super::Action::MouseUp { x: 200, y: 200 });

        let expected = super::RectI32 {
            left: 0,
            top: 0,
            right: 200,
            bottom: 200,
        };

        assert_eq!(
            m.phase(),
            &super::Phase::Editing {
                selection: expected
            }
        );
        assert_eq!(
            eff,
            vec![super::Effect::ShowToolbar {
                selection: expected
            }]
        );
        assert!(!m.has_auto_highlight());
    }

    #[test]
    fn click_confirms_hover_selection_and_shows_toolbar() {
        let mut m = super::Model::default();

        // Hover selection can be smaller than MIN_BOX_SIZE (auto-highlight candidates).
        let hover = super::RectI32 {
            left: 10,
            top: 10,
            right: 20,
            bottom: 20,
        };
        m.reduce(super::Action::SetHoverSelection {
            selection: Some(hover),
        });
        m.reduce(super::Action::SetAutoHighlightActive { active: true });

        m.reduce(super::Action::MouseDown { x: 0, y: 0 });
        let eff = m.reduce(super::Action::MouseUp { x: 1, y: 1 });

        assert_eq!(m.phase(), &super::Phase::Editing { selection: hover });
        assert_eq!(eff, vec![super::Effect::ShowToolbar { selection: hover }]);
        assert!(!m.has_auto_highlight());
    }

    #[test]
    fn editing_drag_does_not_show_toolbar_but_stays_in_editing() {
        let mut m = super::Model::default();
        m.reduce(super::Action::MouseDown { x: 0, y: 0 });

        // First confirm the selection to enter editing.
        m.reduce(super::Action::MouseMove { x: 200, y: 200 });
        let _ = m.reduce(super::Action::MouseUp { x: 200, y: 200 });

        let sel = match m.phase() {
            super::Phase::Editing { selection } => *selection,
            _ => panic!("expected editing"),
        };

        // Then simulate a mouse-up while editing: should be ignored.
        m.reduce(super::Action::MouseDown { x: 0, y: 0 });
        let eff = m.reduce(super::Action::MouseUp { x: 10, y: 0 });

        assert_eq!(m.phase(), &super::Phase::Editing { selection: sel });
        assert!(eff.is_empty());
        assert!(!m.has_auto_highlight());
    }

    #[test]
    fn edit_drag_move_updates_editing_rect_and_requests_toolbar_position_update() {
        let mut m = super::Model::default();

        // Enter editing first.
        m.reduce(super::Action::MouseDown { x: 0, y: 0 });
        m.reduce(super::Action::MouseMove { x: 200, y: 200 });
        let _ = m.reduce(super::Action::MouseUp { x: 200, y: 200 });

        m.reduce(super::Action::SetAutoHighlightActive { active: true });
        assert!(m.has_auto_highlight());

        // Begin a move drag and move by (10, 20).
        m.reduce(super::Action::BeginEditDrag {
            drag_mode: super::DragMode::Moving,
            x: 0,
            y: 0,
        });
        let eff = m.reduce(super::Action::EditDragMove { x: 10, y: 20 });

        let expected = super::RectI32 {
            left: 10,
            top: 20,
            right: 210,
            bottom: 220,
        };

        assert_eq!(
            m.phase(),
            &super::Phase::Editing {
                selection: expected
            }
        );
        assert_eq!(
            eff,
            vec![super::Effect::UpdateToolbarPosition {
                selection: expected
            }]
        );
        assert!(!m.has_auto_highlight());
    }

    #[test]
    fn edit_drag_resize_rejects_below_min_size() {
        let mut m = super::Model::default();

        // Confirm a valid selection.
        m.reduce(super::Action::MouseDown { x: 0, y: 0 });
        m.reduce(super::Action::MouseMove { x: 100, y: 100 });
        let _ = m.reduce(super::Action::MouseUp { x: 100, y: 100 });

        let original = match m.phase() {
            super::Phase::Editing { selection } => *selection,
            _ => panic!("expected editing"),
        };

        // Try to shrink to 20x20 (invalid for MIN_BOX_SIZE=50).
        m.reduce(super::Action::BeginEditDrag {
            drag_mode: super::DragMode::ResizingTopLeft,
            x: 0,
            y: 0,
        });
        let eff = m.reduce(super::Action::EditDragMove { x: 80, y: 80 });

        assert_eq!(
            m.phase(),
            &super::Phase::Editing {
                selection: original
            }
        );
        assert!(eff.is_empty());
    }

    #[test]
    fn no_selection_enters_idle() {
        let mut m = super::Model::default();
        m.reduce(super::Action::MouseDown { x: 0, y: 0 });
        m.reduce(super::Action::SetAutoHighlightActive { active: true });

        m.reduce(super::Action::MouseMove { x: 2, y: 2 });
        let eff = m.reduce(super::Action::MouseUp { x: 2, y: 2 });

        assert_eq!(m.phase(), &super::Phase::Idle);
        assert!(eff.is_empty());
        assert!(!m.has_auto_highlight());
    }

    #[test]
    fn auto_highlight_can_be_toggled_explicitly() {
        let mut m = super::Model::default();
        assert!(!m.has_auto_highlight());

        m.reduce(super::Action::SetAutoHighlightActive { active: true });
        assert!(m.has_auto_highlight());

        m.reduce(super::Action::SetAutoHighlightActive { active: false });
        assert!(!m.has_auto_highlight());
    }

    #[test]
    fn hover_selection_is_used_for_visible_selection_while_idle() {
        let mut m = super::Model::default();
        let rect = super::RectI32 {
            left: 1,
            top: 2,
            right: 3,
            bottom: 4,
        };

        assert_eq!(m.visible_selection(), None);
        m.reduce(super::Action::SetHoverSelection {
            selection: Some(rect),
        });
        assert_eq!(m.visible_selection(), Some(rect));

        m.reduce(super::Action::SetHoverSelection { selection: None });
        assert_eq!(m.visible_selection(), None);
    }

    #[test]
    fn validate_min_size_rejects_small_rectangles() {
        let rect = super::RectI32 {
            left: 0,
            top: 0,
            right: 10,
            bottom: 10,
        };

        assert_eq!(super::validate_min_size(rect, super::MIN_BOX_SIZE), None);
        assert_eq!(super::validate_min_size(rect, 5), Some(rect));
    }

    #[test]
    fn update_rect_by_drag_validated_matches_drawing_geometry() {
        let original = super::RectI32 {
            left: 10,
            top: 10,
            right: 100,
            bottom: 100,
        };

        let updated =
            super::update_rect_by_drag_validated(super::DragMode::Moving, 5, 5, original, 50)
                .unwrap();
        assert_eq!(
            updated,
            super::RectI32 {
                left: 15,
                top: 15,
                right: 105,
                bottom: 105,
            }
        );

        let updated = super::update_rect_by_drag_validated(
            super::DragMode::ResizingBottomRight,
            10,
            10,
            original,
            50,
        )
        .unwrap();
        assert_eq!(
            updated,
            super::RectI32 {
                left: 10,
                top: 10,
                right: 110,
                bottom: 110,
            }
        );

        // Reject updates that would break min-size.
        let too_small = super::update_rect_by_drag_validated(
            super::DragMode::ResizingBottomRight,
            -100,
            -100,
            original,
            50,
        );
        assert!(too_small.is_none());
    }
}

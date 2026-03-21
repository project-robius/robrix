//! A loading indicator showing the status of room restoration operations.
//!
//! This shows a loading spinner and short status label to inform the user
//! that a room they were viewing previously has not yet been received
//! from the Matrix homeserver, or that all rooms have been received but that
//! the current room no longer exists.

use makepad_widgets::*;
use crate::utils::RoomNameId;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;

    pub RestoreStatusView = {{RestoreStatusView}} {
        width: Fill, height: Fit,
        flow: Down,
        align: {x: 0.5, y: 0.5},
        padding: 20,

        restore_status_spinner = <LoadingSpinner> {
            width: 50,
            height: 50,
            visible: true,
            draw_bg: {
                color: (COLOR_ACTIVE_PRIMARY)
                border_size: 3.0,
            }
        }

        restore_status_label = <Label> {
            width: Fill, height: Fit,
            align: {x: 0.5, y: 0.0},
            padding: {left: 5.0, right: 0.0}
            margin: {top: 10.0},
            flow: RightWrap,
            draw_text: {
                color: (TYPING_NOTICE_TEXT_COLOR),
            }
        }
    }
}

/// A view that displays a spinner and a label to indicate that a restore operation is in progress for a room.
#[derive(Live, LiveHook, Widget)]
pub struct RestoreStatusView {
    #[deref] view: View,
    #[live(true)] visible: bool,
}

impl Widget for RestoreStatusView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.visible {
            self.view.handle_event(cx, event, scope);
        }
    }
    
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.visible {
            self.view.draw_walk(cx, scope, walk)
        } else {
            DrawStep::done()
        }
    }
}

impl RestoreStatusViewRef {
    /// Sets whether the restore status view is visible or not.
    ///
    /// When the view is not visible, the label is cleared of any text content.
    /// When the view becomes visible, you must call [`Self::set_content()`] again.
    pub fn set_visible(&self, cx: &mut Cx, visible: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = visible;
            if !visible {
                inner.label(ids!(restore_status_label))
                    .set_text(cx, "");
            }
        }
    }

    /// Sets the text displayed in the restore status view based on the given parameters.
    ///
    /// If `all_rooms_loaded` is true, the text will be a message indicating that the room
    /// was not found in the homeserver's list of all rooms, and that the user can close
    /// the parent room view.
    ///
    /// If `all_rooms_loaded` is false, the text will be a message indicating that the room
    /// is still being loaded from the homeserver.
    ///
    /// The `room_name` parameter is used to fill in the room name in the error message.
    /// Its `Display` implementation automatically handles Empty names by falling back to the room ID.
    pub fn set_content(
        &self,
        cx: &mut Cx,
        all_rooms_loaded: bool,
        room_name: &RoomNameId,
    ) {
        let Some(inner) = self.borrow() else { return };
        let restore_status_spinner = inner.view.view(ids!(restore_status_spinner));
        let restore_status_label = inner.view.label(ids!(restore_status_label));
        if all_rooms_loaded {
            restore_status_spinner.set_visible(cx, false);
            restore_status_label.set_text(
                cx,
                &format!(
                    "Room {room_name} was not found in the homeserver's list \
                    of all rooms.\n\nYou may close this screen."
                ),
            );
        } else {
            restore_status_spinner.set_visible(cx, true);
            restore_status_label.set_text(
                cx,
                "Waiting for this room to be loaded from the homeserver",
            );
        }
    }
}

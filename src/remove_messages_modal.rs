//! A modal dialog for removing a user's messages from a room.
//!
//! It lists all of that user's messages that we can see, and the user chooses which ones to remove,
//! then confirms that choice on a second step, since this can't be undone.
//! They're removed in the background, which keeps going if this modal gets hidden.

use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use rustc_hash::FxHashSet;
use tokio::sync::oneshot;

use crate::{
    moderation::{RemoveMessagesUpdate, UserMessage},
    moderation_modal::get_entered_reason,
    shared::{
        list_rows::{LIST_ROW_HEIGHT, handle_row_actions},
        popup_list::{PopupKind, enqueue_popup_notification},
        progress_bar::ProgressBarWidgetExt,
        styles::set_view_bg_color,
    },
    sliding_sync::{MatrixRequest, submit_async_request},
};

/// How much of the window's height the list of messages can take up.
const MAX_LIST_HEIGHT_FRACTION: f64 = 0.4;
/// How long to show a popup about a removal that finished while this modal was hidden, in seconds.
const RESULT_POPUP_DURATION: f64 = 8.0;


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.RemoveMessagesModal = set_type_default() do #(RemoveMessagesModal::register_widget(vm)) {
        ..mod.widgets.LargeModal

        color: (COLOR_TRANSPARENT)
        color_hover: (COLOR_LIST_ITEM_BG_HOVER)
        remove_icon_color: (COLOR_FG_DANGER_RED)
        remove_icon_color_disabled: (COLOR_FG_DISABLED)

        title := ModalTitle { margin: Inset{bottom: 12} }

        body := ModalBody {}

        // This goes above the list, where it can't end up beneath a soft keyboard.
        reason_view := mod.widgets.ModerationReasonView {}

        select_view := View {
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true, row_align: RowAlign.Center}, spacing: 8
            wrap_spacing: 8
            margin: Inset{top: 15, left: 6}

            select_all_button := mod.widgets.SelectRowsButton { text: "Select All" }
            select_none_button := mod.widgets.SelectRowsButton { text: "Select None" }

            // This sits outside of the list so that it's always in view.
            loading_view := View {
                width: Fit, height: Fit
                flow: Right, spacing: 8
                align: Align{y: 0.5}
                margin: Inset{left: 6}

                LoadingSpinner {
                    width: 18, height: 18
                    draw_bg +: { color: (COLOR_ACTIVE_PRIMARY), stroke_width: 2.5 }
                }
                Label {
                    width: Fit, height: Fit
                    padding: 0
                    draw_text +: { color: (COLOR_LIST_ROW_TEXT_MUTED), text_style: theme.font_regular {font_size: 10} }
                    text: "Loading messages…"
                }
            }
        }

        // Its height is set whenever it's drawn, since a PortalList can't fit itself to its rows.
        messages_view := View {
            width: Fill, height: 0
            margin: Inset{top: 8}

            messages_list := PortalList {
                width: Fill, height: Fill
                flow: Down
                scroll_bar: ListScrollBar {}
                auto_tail: false
                keep_invisible: false
                emit_scroll_actions: false

                message_row := RoundedView {
                    width: Fill, height: #(LIST_ROW_HEIGHT)
                    flow: Right
                    align: Align{y: 0.5}
                    padding: Inset{left: 6, right: 12}
                    show_bg: true, cursor: MouseCursor.Hand
                    grab_key_focus: false
                    draw_bg +: { color: (COLOR_TRANSPARENT), border_radius: 5.0 }

                    // A real CheckBox would flip itself when a scroll is released over it, so the row takes all taps.
                    selection_box := RoundedView {
                        width: 20, height: 20
                        margin: Inset{right: 10}
                        align: Align{x: 0.5, y: 0.5}
                        show_bg: true
                        draw_bg +: {
                            color: (COLOR_BG_ACCENT_TINT)
                            border_size: 0.0
                            border_radius: 2.0
                        }

                        // An Icon ignores `set_visible`, so this wrapper is what hides the checkmark.
                        checkmark := View {
                            width: Fit, height: Fit
                            Icon {
                                icon_walk: Walk{width: 11, height: 11}
                                draw_icon +: { svg: (ICON_CHECKMARK), color: (COLOR_ACTIVE_PRIMARY_DARKER) }
                            }
                        }
                    }

                    info := View {
                        width: Fill, height: Fit, flow: Down, spacing: 1
                        preview := Label {
                            width: Fill, height: Fit, max_lines: 1, text_overflow: Ellipsis, padding: 0
                            draw_text +: { color: (COLOR_TEXT), text_style: theme.font_regular {font_size: 11, line_spacing: 1.0} }
                        }
                        details := Label {
                            width: Fill, height: Fit, max_lines: 1, text_overflow: Ellipsis, padding: 0
                            draw_text +: { color: (COLOR_LIST_ROW_TEXT_MUTED), text_style: theme.font_regular {font_size: 9.5, line_spacing: 1.0} }
                        }
                    }
                }
            }
        }

        error_label := Label {
            visible: false
            width: Fill, height: Fit
            flow: Flow.Right{wrap: true}
            margin: Inset{top: 10}
            draw_text +: { color: (COLOR_FG_DANGER_RED), text_style: REGULAR_TEXT {font_size: 10.5} }
        }

        progress_bar := ProgressBar {
            visible: false
            margin: Inset{top: 18}
        }

        buttons_view := ModalButtonsRow {
            cancel_button := RobrixNeutralIconButton {
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_CLOSE)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Cancel"
            }

            back_button := RobrixNeutralIconButton {
                visible: false
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_ARROW_LEFT)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Go Back"
            }

            remove_button := RobrixNegativeIconButton {
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_TRASH)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Remove Messages"
            }

            confirm_button := RobrixNegativeIconButton {
                visible: false
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_TRASH)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Yes, Remove"
            }

            stop_button := RobrixNegativeIconButton {
                visible: false
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_FORBIDDEN)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Stop"
            }

            okay_button := RobrixIconButton {
                visible: false
                width: Fit{min: FitBound.Abs(120.0)}
                align: Align{x: 0.5, y: 0.5}
                padding: 15,
                draw_icon.svg: (ICON_CHECKMARK)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Okay"
            }
        }
    }
}

/// A request to remove some of a user's messages from a room.
#[derive(Clone, Debug)]
pub struct RemoveMessagesRequest {
    pub room_id: OwnedRoomId,
    pub room_name: String,
    pub user_id: OwnedUserId,
    pub user_name: String,
    /// The reason to fill in up front, e.g., one that was just given for banning this user.
    pub initial_reason: Option<String>,
}

/// Actions handled by the parent widget of the [`RemoveMessagesModal`].
#[derive(Clone, Debug)]
pub enum RemoveMessagesModalAction {
    /// The modal should be opened by its parent widget to handle the given request.
    Open(RemoveMessagesRequest),
    /// The modal requested its parent widget to close it.
    Close,
    /// The parent widget should pass this on to the modal, which gets no events while it's hidden.
    Update(RemoveMessagesUpdate),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RemoveMessagesModalState {
    /// We aren't showing or doing anything.
    #[default]
    Idle,
    /// Waiting for the rest of the user's messages, which can already be selected.
    WaitingForMessages,
    /// Waiting for the user to choose which messages to remove.
    WaitingForUserInput,
    /// Removing the chosen messages.
    Removing,
    /// We're showing the final success or failure message.
    Finished,
}

#[derive(Script, ScriptHook, Widget)]
pub struct RemoveMessagesModal {
    #[deref] view: View,

    #[live] color: Vec4f,
    #[live] color_hover: Vec4f,
    #[live] remove_icon_color: Vec4f,
    #[live] remove_icon_color_disabled: Vec4f,

    #[rust] state: RemoveMessagesModalState,
    #[rust] request: Option<RemoveMessagesRequest>,
    /// Identifies the latest fetch, so that we can ignore anything from an earlier one.
    #[rust] fetch_id: u64,
    /// Dropping this stops fetching the user's messages.
    #[rust] stop_fetching: Option<oneshot::Sender<()>>,
    /// Dropping this stops removing the user's messages.
    #[rust] stop_removing: Option<oneshot::Sender<()>>,
    /// The user's messages, newest first.
    #[rust] messages: Vec<UserMessage>,
    /// The indices of the selected messages.
    #[rust] selected_messages: FxHashSet<usize>,
    /// Whether messages that are still being fetched should start out selected.
    #[rust] should_select_new_messages: bool,
    /// The exact messages we're asking the user to confirm, since more can get selected as they're fetched.
    #[rust] messages_to_confirm: Option<Vec<UserMessage>>,
    #[rust] num_messages_to_remove: usize,
    /// Whether this modal was hidden while removal keeps going in the background.
    #[rust] is_hidden: bool,
    #[rust] hovered_row: Option<usize>,
}

impl Widget for RemoveMessagesModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let list_height = (self.messages.len() as f64 * LIST_ROW_HEIGHT)
            .min(cx.current_pass_size().y * MAX_LIST_HEIGHT_FRACTION);
        if let Some(mut messages_view) = self.view.view(cx, ids!(messages_view)).borrow_mut() {
            messages_view.walk.height = Size::Fixed(list_height);
        }

        while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
            let list_ref = step.as_portal_list();
            let Some(mut list) = list_ref.borrow_mut() else { continue };
            list.set_item_range(cx, 0, self.messages.len());
            while let Some(index) = list.next_visible_item(cx) {
                let Some(message) = self.messages.get(index) else { continue };
                let row = list.item(cx, index, id!(message_row));
                row.child_by_path(ids!(info.preview)).set_text(cx, &message.preview);
                row.child_by_path(ids!(info.details)).set_text(cx, &message.details);
                row.child_by_path(ids!(checkmark)).set_visible(cx, self.selected_messages.contains(&index));
                let color = if self.hovered_row == Some(index) { self.color_hover } else { self.color };
                set_view_bg_color(cx, &row.as_view(), color);
                row.draw_all(cx, scope);
            }
        }
        DrawStep::done()
    }
}

impl WidgetMatchEvent for RemoveMessagesModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let reason_input = self.view.text_input(cx, ids!(reason_input));

        // Closing this modal for any reason lands here, even when we asked for it ourselves.
        if actions.iter().any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed))) {
            self.hide_keyboard(cx);
            if self.state == RemoveMessagesModalState::Removing {
                self.is_hidden = true;
                enqueue_popup_notification(
                    "Still removing messages in the background.",
                    PopupKind::Info,
                    Some(4.0),
                );
            } else {
                self.reset();
            }
            return;
        }

        if self.view.button(cx, ids!(cancel_button)).clicked(actions)
            || self.view.button(cx, ids!(okay_button)).clicked(actions)
        {
            cx.action(RemoveMessagesModalAction::Close);
            return;
        }

        let stop_button = self.view.button(cx, ids!(stop_button));
        if stop_button.clicked(actions) {
            self.stop_removing = None;
            stop_button.set_text(cx, "Stopping…");
            self.set_negative_button_enabled(cx, ids!(stop_button), false);
            self.redraw(cx);
        }

        if !matches!(self.state, RemoveMessagesModalState::WaitingForMessages | RemoveMessagesModalState::WaitingForUserInput) {
            return;
        }

        if self.messages_to_confirm.is_some() {
            if self.view.button(cx, ids!(back_button)).clicked(actions) {
                self.show_selection_step(cx);
            } else if self.view.button(cx, ids!(confirm_button)).clicked(actions)
                && let Some(messages) = self.messages_to_confirm.take()
            {
                self.start_removing(cx, messages, get_entered_reason(&reason_input));
            }
            return;
        }

        let select_all_clicked = self.view.button(cx, ids!(select_all_button)).clicked(actions);
        if select_all_clicked || self.view.button(cx, ids!(select_none_button)).clicked(actions) {
            self.selected_messages.clear();
            if select_all_clicked {
                self.selected_messages.extend(0..self.messages.len());
            }
            self.should_select_new_messages = select_all_clicked;
            self.update_remove_button(cx);
            self.redraw(cx);
        }

        let list = self.view.portal_list(cx, ids!(messages_list));
        let (tapped_row, hover_changed) = handle_row_actions(
            list.items_with_actions(actions),
            list.was_scrolling(),
            actions,
            &mut self.hovered_row,
        );
        if let Some(index) = tapped_row
            && !self.selected_messages.remove(&index)
        {
            self.selected_messages.insert(index);
        }
        if tapped_row.is_some() {
            self.update_remove_button(cx);
        }
        if tapped_row.is_some() || hover_changed {
            self.redraw(cx);
        }

        let was_requested = self.view.button(cx, ids!(remove_button)).clicked(actions)
            || reason_input.returned(actions).is_some();
        if was_requested && !self.selected_messages.is_empty() {
            self.show_confirmation_step(cx);
        }
    }
}

impl RemoveMessagesModal {
    /// Shows how many messages are selected on the remove button, which needs at least one of them.
    fn update_remove_button(&mut self, cx: &mut Cx) {
        let text = match self.selected_messages.len() {
            0 => String::from("Remove Messages"),
            1 => String::from("Remove 1 Message"),
            count => format!("Remove {count} Messages"),
        };
        self.view.button(cx, ids!(remove_button)).set_text(cx, &text);
        self.set_negative_button_enabled(cx, ids!(remove_button), !self.selected_messages.is_empty());
    }

    /// Sets whether the given negative button can be clicked, and makes it look that way too.
    fn set_negative_button_enabled(&mut self, cx: &mut Cx, button: &[LiveId], is_enabled: bool) {
        let button = self.view.button(cx, button);
        button.set_enabled(cx, is_enabled);
        button.set_disabled(cx, !is_enabled);
        if let Some(mut button) = button.borrow_mut() {
            button.draw_icon.color = if is_enabled { self.remove_icon_color } else { self.remove_icon_color_disabled };
        }
    }

    /// Hides the keyboard of the reason input, which won't get its focus-lost event once it's hidden.
    fn hide_keyboard(&mut self, cx: &mut Cx) {
        if self.view.text_input(cx, ids!(reason_input)).key_focus(cx) {
            cx.hide_text_ime();
            cx.set_key_focus(self.view.area());
        }
    }

    /// Shows or hides everything that the user needs for choosing messages.
    fn set_selection_visible(&mut self, cx: &mut Cx, is_visible: bool) {
        self.view.view(cx, ids!(select_view)).set_visible(cx, is_visible);
        self.view.view(cx, ids!(messages_view)).set_visible(cx, is_visible);
        self.view.view(cx, ids!(reason_view)).set_visible(cx, is_visible);
        self.view.button(cx, ids!(remove_button)).set_visible(cx, is_visible);
    }

    /// Swaps the buttons and everything else between choosing messages and confirming their removal.
    fn set_confirmation_visible(&mut self, cx: &mut Cx, is_visible: bool) {
        self.set_selection_visible(cx, !is_visible);
        self.view.button(cx, ids!(cancel_button)).set_visible(cx, !is_visible);
        self.view.button(cx, ids!(back_button)).set_visible(cx, is_visible);
        self.view.button(cx, ids!(confirm_button)).set_visible(cx, is_visible);
        for button in [ids!(cancel_button), ids!(remove_button), ids!(back_button), ids!(confirm_button)] {
            self.view.button(cx, button).reset_hover(cx);
        }
    }

    /// Shows the instructions for choosing which messages to remove.
    fn show_selection_text(&mut self, cx: &mut Cx, request: &RemoveMessagesRequest) {
        self.view.label(cx, ids!(title)).set_text(cx, &format!("Remove messages from {}?", request.user_name));
        self.view.label(cx, ids!(body)).set_text(cx, &format!(
            "Select which of their messages to remove from \"{}\". This can't be undone.",
            request.room_name,
        ));
    }

    /// Goes back to choosing messages, with the selection and reason just as the user left them.
    fn show_selection_step(&mut self, cx: &mut Cx) {
        let Some(request) = self.request.clone() else { return };
        self.messages_to_confirm = None;
        self.show_selection_text(cx, &request);
        self.set_confirmation_visible(cx, false);
        self.redraw(cx);
    }

    /// Asks the user to confirm removing exactly the messages that are selected right now.
    fn show_confirmation_step(&mut self, cx: &mut Cx) {
        let Some(request) = self.request.as_ref() else { return };
        let messages: Vec<UserMessage> = self.messages.iter().enumerate()
            .filter(|(index, _)| self.selected_messages.contains(index))
            .map(|(_, message)| message.clone())
            .collect();
        let body = format!(
            "You're about to remove {} from {} in \"{}\" for everyone. This can't be undone.",
            format_message_count(messages.len()),
            request.user_name,
            request.room_name,
        );
        self.messages_to_confirm = Some(messages);
        self.hide_keyboard(cx);
        self.view.label(cx, ids!(title)).set_text(cx, "Are you sure?");
        self.view.label(cx, ids!(body)).set_text(cx, &body);
        self.set_confirmation_visible(cx, true);
        self.redraw(cx);
    }

    fn finish_fetching(&mut self, cx: &mut Cx, error: Option<&str>) {
        self.stop_fetching = None;
        self.state = RemoveMessagesModalState::WaitingForUserInput;
        self.view.view(cx, ids!(loading_view)).set_visible(cx, false);

        if !self.messages.is_empty() {
            let error_label = self.view.label(cx, ids!(error_label));
            error_label.set_visible(cx, error.is_some());
            if let Some(error) = error {
                error_label.set_text(cx, &format!("Couldn't load all of their messages: {error}"));
            }
            return;
        }
        let Some(request) = self.request.as_ref() else { return };
        let description = match error {
            Some(error) => format!("Couldn't load {}'s messages.\n\nError: {error}", request.user_name),
            None => format!("{} has no messages in \"{}\" that you can see.", request.user_name, request.room_name),
        };
        let title = if error.is_some() { "Error loading messages!" } else { "No messages to remove" };
        self.show_final_state(cx, title, &description);
    }

    fn start_removing(&mut self, cx: &mut Cx, messages: Vec<UserMessage>, reason: Option<String>) {
        let Some(request) = self.request.as_ref() else { return };
        let (stop_removing, stop) = oneshot::channel();
        self.stop_removing = Some(stop_removing);
        self.num_messages_to_remove = messages.len();
        submit_async_request(MatrixRequest::RemoveUserMessages {
            room_id: request.room_id.clone(),
            messages,
            reason,
            stop,
        });

        self.stop_fetching = None;
        self.state = RemoveMessagesModalState::Removing;
        self.view.label(cx, ids!(title)).set_text(cx, "Removing messages…");
        self.view.label(cx, ids!(error_label)).set_visible(cx, false);
        self.view.widget(cx, ids!(progress_bar)).set_visible(cx, true);
        self.view.button(cx, ids!(back_button)).set_visible(cx, false);
        self.view.button(cx, ids!(confirm_button)).set_visible(cx, false);
        // Removal keeps going without this modal, so "Cancel" would be a lie.
        let cancel_button = self.view.button(cx, ids!(cancel_button));
        cancel_button.set_text(cx, "Hide");
        cancel_button.set_visible(cx, true);
        cancel_button.reset_hover(cx);
        let stop_button = self.view.button(cx, ids!(stop_button));
        stop_button.set_text(cx, "Stop");
        stop_button.set_visible(cx, true);
        stop_button.reset_hover(cx);
        self.set_negative_button_enabled(cx, ids!(stop_button), true);
        self.show_progress(cx, 0, 0);
    }

    fn show_progress(&mut self, cx: &mut Cx, num_removed: usize, num_failed: usize) {
        let Some(request) = self.request.as_ref() else { return };
        let total = self.num_messages_to_remove;
        self.view.label(cx, ids!(body)).set_text(cx, &format!(
            "Removed {num_removed} of {} from {}.",
            format_message_count(total),
            request.user_name,
        ));
        self.view.progress_bar(cx, ids!(progress_bar)).set_progress(
            cx,
            (num_removed + num_failed) as f32 / total.max(1) as f32,
        );
        self.redraw(cx);
    }

    /// Shows the given message with just an okay button.
    fn show_final_state(&mut self, cx: &mut Cx, title: &str, description: &str) {
        self.state = RemoveMessagesModalState::Finished;
        self.view.label(cx, ids!(title)).set_text(cx, title);
        self.view.label(cx, ids!(body)).set_text(cx, description);
        self.set_selection_visible(cx, false);
        self.view.widget(cx, ids!(progress_bar)).set_visible(cx, false);
        self.view.button(cx, ids!(cancel_button)).set_visible(cx, false);
        self.view.button(cx, ids!(stop_button)).set_visible(cx, false);
        let okay_button = self.view.button(cx, ids!(okay_button));
        okay_button.set_visible(cx, true);
        okay_button.reset_hover(cx);
        self.redraw(cx);
    }

    /// Forgets the current request, stopping any fetch that's still going.
    fn reset(&mut self) {
        self.state = RemoveMessagesModalState::Idle;
        self.request = None;
        self.stop_fetching = None;
        self.messages.clear();
        self.selected_messages.clear();
        self.messages_to_confirm = None;
        self.hovered_row = None;
    }

    /// Shows the given request, or the removal in progress if there is one.
    fn show(&mut self, cx: &mut Cx, request: RemoveMessagesRequest) {
        self.is_hidden = false;
        if self.state == RemoveMessagesModalState::Removing {
            if let Some(current_request) = self.request.as_ref()
                && (current_request.user_id != request.user_id || current_request.room_id != request.room_id)
            {
                enqueue_popup_notification(
                    format!(
                        "Still removing messages from {}, so {}'s messages can't be removed yet.",
                        current_request.user_name,
                        request.user_name,
                    ),
                    PopupKind::Warning,
                    Some(RESULT_POPUP_DURATION),
                );
            }
            self.view.button(cx, ids!(cancel_button)).reset_hover(cx);
            self.view.button(cx, ids!(stop_button)).reset_hover(cx);
            return;
        }

        self.show_selection_text(cx, &request);

        // Reset the UI state
        self.reset();
        self.should_select_new_messages = true;
        self.set_confirmation_visible(cx, false);
        self.view.view(cx, ids!(loading_view)).set_visible(cx, true);
        self.view.label(cx, ids!(error_label)).set_visible(cx, false);
        self.view.widget(cx, ids!(progress_bar)).set_visible(cx, false);
        self.view.progress_bar(cx, ids!(progress_bar)).set_progress(cx, 0.0);
        self.view.text_input(cx, ids!(reason_input)).set_text(cx, request.initial_reason.as_deref().unwrap_or_default());
        let messages_list = self.view.portal_list(cx, ids!(messages_list));
        messages_list.stop_scrolling();
        messages_list.set_first_id_and_scroll(0, 0.0);
        self.view.button(cx, ids!(cancel_button)).set_text(cx, "Cancel");
        self.view.button(cx, ids!(stop_button)).set_visible(cx, false);
        self.view.button(cx, ids!(okay_button)).set_visible(cx, false);
        self.view.button(cx, ids!(select_all_button)).reset_hover(cx);
        self.view.button(cx, ids!(select_none_button)).reset_hover(cx);
        self.update_remove_button(cx);
        self.redraw(cx);

        let (stop_fetching, stop) = oneshot::channel();
        self.stop_fetching = Some(stop_fetching);
        self.fetch_id = self.fetch_id.wrapping_add(1);
        submit_async_request(MatrixRequest::GetUserMessages {
            room_id: request.room_id.clone(),
            user_id: request.user_id.clone(),
            fetch_id: self.fetch_id,
            stop,
        });
        self.state = RemoveMessagesModalState::WaitingForMessages;
        self.request = Some(request);
    }

    fn handle_update(&mut self, cx: &mut Cx, update: &RemoveMessagesUpdate) {
        match update {
            RemoveMessagesUpdate::MessagesFetched { fetch_id, messages, is_finished, error } => {
                if *fetch_id != self.fetch_id || self.state != RemoveMessagesModalState::WaitingForMessages {
                    return;
                }
                let num_old_messages = self.messages.len();
                self.messages.extend_from_slice(messages);
                if self.should_select_new_messages {
                    self.selected_messages.extend(num_old_messages..self.messages.len());
                }
                self.update_remove_button(cx);
                if *is_finished {
                    self.finish_fetching(cx, error.as_deref());
                }
                self.redraw(cx);
            }
            RemoveMessagesUpdate::RemovalProgressed { num_removed, num_failed } => {
                self.show_progress(cx, *num_removed, *num_failed);
            }
            RemoveMessagesUpdate::RemovalFinished { num_removed, num_failed, first_error, was_stopped } => {
                self.stop_removing = None;
                let Some(request) = self.request.as_ref() else { return };
                let user_name = &request.user_name;
                let total = format_message_count(self.num_messages_to_remove);
                let (title, description, popup_kind) = if *was_stopped {
                    (
                        "Stopped removing messages",
                        format!("Removed {num_removed} of {total} from {user_name} before stopping."),
                        PopupKind::Warning,
                    )
                } else if *num_failed > 0 {
                    (
                        "Some messages weren't removed",
                        format!(
                            "Removed {num_removed} of {total} from {user_name}.\n\nError: {}",
                            first_error.as_deref().unwrap_or("unknown"),
                        ),
                        PopupKind::Error,
                    )
                } else {
                    ("Messages removed", format!("Removed {total} from {user_name}."), PopupKind::Success)
                };
                if self.is_hidden {
                    enqueue_popup_notification(description, popup_kind, Some(RESULT_POPUP_DURATION));
                    self.reset();
                } else {
                    self.show_final_state(cx, title, &description);
                }
            }
        }
    }
}

impl RemoveMessagesModalRef {
    /// See [`RemoveMessagesModal::show()`].
    pub fn show(&self, cx: &mut Cx, request: RemoveMessagesRequest) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, request);
    }

    /// Applies the given update, even while this modal is hidden.
    pub fn handle_update(&self, cx: &mut Cx, update: &RemoveMessagesUpdate) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.handle_update(cx, update);
    }

    /// Stops anything in progress and forgets about it, e.g., upon logging out.
    pub fn reset(&self) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.stop_removing = None;
        inner.reset();
    }
}

fn format_message_count(count: usize) -> String {
    if count == 1 { String::from("1 message") } else { format!("{count} messages") }
}

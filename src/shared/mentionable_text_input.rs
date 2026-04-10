//! A temporary mock/placeholder for MentionableTextInput that uses a simple TextInput
//! instead of the full @mention popup system (CommandTextInput).
//!
//! This preserves the same external-facing API so that the real MentionableTextInput
//! can be slotted back in later without changing the code that depends on it.

use std::collections::{BTreeMap, BTreeSet};
use makepad_widgets::*;
use matrix_sdk::ruma::{
    events::{room::message::RoomMessageEventContent, Mentions},
    OwnedMxcUri, OwnedRoomId, OwnedUserId,
};
use crate::LivePtr;
use crate::shared::command_text_input::CommandTextInput;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    // Template for user list items in the mention dropdown
    mod.widgets.UserListItem = RoundedView {
        width: Fill,
        height: Fit,
        margin: Inset{left: 4, right: 4}
        padding: Inset{left: 8, right: 8, top: 4, bottom: 4}
        show_bg: true
        cursor: MouseCursor.Hand
        draw_bg +: {
            color: instance(#ffffff),
            border_radius: uniform(4.0),
            hover: instance(0.0),
            selected: instance(0.0),

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size);
                sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                // Light blue hover color (#DBEAFE)
                let hover_color = vec4(0.859, 0.918, 0.996, 1.0);

                if self.selected > 0.0 {
                    sdf.fill(hover_color)
                } else if self.hover > 0.0 {
                    sdf.fill(hover_color)
                } else {
                    sdf.fill(self.color)
                }
                return sdf.result
            }
        }
        animator: Animator {
            hover: {
                default: @off
                off: AnimatorState{ from: {all: Forward{duration: 0.1}} apply: { draw_bg: { hover: 0.0 }}}
                on: AnimatorState{ from: {all: Forward{duration: 0.1}} apply: { draw_bg: { hover: 1.0 }}}
            }
            selected: {
                default: @off
                off: AnimatorState{ from: {all: Forward{duration: 0.1}} apply: { draw_bg: { selected: 0.0 }}}
                on: AnimatorState{ from: {all: Forward{duration: 0.1}} apply: { draw_bg: { selected: 1.0 }}}
            }
        }
        flow: Down
        spacing: 2.0

        user_info := View {
            width: Fill,
            height: Fit,
            flow: Right,
            spacing: 8.0
            align: Align{y: 0.5}

            avatar := Avatar {
                width: 24,
                height: 24,
                text_view +: {
                    text +: {
                        draw_text +: {
                            text_style: theme.font_regular { font_size: 12.0 }
                        }
                    }
                }
            }

            username := Label {
                height: Fit,
                draw_text +: {
                    color: #000,
                    text_style: theme.font_regular {font_size: 14.0}
                }
            }

            filler := FillerX {}
        }

        user_id := Label {
            height: Fit,
            draw_text +: {
                color: #666,
                text_style: theme.font_regular {font_size: 12.0}
            }
        }
    }

    // Template for the @room mention list item
    mod.widgets.RoomMentionListItem = RoundedView {
        width: Fill,
        height: Fit,
        margin: Inset{left: 4, right: 4}
        padding: Inset{left: 8, right: 8, top: 4, bottom: 4}
        show_bg: true
        cursor: MouseCursor.Hand
        draw_bg +: {
            color: instance(#ffffff),
            border_radius: uniform(4.0),
            hover: instance(0.0),
            selected: instance(0.0),

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size);
                sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                // Light blue hover color (#DBEAFE)
                let hover_color = vec4(0.859, 0.918, 0.996, 1.0);

                if self.selected > 0.0 {
                    sdf.fill(hover_color)
                } else if self.hover > 0.0 {
                    sdf.fill(hover_color)
                } else {
                    sdf.fill(self.color)
                }
                return sdf.result
            }
        }
        animator: Animator {
            hover: {
                default: @off
                off: AnimatorState{ from: {all: Forward{duration: 0.1}} apply: { draw_bg: { hover: 0.0 }}}
                on: AnimatorState{ from: {all: Forward{duration: 0.1}} apply: { draw_bg: { hover: 1.0 }}}
            }
            selected: {
                default: @off
                off: AnimatorState{ from: {all: Forward{duration: 0.1}} apply: { draw_bg: { selected: 0.0 }}}
                on: AnimatorState{ from: {all: Forward{duration: 0.1}} apply: { draw_bg: { selected: 1.0 }}}
            }
        }
        flow: Down
        spacing: 2.0
        align: Align{y: 0.5}

        user_info := View {
            width: Fill,
            height: Fit,
            flow: Right,
            spacing: 8.0
            align: Align{y: 0.5}

            room_avatar := Avatar {
                width: 24,
                height: 24,
                text_view +: {
                    text +: {
                        draw_text +: {
                            text_style: theme.font_regular { font_size: 12.0 }
                        }
                    }
                }
            }

            room_mention := Label {
                height: Fit,
                draw_text +: {
                    color: #000,
                    text_style: theme.font_regular {font_size: 14.0}
                }
                text: "Notify the entire room"
            }

            filler := FillerX {}
        }

        room_user_id := Label {
            height: Fit,
            align: Align{y: 0.5},
            draw_text +: {
                color: #666,
                text_style: theme.font_regular {font_size: 12.0}
            }
            text: "@room"
        }
    }

    // Template for loading indicator when members are being fetched
    mod.widgets.LoadingIndicator = SolidView {
        width: Fill,
        height: 48,
        margin: Inset{left: 4, right: 4}
        padding: Inset{left: 8, right: 8, top: 8, bottom: 8},
        flow: Right,
        spacing: 8.0,
        align: Align{x: 0.0, y: 0.5}
        draw_bg.color: (COLOR_PRIMARY),

        loading_text := Label {
            height: Fit,
            draw_text +: {
                color: #666,
                text_style: theme.font_regular {font_size: 14.0}
            }
            text: "Loading members"
        }

        loading_animation := BouncingDots {
            width: 60,
            height: 24,
            draw_bg +: {
                color: (COLOR_ROBRIX_PURPLE),
                dot_radius: 2.0,
            }
        }
    }

    // Template for no matches indicator when no users match the search
    mod.widgets.NoMatchesIndicator = SolidView {
        width: Fill,
        height: 48,
        margin: Inset{left: 4, right: 4}
        padding: Inset{left: 8, right: 8, top: 8, bottom: 8},
        flow: Right,
        spacing: 8.0,
        align: Align{x: 0.0, y: 0.5}
        draw_bg.color: (COLOR_PRIMARY)

        no_matches_text := Label {
            height: Fit,
            draw_text +: {
                color: #666,
                text_style: theme.font_regular {font_size: 14.0}
            }
            text: "No matching users found"
        }
    }

    // Template for user mention pill shown in the input area
    mod.widgets.UserPill = RoundedView {
        width: Fit,
        height: Fit,
        margin: Inset{left: 2, right: 2, top: 2, bottom: 2}
        padding: Inset{left: 4, right: 2, top: 2, bottom: 2}
        show_bg: true
        draw_bg +: {
            color: instance(#E8F4FD),
            border_radius: uniform(12.0),
        }
        flow: Right,
        spacing: 4.0,
        align: Align{y: 0.5}

        pill_avatar := Avatar {
            width: 18,
            height: 18,
            text_view +: {
                text +: {
                    draw_text +: {
                        text_style: theme.font_regular { font_size: 9.0 }
                    }
                }
            }
        }

        pill_username := Label {
            height: Fit,
            draw_text +: {
                color: #1976D2,
                text_style: theme.font_regular {font_size: 12.0}
            }
        }

        close_button := RoundedView {
            width: 16,
            height: 16,
            show_bg: true
            cursor: MouseCursor.Hand
            draw_bg +: {
                color: instance(#00000000),
                border_radius: uniform(8.0),
                hover: instance(0.0),

                pixel: fn() {
                    let sdf = Sdf2d.viewport(self.pos * self.rect_size);
                    sdf.circle(self.rect_size.x * 0.5, self.rect_size.y * 0.5, self.rect_size.x * 0.5);
                    // Light red hover color
                    let hover_color = vec4(0.95, 0.85, 0.85, 1.0);
                    if self.hover > 0.0 {
                        sdf.fill(hover_color)
                    } else {
                        sdf.fill(self.color)
                    }
                    return sdf.result
                }
            }
            animator: Animator {
                hover: {
                    default: @off
                    off: AnimatorState{ from: {all: Forward{duration: 0.1}} apply: { draw_bg: { hover: 0.0 }}}
                    on: AnimatorState{ from: {all: Forward{duration: 0.1}} apply: { draw_bg: { hover: 1.0 }}}
                }
            }
            align: Align{x: 0.5, y: 0.5}

            close_icon := Label {
                width: Fit,
                height: Fit,
                draw_text +: {
                    color: #666,
                    text_style: theme.font_regular {font_size: 10.0}
                }
                text: "×"
            }
        }
    }

    // Step 1: Register the base widget type
    mod.widgets.MentionableTextInputBase = #(MentionableTextInput::register_widget(vm))

    // Step 2: Define the full widget inheriting from CommandTextInput's DSL structure
    mod.widgets.MentionableTextInput = mod.widgets.MentionableTextInputBase {
        //..mod.widgets.CommandTextInput
        flow: Down
        height: Fit
        trigger: "@"
        inline_search: true

        color_focus: (mod.widgets.FOCUS_HOVER_COLOR),
        color_hover: (mod.widgets.FOCUS_HOVER_COLOR),

        popup := RoundedView {
            spacing: 0.0
            padding: 0.0

            draw_bg.color: (COLOR_SECONDARY)

            header_view := SolidView {
                margin: Inset{left: 4, right: 4}
                draw_bg.color: (COLOR_ROBRIX_PURPLE)
                header_label := Label {
                    draw_text.color: (COLOR_PRIMARY_DARKER),
                    text: "Users in this Room"
                }
            }

            list := mod.widgets.CommandTextInputList {
                height: Fit
                clip_y: true
                spacing: 0.0
                padding: 0.0
            }
        }

        persistent := RoundedView {
            width: Fill,
            height: Fit,
            flow: Down,
            top := View { height: 0 }
            center := RoundedView {
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                pills_container := View {
                    width: Fit
                    height: Fit
                    flow: Right
                    spacing: 2.0
                    align: Align{y: 0.5}

                    // Pre-defined pill slots (max 5 pills)
                    pill_0 := mod.widgets.UserPill { visible: false }
                    pill_1 := mod.widgets.UserPill { visible: false }
                    pill_2 := mod.widgets.UserPill { visible: false }
                    pill_3 := mod.widgets.UserPill { visible: false }
                    pill_4 := mod.widgets.UserPill { visible: false }
                }
                left := View{ width: Fit, height: Fit }
                right := View{ width: Fit, height: Fit }
                width: Fill,
                height: Fit,
                text_input := RobrixTextInput {
                    width: Fill
                    empty_text: "Start typing..."
                }
            }
        }

        // Template for user list items in the mention popup
        user_list_item: mod.widgets.UserListItem {}
        room_mention_list_item: mod.widgets.RoomMentionListItem {}
        loading_indicator: mod.widgets.LoadingIndicator {}
        no_matches_indicator: mod.widgets.NoMatchesIndicator {}
        user_pill: mod.widgets.UserPill {}
    }
}

// /// A special string used to denote the start of a mention within
// /// the actual text being edited.
// /// This is used to help easily locate and distinguish actual mentions
// /// from normal `@` characters.
// const MENTION_START_STRING: &str = "\u{8288}@\u{8288}";


#[derive(Debug)]
pub enum MentionableTextInputAction {
    /// Notifies the MentionableTextInput about updated power levels for the room.
    PowerLevelsUpdated {
        room_id: OwnedRoomId,
        can_notify_room: bool,
    }
}

/// Data for a selected user pill
#[derive(Clone, Debug)]
pub struct SelectedPill {
    pub user_id: OwnedUserId,
    pub display_name: String,
    pub avatar_url: Option<OwnedMxcUri>,
}

/// Temporary mock widget that wraps a simple TextInput (RobrixTextInput)
/// while preserving the same external API as the real MentionableTextInput.
#[derive(Script, ScriptHook, Widget)]
pub struct MentionableTextInput {
    #[source] source: ScriptObjectRef,
    /// Base command text input
    #[deref] cmd_text_input: CommandTextInput,
    /// Template for user list items
    #[live] user_list_item: Option<LivePtr>,
    /// Template for the @room mention list item
    #[live] room_mention_list_item: Option<LivePtr>,
    /// Template for loading indicator
    #[live] loading_indicator: Option<LivePtr>,
    /// Template for no matches indicator
    #[live] no_matches_indicator: Option<LivePtr>,
    /// Template for user pill
    #[live] user_pill: Option<LivePtr>,
    /// Position where the @ mention starts
    #[rust] current_mention_start_index: Option<usize>,
    /// The set of users that were mentioned (at one point) in this text input.
    /// Due to characters being deleted/removed, this list is a *superset*
    /// of possible users who may have been mentioned.
    /// All of these mentions may not exist in the final text input content;
    /// this is just a list of users to search the final sent message for
    /// when adding in new mentions.
    #[rust] possible_mentions: BTreeMap<OwnedUserId, String>,
    /// Indicates if the `@room` option was explicitly selected.
    #[rust] possible_room_mention: bool,
    /// Indicates if currently in mention search mode
    #[rust] is_searching: bool,
    /// Whether the current user can notify everyone in the room (@room mention)
    #[deref] view: View,
    /// Whether the current user can notify everyone in the room (@room mention).
    /// Stored but not used in this mock; kept for API compatibility.
    #[rust] can_notify_room: bool,
    /// Whether the room members are currently being loaded
    #[rust] members_loading: bool,
    /// Selected user pills to display in the input
    #[rust] selected_pills: Vec<SelectedPill>,
    /// View references for rendered pills (to handle close button events)
    #[rust] pill_widgets: Vec<ViewRef>,
}

impl Widget for MentionableTextInput {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.cmd_text_input.handle_event(cx, event, scope);

        // Handle pill close button clicks
        self.handle_pill_events(cx, event);

        // Handle MentionableTextInputAction for API compatibility.
        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(MentionableTextInputAction::PowerLevelsUpdated {
                    can_notify_room, ..
                }) = action.downcast_ref()
                {
                    self.can_notify_room = *can_notify_room;
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }

    fn text(&self) -> String {
        self.child_by_path(ids!(text_input)).as_text_input().text()
    }

    fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.text_input(cx, ids!(persistent.center.text_input)).set_text(cx, text);
        self.redraw(cx);
    }

    fn set_key_focus(&self, cx: &mut Cx) {
        self.text_input(cx, ids!(persistent.center.text_input)).set_key_focus(cx);
    }
}

impl MentionableTextInput {

    /// Sets whether the current user can notify the entire room (@room mention).
    pub fn set_can_notify_room(&mut self, can_notify: bool) {
        self.can_notify_room = can_notify;
    }

    /// Gets whether the current user can notify the entire room (@room mention).
    pub fn can_notify_room(&self) -> bool {
        self.can_notify_room
    }

    /// Handle pill close button click events.
    fn handle_pill_events(&mut self, _cx: &mut Cx, _event: &Event) {
        // TODO: Implement pill close button event handling
    }

    /// Re-render the pill widgets.
    fn render_pills(&mut self, _cx: &mut Cx) {
        // TODO: Implement pill rendering
    }
}

impl MentionableTextInputRef {
    /// Returns a reference to the inner `TextInput` widget.
    pub fn text_input_ref(&self) -> TextInputRef {
        self.child_by_path(ids!(persistent.center.text_input)).as_text_input()
    }

    /// Sets whether the current user can notify the entire room (@room mention).
    pub fn set_can_notify_room(&self, can_notify: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_can_notify_room(can_notify);
        }
    }

    /// Gets whether the current user can notify the entire room (@room mention).
    pub fn can_notify_room(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.can_notify_room())
    }

    /// Returns the mentions from selected pills plus any @room mention in the text.
    fn get_mentions_from_pills_and_text(&self, text: &str) -> Mentions {
        let mut mentions = Mentions::new();

        let Some(inner) = self.borrow() else {
            return mentions;
        };

        // Get user IDs from selected pills
        let user_ids: BTreeSet<OwnedUserId> = inner.selected_pills
            .iter()
            .map(|pill| pill.user_id.clone())
            .collect();

        mentions.user_ids = user_ids;
        // Check for @room mention in text content
        mentions.room = inner.possible_room_mention && text.contains("@room");
        mentions
    }

    /// Builds the message text with pill mentions converted to markdown links.
    fn build_text_with_pill_mentions(&self, entered_text: &str) -> String {
        let Some(inner) = self.borrow() else {
            return entered_text.to_string();
        };

        if inner.selected_pills.is_empty() {
            return entered_text.to_string();
        }

        // Build mention prefix from pills
        let mention_prefix: String = inner.selected_pills
            .iter()
            .map(|pill| format!("[{}]({}) ", pill.display_name, pill.user_id.matrix_to_uri()))
            .collect();

        // Prepend pill mentions to the entered text
        format!("{}{}", mention_prefix, entered_text)
    }

    /// Creates a message from the entered text.
    ///
    /// This mock version handles `/html` and `/plain` prefixes
    /// but does not track or extract @mentions (since the mention popup is disabled).
    pub fn create_message_with_mentions(&self, entered_text: &str) -> RoomMessageEventContent {
        if let Some(html_text) = entered_text.strip_prefix("/html") {
            let full_text = self.build_text_with_pill_mentions(html_text);
            let message = RoomMessageEventContent::text_html(&full_text, &full_text);
            message.add_mentions(self.get_mentions_from_pills_and_text(&full_text))
        } else if let Some(plain_text) = entered_text.strip_prefix("/plain") {
            RoomMessageEventContent::text_plain(plain_text)
        } else {
            let full_text = self.build_text_with_pill_mentions(entered_text);
            let message = RoomMessageEventContent::text_markdown(&full_text);
            message.add_mentions(self.get_mentions_from_pills_and_text(&full_text))
        }
    }

    /// Clears all selected pills
    pub fn clear_pills(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.selected_pills.clear();
            inner.pill_widgets.clear();
            inner.possible_mentions.clear();
            inner.possible_room_mention = false;
            inner.render_pills(cx);
        }
    }

    /// Returns true if there are any selected pills
    pub fn has_pills(&self) -> bool {
        self.borrow().is_some_and(|inner| !inner.selected_pills.is_empty())
    }
}

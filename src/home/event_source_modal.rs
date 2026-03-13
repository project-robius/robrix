//! A modal dialog that displays the raw JSON source of a Matrix event.

use makepad_code_editor::code_view::CodeViewWidgetExt;
use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedEventId, OwnedRoomId};

use crate::shared::popup_list::{PopupKind, enqueue_popup_notification};


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.VIEW_SOURCE_MODAL_BORDER_RADIUS = 6.0

    // A small icon button for copying content
    mod.widgets.CopyButton = RobrixIconButton {
        width: Fit, height: Fit,
        padding: 8,
        spacing: 0
        align: Align{x: 0.5, y: 0.5}
        icon_walk: Walk{width: 14, height: 14, margin: 0}
        draw_icon +: {
            svg: (ICON_COPY),
            color: #666
        }
        draw_bg +: {
            border_size: 0,
            color: #0000
        }
    }

    mod.widgets.EventSourceModal = set_type_default() do #(EventSourceModal::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill { max: 1000 } //, min: 600 }
        // TODO: i'd like for this height to be Fit with a max of Rel { base: Full, factor: 0.90 },
        //       but Makepad doesn't allow Fit views with a max to be scrolled.
        height: Fill // { max: 1400 }
        margin: 40,
        align: Align{x: 0.5, y: 0}
        flow: Down
        padding: Inset{top: 20, right: 25, bottom: 20, left: 25}

        // Make this a ScrollYView
        scroll_bars: ScrollBars {
            show_scroll_x: false, show_scroll_y: true,
            scroll_bar_y: ScrollBar {drag_scrolling: true}
        }

        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: mod.widgets.VIEW_SOURCE_MODAL_BORDER_RADIUS
            border_size: 0.0
        }

        // Title and close button (outside scroll so it stays visible)
        title_view := View {
            width: Fill, height: Fit,
            flow: Right,
            align: Align{y: 0.5}

            title := Label {
                width: Fill, height: Fit,
                draw_text +: {
                    text_style: TITLE_TEXT {font_size: 16},
                    color: #000
                }
                text: "View Event Source"
            }

            close_button := RobrixIconButton {
                width: Fit, height: Fit,
                padding: 12,
                spacing: 0
                align: Align{x: 0.5, y: 0.5}
                icon_walk: Walk{width: 18, height: 18, margin: 0}
                draw_icon +: {
                    svg: (ICON_CLOSE),
                    color: #666
                }
                draw_bg +: {
                    border_size: 0
                    color: #0000
                }
            }
        }

        // Room ID row
        room_id_row := View {
            width: Fill, height: Fit,
            flow: Flow.Right {wrap: true}
            align: Align{y: 0.5}
            margin: Inset{top: -1, bottom: -1}
            padding: 0

            Label {
                width: Fit, height: Fit,
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 11},
                    color: #666
                }
                text: "Room ID:"
            }
            room_id_value := Label {
                width: Fit, height: Fit,
                // the top margin is a hack to fix vertical alignment
                margin: Inset{top: 1, left: 4}
                draw_text +: {
                    text_style: theme.font_code {font_size: 10},
                    color: #000
                }
                text: "<Unknown Room ID>"
            }
            room_id_copy_button := mod.widgets.CopyButton {
                margin: Inset{left: 4}
            }
        }

        // Event ID row
        event_id_row := View {
            width: Fill, height: Fit,
            flow: Flow.Right {wrap: true}
            align: Align{y: 0.5}
            margin: Inset{top: -1, bottom: -1}
            padding: 0

            Label {
                width: Fit, height: Fit,
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 11},
                    color: #666
                }
                text: "Event ID:"
            }
            event_id_value := Label {
                width: Fit, height: Fit,
                // the top margin is a hack to fix vertical alignment
                margin: Inset{top: 1, left: 4}
                draw_text +: {
                    text_style: theme.font_code {font_size: 10},
                    color: #000
                }
                text: "<Unknown Event ID>"
            }
            event_id_copy_button := mod.widgets.CopyButton {
                margin: Inset{left: 4}
            }
        }

        LineH {
            height: 1
            margin: 3
        }

        // Original event source section header
        source_header := View {
            width: Fill, height: Fit,
            flow: Right,
            align: Align{y: 0.5}
            padding: Inset{top: 3, left: 3, right: 6}

            source_label := Label {
                width: Fill, height: Fit,
                draw_text +: {
                    text_style: TITLE_TEXT {font_size: 13},
                    color: #000
                }
                text: "Original event source"
            }

            copy_source_button := mod.widgets.CopyButton {}
        }

        // An overlay view that draws a border frame around the code view.
        code_block := View {
            width: Fill,
            height: Fit,
            flow: Overlay 
            // align the left side of the border frame with the left side of the room id / event id rows
            padding: 6

            // The code editor content (drawn first, behind the overlay)
            code_view := mod.widgets.CodeView {
                editor +: {
                    margin: 12,
                    width: Fill,
                    height: Fit,
                    word_wrap: true
                    draw_bg +: { color: (COLOR_TRANSPARENT) }
                    draw_text +: { text_style +: { font_size: 11 } }

                    // Light mode syntax highlighting (inspired by GitHub Light / VS Code Light+)
                    token_colors +: {
                        whitespace: #x6a737d,         // Gray for whitespace markers
                        delimiter: #x24292e,          // Dark gray for punctuation
                        delimiter_highlight: #x005cc5, // Blue for highlighted delimiters
                        error_decoration: #xcb2431,   // Red for errors
                        warning_decoration: #xb08800, // Dark yellow/amber for warnings

                        unknown: #x24292e,            // Default dark text
                        branch_keyword: #xd73a49,     // Red/pink for keywords (if, else, match)
                        constant: #x005cc5,           // Blue for constants
                        identifier: #x24292e,         // Dark gray for variables
                        loop_keyword: #xd73a49,       // Red/pink for loop keywords
                        number: #x005cc5,             // Blue for numbers
                        other_keyword: #xd73a49,      // Red/pink for other keywords
                        punctuator: #x24292e,         // Dark gray for punctuation
                        string: #x22863a,             // Green for strings
                        function: #x6f42c1,           // Purple for functions
                        typename: #xe36209,           // Orange for types
                        comment: #x6a737d,            // Gray for comments
                    }
                }
                text: "<Unknown Event Source>"
            }

            // Border overlay frame (drawn on top of content)
            // Only draws the stroke, fill is transparent
            border_frame := RoundedView {
                width: Fill,
                height: Fill,
                show_bg: true
                draw_bg +: {
                    color: (COLOR_TRANSPARENT)
                    border_radius: mod.widgets.VIEW_SOURCE_MODAL_BORDER_RADIUS,
                    border_size: 1.25,
                    border_color: (COLOR_SECONDARY)
                }
            }
        }

        // just some extra space at the bottom
        View {
            width: Fill, height: 25
        }
    }
}

/// Actions emitted by this modal to request showing or closing it.
#[derive(Clone, Debug)]
pub enum EventSourceModalAction {
    /// Open the modal with the given event details and JSON source.
    Open {
        room_id: OwnedRoomId,
        event_id: Option<OwnedEventId>,
        original_json: Option<String>,
    },
    /// Close the modal.
    Close,
}


#[derive(Script, ScriptHook, Widget)]
pub struct EventSourceModal {
    #[deref] view: View,
    #[rust] room_id: Option<OwnedRoomId>,
    #[rust] event_id: Option<OwnedEventId>,
    #[rust] original_json: Option<String>,
}

impl Widget for EventSourceModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if let Some(room_id) = &self.room_id {
            self.view.label(cx, ids!(room_id_value)).set_text(cx, room_id.as_str());
        }
        if let Some(event_id) = &self.event_id {
            self.view.label(cx, ids!(event_id_value)).set_text(cx, event_id.as_str());
        }
        if let Some(json) = &self.original_json {
            self.view.code_view(cx, ids!(code_view)).set_text(cx, json);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for EventSourceModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let close_button = self.view.button(cx, ids!(close_button));

        // Handle canceling/closing the modal.
        let close_clicked = close_button.clicked(actions);
        if close_clicked ||
            actions.iter().any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            // If the modal was dismissed by clicking outside of it, we MUST NOT emit
            // an EventSourceModalAction::Close action, as that would cause
            // an infinite action feedback loop.
            if close_clicked {
                cx.action(EventSourceModalAction::Close);
            }
            return;
        }

        if self.view.button(cx, ids!(room_id_copy_button)).clicked(actions) {
            if let Some(room_id) = &self.room_id {
                cx.copy_to_clipboard(room_id.as_str());
                enqueue_popup_notification(
                    "Copied Room ID to clipboard.",
                    PopupKind::Success,
                    Some(3.0),
                );
            }
        }

        if self.view.button(cx, ids!(event_id_copy_button)).clicked(actions) {
            if let Some(event_id) = &self.event_id {
                cx.copy_to_clipboard(event_id.as_str());
                enqueue_popup_notification(
                    "Copied Event ID to clipboard.",
                    PopupKind::Success,
                    Some(3.0),
                );
            }
        }

        if self.view.button(cx, ids!(copy_source_button)).clicked(actions) {
            if let Some(json) = &self.original_json {
                cx.copy_to_clipboard(json);
                enqueue_popup_notification(
                    "Copied event source to clipboard.",
                    PopupKind::Success,
                    Some(3.0),
                );
            }
        }
    }
}

impl EventSourceModal {
    /// Shows the modal with the given event details and JSON source.
    pub fn show(
        &mut self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
        event_id: Option<OwnedEventId>,
        original_json: Option<String>,
    ) {
        self.room_id = Some(room_id.clone());
        self.event_id = event_id.clone();
        self.original_json = original_json.clone();

        self.view.button(cx, ids!(close_button)).reset_hover(cx);
        self.view.button(cx, ids!(room_id_copy_button)).reset_hover(cx);
        self.view.button(cx, ids!(event_id_copy_button)).reset_hover(cx);
        self.view.button(cx, ids!(copy_source_button)).reset_hover(cx);
        self.view.redraw(cx);
    }
}

impl EventSourceModalRef {
    /// Shows the modal with the given event details and JSON source.
    pub fn show(
        &self,
        cx: &mut Cx,
        room_id: OwnedRoomId,
        event_id: Option<OwnedEventId>,
        original_json: Option<String>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx, room_id, event_id, original_json);
    }
}

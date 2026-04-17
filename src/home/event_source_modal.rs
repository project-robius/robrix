//! A modal dialog that displays the raw JSON source of a Matrix event.

use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedEventId, OwnedRoomId};

use crate::shared::popup_list::{PopupKind, enqueue_popup_notification};


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.VIEW_SOURCE_MODAL_BORDER_RADIUS = 6.0

    mod.widgets.EventSourceHtml = mod.widgets.MessageHtml {
        width: Fill
        height: Fit
        padding: 0.0
        font_size: 11.0
        font_color: #x24292e

        draw_text +: {
            color: #x24292e
        }
        text_style_normal: mod.widgets.EVENT_SOURCE_CODE_TEXT_STYLE { }
        text_style_italic: mod.widgets.EVENT_SOURCE_CODE_TEXT_STYLE { }
        text_style_bold: mod.widgets.EVENT_SOURCE_CODE_TEXT_STYLE { }
        text_style_bold_italic: mod.widgets.EVENT_SOURCE_CODE_TEXT_STYLE { }
        text_style_fixed: mod.widgets.EVENT_SOURCE_CODE_TEXT_STYLE { }
        draw_block +: {
            line_color: #x24292e
            sep_color: (COLOR_SECONDARY)
            quote_bg_color: #xF7F9FC
            quote_fg_color: #x24292e
            code_color: #xF7F9FC
        }
        code_layout: Layout{
            flow: Flow.Right{wrap: true}
            padding: Inset{ left: 14.0, right: 14.0, top: 12.0, bottom: 12.0 }
        }
        code_walk: Walk{ width: Fill, height: Fit }

        body: "<pre>{}</pre>"
    }

    // A small icon button for copying content
    mod.widgets.CopyButton = RobrixIconButton {
        width: Fit, height: Fit,
        padding: 8,
        spacing: 0
        align: Align{x: 0.5, y: 0.5}
        icon_walk: Walk{width: 14, height: 14, margin: 0}
        draw_icon.svg: (ICON_COPY)
        draw_icon.color: #666
        draw_bg +: {
            border_size: 0,
            color: #0000
            color_hover: #00000015
            color_down: #00000025
        }
    }

    mod.widgets.EventSourceModal = set_type_default() do #(EventSourceModal::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill { max: 1000 }
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
                draw_icon.svg: (ICON_CLOSE)
                draw_icon.color: #666
                draw_bg +: {
                    border_size: 0
                    color: #0000
                    color_hover: #00000015
                    color_down: #00000025
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

        // An overlay view that draws a border frame around the source view.
        code_block := View {
            width: Fill,
            height: Fit,
            flow: Overlay 
            // align the left side of the border frame with the left side of the room id / event id rows
            padding: 6

            source_html := mod.widgets.EventSourceHtml {
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
            self.view.html(cx, ids!(source_html))
                .set_text(cx, &format_event_source_html(json));
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

fn format_event_source_html(json: &str) -> String {
    const COLOR_PUNCTUATION: &str = "#6A737D";
    const COLOR_KEY: &str = "#22863A";
    const COLOR_STRING: &str = "#032F62";
    const COLOR_NUMBER: &str = "#005CC5";
    const COLOR_LITERAL: &str = "#D73A49";

    let chars: Vec<char> = json.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    let mut at_line_start = true;

    while i < chars.len() {
        match chars[i] {
            '"' => {
                let start = i;
                i += 1;
                let mut escaped = false;
                while i < chars.len() {
                    let ch = chars[i];
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                let token: String = chars[start..i.min(chars.len())].iter().collect();
                let mut j = i;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                let color = if j < chars.len() && chars[j] == ':' {
                    COLOR_KEY
                } else {
                    COLOR_STRING
                };
                push_colored_html(&mut out, color, &token);
                at_line_start = false;
            }
            '-' | '0'..='9' => {
                let start = i;
                i += 1;
                while i < chars.len() && matches!(chars[i], '0'..='9' | '.' | 'e' | 'E' | '+' | '-') {
                    i += 1;
                }
                let token: String = chars[start..i].iter().collect();
                push_colored_html(&mut out, COLOR_NUMBER, &token);
                at_line_start = false;
            }
            't' if starts_with_chars(&chars, i, "true") => {
                push_colored_html(&mut out, COLOR_LITERAL, "true");
                i += 4;
                at_line_start = false;
            }
            'f' if starts_with_chars(&chars, i, "false") => {
                push_colored_html(&mut out, COLOR_LITERAL, "false");
                i += 5;
                at_line_start = false;
            }
            'n' if starts_with_chars(&chars, i, "null") => {
                push_colored_html(&mut out, COLOR_LITERAL, "null");
                i += 4;
                at_line_start = false;
            }
            '{' | '}' | '[' | ']' | ':' | ',' => {
                push_colored_html(&mut out, COLOR_PUNCTUATION, &chars[i].to_string());
                i += 1;
                at_line_start = false;
            }
            '\n' => {
                out.push_str("<br>");
                i += 1;
                at_line_start = true;
            }
            ' ' => {
                if at_line_start {
                    let mut leading_spaces = 0usize;
                    while i < chars.len() && chars[i] == ' ' {
                        leading_spaces += 1;
                        i += 1;
                    }
                    push_indent_html(&mut out, leading_spaces * 2);
                } else {
                    out.push_str("&nbsp;");
                    i += 1;
                }
            }
            '\t' => {
                if at_line_start {
                    push_indent_html(&mut out, 8);
                } else {
                    out.push_str("&nbsp;&nbsp;&nbsp;&nbsp;");
                }
                i += 1;
            }
            ch if ch.is_whitespace() => {
                out.push_str(&htmlize::escape_text(ch.to_string()));
                i += 1;
            }
            ch => {
                out.push_str(&htmlize::escape_text(ch.to_string()));
                i += 1;
                at_line_start = false;
            }
        }
    }
    out
}

fn push_colored_html(out: &mut String, color: &str, token: &str) {
    out.push_str("<font color=\"");
    out.push_str(color);
    out.push_str("\">");
    out.push_str(&htmlize::escape_text(token));
    out.push_str("</font>");
}

fn push_indent_html(out: &mut String, width: usize) {
    if width == 0 {
        return;
    }
    // Makepad trims leading Unicode whitespace on a fresh line, including NBSP.
    // U+2800 is visually blank but not classified as whitespace, so it survives
    // the trim pass and still reserves indentation width in the HTML flow.
    out.push_str("<font color=\"#F7F9FC\">");
    for _ in 0..width {
        out.push('\u{2800}');
    }
    out.push_str("</font>");
}

fn starts_with_chars(chars: &[char], start: usize, needle: &str) -> bool {
    for (idx, needle_ch) in (start..).zip(needle.chars()) {
        if chars.get(idx).copied() != Some(needle_ch) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::format_event_source_html;

    #[test]
    fn event_source_html_highlights_keys_strings_numbers_and_literals() {
        let json = "{\n  \"body\": \"拿捏中\",\n  \"age\": 3749,\n  \"ok\": true,\n  \"missing\": null\n}";
        let html = format_event_source_html(json);

        assert!(html.contains("<font color=\"#22863A\">\"body\"</font>"));
        assert!(html.contains("<font color=\"#032F62\">\"拿捏中\"</font>"));
        assert!(html.contains("<font color=\"#005CC5\">3749</font>"));
        assert!(html.contains("<font color=\"#D73A49\">true</font>"));
        assert!(html.contains("<font color=\"#D73A49\">null</font>"));
        assert!(html.contains("<br>"));
        assert!(html.contains("<font color=\"#F7F9FC\">"));
        assert!(html.contains('\u{2800}'));
    }

    #[test]
    fn event_source_html_escapes_angle_brackets_inside_strings() {
        let json = "{\n  \"formatted_body\": \"<b>拿捏中</b><br>ok\"\n}";
        let html = format_event_source_html(json);

        assert!(html.contains("&lt;b&gt;拿捏中&lt;/b&gt;&lt;br&gt;ok"));
        assert!(!html.contains("<b>拿捏中</b>"));
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

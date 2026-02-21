//! Widgets that represent a preview of a message that was (or is being) replied to.
//!
//! The core view is private, `ReplyPreviewContent`, which is used by both of the public views
//! exported by this module: `RepliedToMessage` and `ReplyingPreview`.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.ReplyPreviewContent = View {
        width: Fill
        height: Fit
        flow: Down
        padding: Inset{left: 16.0, bottom: 5.0, top: 2.0, right: 11.0}
        cursor: Hand,

        View {
            width: Fill
            height: Fit
            flow: Right
            margin: Inset{ bottom: 10.0, top: 0.0, right: 5.0 }
            align: Align{y: 0.5}

            reply_preview_avatar := Avatar {
                width: 19.,
                height: 19.,
                text_view: { text := Label { draw_text +: {
                    text_style: theme.font_regular { font_size: 6.0 }
                }}}
            }

            reply_preview_username := Label {
                width: Fill,
                flow: Right, // do not wrap
                margin: Inset{ left: 5.0, top: 2 }
                draw_text +: {
                    text_style: USERNAME_TEXT_STYLE { font_size: 10 },
                    color: (USERNAME_TEXT_COLOR)
                    flow: Flow.Right{wrap: true},
                }
                text: "<Username not available>"
            }
        }

        reply_preview_body := HtmlOrPlaintext {
            margin: Inset{left: 1.5}
            html_view: { html := mod.widgets.MessageHtml {
                font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE)
                text_style_normal: theme.font_regular { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) }
                text_style_italic: theme.font_italic { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) }
                text_style_bold: theme.font_bold { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) }
                text_style_bold_italic: theme.font_bold_italic { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) }
                text_style_fixed: theme.font_code { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) }
            } }
            plaintext_view: { pt_label := Label {
                draw_text +: {
                    text_style: MESSAGE_TEXT_STYLE { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) },
                }
            } }
        }
    }

    // A view that shows a preview of the message that the user is currently drafting a reply to,
    // along with a "Replying to" label and a cancel button.
    mod.widgets.ReplyingPreview = View {
        visible: false
        width: Fill
        height: Fit
        flow: Down
        padding: Inset{ left: 9, right: 9 }

        // Displays a "Replying to" label and a cancel button
        // above the preview of the message being replied to.
        View {
            width: Fill
            height: Fit
            flow: Right
            align: Align{y: 0.5}
            padding: Inset{left: 14, right: 6, top: 10, bottom: 0}

            Label {
                width: Fill,
                flow: Right, // do not wrap
                // Vertically align the text with the X icon in the cancel_reply_button
                padding: Inset{top: 5}

                draw_text +: {
                    text_style: USERNAME_TEXT_STYLE {},
                    color: #222,
                    flow: Flow.Right{wrap: true},
                }
                text: "Replying to:"
            }

            cancel_reply_button := RobrixIconButton {
                width: Fit,
                height: Fit,
                padding: 13,
                spacing: 0,
                margin: Inset{left: 5, right: 0},

                draw_bg +: {
                    border_color: (COLOR_FG_DANGER_RED),
                    color: (COLOR_BG_DANGER_RED)
                    border_radius: 5
                }
                draw_icon +: {
                    svg_file: (ICON_CLOSE),
                    color: (COLOR_FG_DANGER_RED)
                }
                icon_walk: Walk{width: 16, height: 16, margin: 0}
            }
        }

        reply_preview_content := mod.widgets.ReplyPreviewContent { }

        LineH {
            margin: Inset{top: 4.0, left: 5, right: 5} //, bottom: 10}
        }
    }

    // A small inline preview of a message that was replied to by another message
    // within a room timeline.
    // That is, this view contains a preview of the earlier message
    // that is shown above the "in-reply-to" message.
    mod.widgets.RepliedToMessage = View {
        visible: false
        width: Fill
        height: Fit
        flow: Down

        padding: Inset{top: 0.0, right: 12.0, bottom: 0.0, left: 12.0}

        // A reply preview with a vertical bar drawn in the background.
        replied_to_message_content := mod.widgets.ReplyPreviewContent {
            show_bg: true
            draw_bg +: {
                vertical_bar_color: instance((USERNAME_TEXT_COLOR))
                vertical_bar_width: instance(2.0)
                border_radius: instance(0.0)

                get_color: fn() -> vec4 {
                    return self.color;
                }

                pixel: fn() -> vec4 {
                    let sdf = Sdf2d.viewport(self.pos * self.rect_size);

                    sdf.box(
                        0.0,
                        0.0,
                        self.rect_size.x,
                        self.rect_size.y,
                        max(1.0, self.border_radius)
                    );
                    sdf.fill(self.get_color());

                    sdf.rect(
                        0.0,
                        0.0,
                        self.vertical_bar_width,
                        self.rect_size.y
                    );
                    sdf.fill(self.vertical_bar_color);

                    return sdf.result;
                }
            }
        }
    }
}

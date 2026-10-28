//! Widgets that represent a preview of a message that was (or is being) replied to.
//!
//! * `ReplyPreviewContent` is the main reply content: the avatar, username, and message body.
//! * `CollapsiblePreview` is a wrapper around that to limits tall reply previews
//!   to a fixed height with a fade-out effect at the bottom.
//! * `RepliedToMessage` are collapsible reply previews in the timeline.
//! * `ReplyingPreview` is a non-collapsible reply preview in the RoomInputBar.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // We render the reply preview content into a CachedView (an off-screen texture)
    // so that we can apply a fade-to-transparent effect towards the bottom.
    mod.widgets.ReplyPreviewContent = CachedView {
        width: Fill
        height: Fit
        flow: Down
        padding: Inset{left: 16.0, bottom: #(REPLY_PREVIEW_BODY_BOTTOM_PADDING), top: 2.0, right: 11.0}
        cursor: MouseCursor.Hand,
        draw_bg +: {
            fade_start: uniform(1.0)
            fade_end: uniform(1.0)
            fade_enabled: uniform(0.0)
            pixel: fn() {
                let c = self.image.sample(self.pos * self.scale + self.shift)
                let t = clamp((self.pos.y - self.fade_start) / max(self.fade_end - self.fade_start, 0.0001), 0.0, 1.0)
                let f = 1.0 - t * t * (3.0 - 2.0 * t)
                return c * mix(1.0, f, self.fade_enabled)
            }
        }

        View {
            width: Fill
            height: Fit
            flow: Right
            margin: Inset{ bottom: 10.0, top: 0.0, right: 5.0 }
            align: Align{y: 0.5}

            reply_preview_avatar := Avatar {
                width: 19.,
                height: 19.,
                text_view +: {
                    text +: {
                        draw_text +: {
                            text_style: theme.font_regular { font_size: 6.0 }
                        }
                    }
                }
            }

            reply_preview_username := Label {
                width: Fill,
                flow: Flow.Right { wrap: false },
                margin: Inset{ left: 5.0, top: 2 }
                max_lines: 1
                text_overflow: Ellipsis
                draw_text +: {
                    text_style: USERNAME_TEXT_STYLE { font_size: 10 },
                    color: (USERNAME_TEXT_COLOR)
                }
                text: "<Username not available>"
            }
        }

        reply_preview_body := HtmlOrPlaintext {
            margin: Inset{left: 1.5}
            html_view +: {
                html +: {
                    // A compact preview keeps its paragraphs close together.
                    paragraph_margin: Inset{ top: 0.33, bottom: 0.33 }
                    font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE)
                    text_style_normal +: { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) }
                    text_style_italic +: { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) }
                    text_style_bold +: { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) }
                    text_style_bold_italic +: { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) }
                    text_style_fixed +: { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) }
                    a +: {
                        matrix_link_view +: {
                            matrix_link +: {
                                pill_bg +: {
                                    draw_bg +: { border_radius: 5.0 }
                                }
                            }
                        }
                    }
                }
            }
            plaintext_view +: {
                pt_label +: {
                    draw_text +: {
                        text_style: MESSAGE_TEXT_STYLE { font_size: (MESSAGE_REPLY_PREVIEW_FONT_SIZE) },
                    }
                }
            }
        }
    }

    // Wraps a reply preview in a collapsible view.
    mod.widgets.CollapsiblePreview = set_type_default() do #(CollapsiblePreview::register_widget(vm)) {
        visible: false
        width: Fill
        height: Fit
        flow: Down

        preview_content := mod.widgets.ReplyPreviewContent { }

        // A vertical bar drawn along the left of a reply preview, which arcs over to point at the avatar.
        reply_left_bar := View {
            visible: false
            show_bg: true
            draw_bg +: {
                bar_color: instance((COLOR_REPLY_PREVIEW_BAR))
                bar_width: uniform(#(REPLY_PREVIEW_BAR_WIDTH))
                arc_radius: instance(8.0)
                pixel: fn() {
                    let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                    let half = self.bar_width * 0.5
                    let corner = half + self.arc_radius
                    sdf.move_to(half, self.rect_size.y)
                    sdf.line_to(half, corner)
                    sdf.arc_to(corner, corner, self.arc_radius, 180.0 * TORAD, 270.0 * TORAD)
                    sdf.line_to(self.rect_size.x, half)
                    // stroke() blurs its edges inward, which thins the bar, so we pad it by half that blur.
                    return sdf.stroke(self.bar_color, half + 0.5 / sdf.aa)
                }
            }
        }

        reply_expand_button := mod.widgets.ShowMoreLessButton {
            draw_icon +: { svg: (ICON_TRIANGLE_DOWN) }
            text: "Show more…"
        }

        reply_collapse_button := mod.widgets.ShowMoreLessButton {
            draw_icon +: { svg: (ICON_TRIANGLE_UP) }
            text: "Show less"
        }
    }

    // A reply preview shown in the timeline/RoomScreen.
    mod.widgets.RepliedToMessage = mod.widgets.CollapsiblePreview {
        expandable: true
        show_left_bar: true
        padding: Inset{top: 0, right: 12, bottom: 0, left: 12}
    }

    // A reply preview shown in the RoomInputBar above the message text input,
    // which is only collapsed and cannot be expanded.
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
                flow: Flow.Right { wrap: true },
                // Vertically align the text with the X icon in the cancel_reply_button
                padding: Inset{top: 5}

                draw_text +: {
                    text_style: USERNAME_TEXT_STYLE {},
                    color: (COLOR_INPUT_BAR_HEADER_TEXT),
                }
                text: "Replying to:"
            }

            cancel_reply_button := RobrixNegativeIconButton {
                width: Fit,
                height: Fit,
                padding: 13,
                spacing: 0,
                margin: Inset{left: 5, right: 0},
                draw_bg.border_radius: 4.0
                draw_icon.svg: (ICON_CLOSE)
                icon_walk: Walk{width: 16, height: 16, margin: 0}
            }
        }

        reply_preview_content := mod.widgets.CollapsiblePreview {
            visible: true
            expandable: false
        }

        LineH {
            margin: Inset{top: 4, left: 5, right: 5}
        }
    }
}

/// If a reply preview exceeds this height, it will be dranw in a collapsed wrapper.
const REPLY_PREVIEW_FULL_SHOW_THRESHOLD: f64 = 158.0;

/// The space above the "Show more"/"Show less" button.
const REPLY_PREVIEW_TOGGLE_BUTTON_GAP: f64 = 6.0;

/// The height of the "Show more"/"Show less" button.
const REPLY_PREVIEW_TOGGLE_BUTTON_HEIGHT: f64 = 24.0;

/// The height of a collapsed reply preview, which leaves room for its "Show more" button
/// so that it ends up exactly as tall as the tallest reply preview we show in full.
const REPLY_PREVIEW_COLLAPSED_HEIGHT: f64 = REPLY_PREVIEW_FULL_SHOW_THRESHOLD
    - REPLY_PREVIEW_TOGGLE_BUTTON_GAP
    - REPLY_PREVIEW_TOGGLE_BUTTON_HEIGHT;

/// The height of the fade effect at the bottom of a collapsed preview.
const REPLY_PREVIEW_FADE_HEIGHT: f64 = 50.0;

/// Max height of a reply preview's cachedview texture.
const REPLY_PREVIEW_TEXTURE_MAX_HEIGHT: f64 = 160.0;

/// Padding beneath the bottom of the reply preview's body.
const REPLY_PREVIEW_BODY_BOTTOM_PADDING: f64 = 5.0;

/// The width of the left bar alongside a reply preview.
const REPLY_PREVIEW_BAR_WIDTH: f64 = 2.0;

/// The space between the end of the left bar and the avatar it points to,
/// which matches the space between the avatar and the username.
const REPLY_PREVIEW_BAR_AVATAR_GAP: f64 = 9.0;

#[derive(Script, ScriptHook, Widget)]
pub struct CollapsiblePreview {
    #[deref] view: View,
    #[walk] walk: Walk,
    #[redraw] #[area] #[rust] area: Area,
    #[layout] layout: Layout,
    /// Whether this collapsed preview can be expanded.
    #[live] expandable: bool,
    /// Whether to draw the vertical bar to the left of this in-timeline reply preview.
    #[live] show_left_bar: bool,
    /// How far the left bar extends below the bottom of this reply preview.
    #[live] left_bar_overhang: f64,
    #[rust] is_expanded: bool,
    #[rust] collapsible: bool,
    #[rust] last_drawn_height: f64,
    #[rust] toggle_button_rect: Rect,
    #[rust] avatar_rect_in_content: Rect,
    #[rust] next_frame: NextFrame,
}

impl Widget for CollapsiblePreview {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Redraw after a first-draw collapse mispredict (see draw_walk).
        if self.next_frame.is_event(event).is_some() {
            self.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let preview_content = self.view.view(cx, ids!(preview_content));
        let left_bar = self.view.widget(cx, ids!(reply_left_bar));

        // We only use the off-screen cachedview for too-tall reply previews that should be collapsed,
        // as they need the fade effect. Normal small reply previews just draw directly.
        let predict_collapsed = self.last_drawn_height > REPLY_PREVIEW_FULL_SHOW_THRESHOLD
            && !(self.expandable && self.is_expanded);
        if let Some(mut inner) = preview_content.borrow_mut() {
            let optimize = if predict_collapsed { ViewOptimize::Texture } else { ViewOptimize::None };
            inner.set_optimize(cx, optimize);
            inner.set_texture_max_height(Some(REPLY_PREVIEW_TEXTURE_MAX_HEIGHT));
        }

        cx.begin_turtle(
            walk,
            Layout { flow: Flow::Down, clip_x: false, clip_y: false, ..self.layout },
        );
        cx.begin_turtle(
            Walk { width: Size::fill(), height: Size::fit(), ..Walk::default() },
            Layout { flow: Flow::Down, clip_y: true, ..Layout::default() },
        );
        preview_content.draw_all(cx, scope);
        let content_rect = preview_content.area().rect(cx);
        let body_rect = self.view.widget(cx, ids!(preview_content.reply_preview_body)).area().rect(cx);
        let used = cx.turtle().used();
        // Obtain the true height of the reply body to determine if it needs to be collapsed.
        if body_rect.pos.y > content_rect.pos.y {
            self.last_drawn_height = body_rect.pos.y + body_rect.size.y
                + REPLY_PREVIEW_BODY_BOTTOM_PADDING - content_rect.pos.y;
        }
        self.collapsible = self.last_drawn_height > REPLY_PREVIEW_FULL_SHOW_THRESHOLD;
        let collapsed = self.collapsible && !(self.expandable && self.is_expanded);

        // Only apply the fade effect if it was collapsed.
        if predict_collapsed {
            let (fade_start, fade_end, enabled) = if collapsed {
                // Bottom fade band of the collapsed window, as fractions of the texture height.
                (
                    ((REPLY_PREVIEW_COLLAPSED_HEIGHT - REPLY_PREVIEW_FADE_HEIGHT) / used.y) as f32,
                    (REPLY_PREVIEW_COLLAPSED_HEIGHT / used.y) as f32,
                    1.0,
                )
            } else {
                (1.0, 1.0, 0.0)
            };
            if let Some(mut inner) = preview_content.borrow_mut() {
                inner.draw_bg.set_uniform_on_area(cx, live_id!(fade_start), &[fade_start]);
                inner.draw_bg.set_uniform_on_area(cx, live_id!(fade_end), &[fade_end]);
                inner.draw_bg.set_uniform_on_area(cx, live_id!(fade_enabled), &[enabled]);
            }
        }
        if collapsed {
            cx.turtle_mut().set_used(used.x, REPLY_PREVIEW_COLLAPSED_HEIGHT);
        }
        let inner_rect = cx.end_turtle();
        // We only know the true height after it's first drawn, so use next frame
        // to cause another draw which will ensure a proper collapsed or expanded display.
        if collapsed != predict_collapsed {
            self.next_frame = cx.new_next_frame();
        }

        // The "Show more"/"Show less" button is drawn below the content,
        // with a left indent indented to align with the text.
        self.toggle_button_rect = Rect::default();
        let expand_button = self.view.widget(cx, ids!(reply_expand_button));
        let collapse_button = self.view.widget(cx, ids!(reply_collapse_button));
        if self.collapsible && self.expandable {
            let (shown, hidden) = if self.is_expanded {
                (&collapse_button, &expand_button)
            } else {
                (&expand_button, &collapse_button)
            };
            hidden.set_visible(cx, false);
            shown.set_visible(cx, true);
            let button_walk = Walk {
                width: Size::fit(),
                height: Size::Fixed(REPLY_PREVIEW_TOGGLE_BUTTON_HEIGHT),
                margin: Inset { left: body_rect.pos.x - inner_rect.pos.x, top: REPLY_PREVIEW_TOGGLE_BUTTON_GAP, ..Inset::default() },
                ..Walk::default()
            };
            let _ = shown.draw_walk(cx, scope, button_walk);
            self.toggle_button_rect = shown.area().rect(cx);
        } else {
            expand_button.set_visible(cx, false);
            collapse_button.set_visible(cx, false);
        }

        // Draw the left bar from the middle of the avatar to the bottom of the show more button.
        if self.show_left_bar {
            left_bar.set_visible(cx, true);
            // The avatar's rect can be stale for a frame, so we track where it sits within the content.
            let avatar_rect = self.view.widget(cx, ids!(preview_content.reply_preview_avatar)).area().rect(cx);
            if content_rect.contains(avatar_rect.center()) {
                self.avatar_rect_in_content = avatar_rect.translate(-content_rect.pos);
            }
            let avatar_rect = self.avatar_rect_in_content.translate(content_rect.pos);
            let bar_top = avatar_rect.center().y - REPLY_PREVIEW_BAR_WIDTH / 2.0;
            let bar_right = avatar_rect.pos.x - REPLY_PREVIEW_BAR_AVATAR_GAP;
            let bar_bottom = if self.collapsible && self.expandable {
                self.toggle_button_rect.pos.y + self.toggle_button_rect.size.y
            } else {
                content_rect.pos.y + content_rect.size.y
            } + self.left_bar_overhang;
            let bar_walk = Walk {
                abs_pos: Some(dvec2(content_rect.pos.x, bar_top)),
                width:   Size::Fixed((bar_right - content_rect.pos.x).max(0.0)),
                height:  Size::Fixed((bar_bottom - bar_top).max(0.0)),
                ..Walk::default()
            };
            let _ = left_bar.draw_walk(cx, scope, bar_walk);
        } else {
            left_bar.set_visible(cx, false);
        }

        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl CollapsiblePreviewRef {
    /// Sets whether this preview should be expanded, but doesn't redraw anything.
    pub fn set_expanded(&self, is_expanded: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.is_expanded = is_expanded;
        }
    }

    /// Clears the previously-calculated height of the reply preview.
    pub fn reset_measured_height(&self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.last_drawn_height = 0.0;
        }
    }

    /// Whether the preview is currently collapsed.
    pub fn is_collapsed(&self) -> bool {
        self.borrow()
            .map(|inner| inner.collapsible && inner.expandable && !inner.is_expanded)
            .unwrap_or(false)
    }
}

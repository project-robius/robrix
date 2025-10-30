//! A notice that slides into view when someone is typing.

use makepad_widgets::*;

use crate::shared::bouncing_dots::BouncingDotsWidgetExt;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::bouncing_dots::BouncingDots;

    TYPING_NOTICE_ANIMATION_DURATION_SECS = 0.3

    pub TypingNotice = {{TypingNotice}} {
        visible: false
        width: Fill
        height: 30
        flow: Right
        padding: {left: 12.0, top: 8.0, bottom: 8.0, right: 10.0}
        show_bg: true,
        draw_bg: {
            color: #e8f4ff,
        }

        typing_label = <Label> {
            align: {x: 0.0, y: 0.5},
            padding: {left: 5.0, right: 0.0, top: 0.0, bottom: 0.0}
            draw_text: {
                color: (TYPING_NOTICE_TEXT_COLOR),
                text_style: <REGULAR_TEXT>{font_size: 9}
            }
            text: "Someone is typing"
        }

        bouncing_dots = <BouncingDots> {
            margin: {top: 1.1, left: -4 }
            padding: 0.0,
            draw_bg: {
                color: (TYPING_NOTICE_TEXT_COLOR),
            }
        }


        animator: {
            typing_notice_animator = {
                default: show,
                show = {
                    redraw: true,
                    from: { all: Forward { duration: (TYPING_NOTICE_ANIMATION_DURATION_SECS) } }
                    apply: { height: 30 }
                }
                hide = {
                    redraw: true,
                    from: { all: Forward { duration: (TYPING_NOTICE_ANIMATION_DURATION_SECS) } }
                    apply: { height: 0 }
                }
            }
        }
    }
}

/// A notice that slides into view when someone is typing.
#[derive(Live, LiveHook, Widget)]
pub struct TypingNotice {
    #[deref] view: View,
    #[animator] animator: Animator,
}

impl Widget for TypingNotice {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl TypingNotice {
    /// Shows or hides the typing notice based on whether there are any typing users.
    fn show_or_hide(&mut self, cx: &mut Cx, typing_users: &[String]) {
        let typing_notice_text = match typing_users {
            [] => {
                // Animate out the typing notice view (sliding it out towards the bottom).
                self.animator_play(cx, ids!(typing_notice_animator.hide));
                self.view.bouncing_dots(ids!(bouncing_dots)).stop_animation(cx);
                return;
            }
            [user] => format!("{user} is typing "),
            [user1, user2] => format!("{user1} and {user2} are typing "),
            [user1, user2, others @ ..] => {
                if others.len() > 1 {
                    format!("{user1}, {user2}, and {} are typing ", &others[0])
                } else {
                    format!(
                        "{user1}, {user2}, and {} others are typing ",
                        others.len()
                    )
                }
            }
        };
        // Set the typing notice text and make its view visible.
        self.view.label(ids!(typing_label)).set_text(cx, &typing_notice_text);
        self.view.set_visible(cx, true);
        // Animate in the typing notice view (sliding it up from the bottom).
        self.animator_play(cx, ids!(typing_notice_animator.show));
        // Start the typing notice text animation of bouncing dots.
        self.view.bouncing_dots(ids!(bouncing_dots)).start_animation(cx);
    }
}

impl TypingNoticeRef {
    /// Shows or hides the typing notice based on whether there are any typing users.
    pub fn show_or_hide(&self, cx: &mut Cx, typing_users: &[String]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_or_hide(cx, typing_users);
        }
    }
}

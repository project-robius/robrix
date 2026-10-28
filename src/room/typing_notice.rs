//! A notice that slides into view when someone is typing.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.TYPING_NOTICE_ANIMATION_DURATION_SECS = 0.3

    mod.widgets.TypingNotice = set_type_default() do #(TypingNotice::register_widget(vm)) {
        ..mod.widgets.SolidView

        visible: false
        width: Fill
        height: 30
        flow: Right
        padding: Inset{left: 12.0, top: 8.0, bottom: 8.0, right: 10.0}

        show_bg: true
        draw_bg +: {
            color: (COLOR_TYPING_NOTICE_BG)
        }

        typing_label := Label {
            align: Align{x: 0.0, y: 0.5},
            padding: Inset{left: 5.0, right: 0.0, top: 0.0, bottom: 0.0}
            draw_text +: {
                color: (TYPING_NOTICE_TEXT_COLOR),
                text_style: REGULAR_TEXT {font_size: 9}
            }
            text: "Someone is typing"
        }

        bouncing_dots := BouncingDots {
            margin: Inset{top: 1.1, left: -4 }
            padding: 0.0,
            draw_bg.color: (TYPING_NOTICE_TEXT_COLOR)
        }

        animator: Animator{
            typing_notice_animator: {
                default: @show
                show: AnimatorState{
                    redraw: true,
                    from: { all: Forward { duration: (mod.widgets.TYPING_NOTICE_ANIMATION_DURATION_SECS) } }
                    apply: { height: 30 }
                }
                hide: AnimatorState{
                    redraw: true,
                    from: { all: Forward { duration: (mod.widgets.TYPING_NOTICE_ANIMATION_DURATION_SECS) } }
                    apply: { height: 0 }
                }
            }
        }
    }
}

/// A notice that slides into view when someone is typing.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct TypingNotice {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
    /// Whether we're animating out, after which we stop drawing.
    #[rust] is_hiding: bool,
}

impl Widget for TypingNotice {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        // Stop drawing once we've began animated out.
        if self.is_hiding && !self.animator.is_track_animating(id!(typing_notice_animator)) {
            self.is_hiding = false;
            self.view.set_visible(cx, false);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl TypingNotice {
    /// Shows or hides the typing notice based on whether there are any typing users.
    fn show_or_hide(&mut self, cx: &mut Cx, typing_users: &[String], animate: Animate) {
        let typing_notice_text = match typing_users {
            [] => {
                // Animate out the typing notice view (sliding it out towards the bottom).
                self.animator_toggle(cx, false, animate, ids!(typing_notice_animator.show), ids!(typing_notice_animator.hide));
                match animate {
                    Animate::Yes => self.is_hiding = true,
                    Animate::No => self.view.set_visible(cx, false),
                }
                self.redraw(cx);
                return;
            }
            [user] => format!("{user} is typing "),
            [user1, user2] => format!("{user1} and {user2} are typing "),
            [user1, user2, others @ ..] => {
                if others.len() == 1 {
                    format!("{user1}, {user2}, and {} are typing ", others[0])
                } else {
                    format!(
                        "{user1}, {user2}, and {} others are typing ",
                        others.len()
                    )
                }
            }
        };
        // Set the typing notice text and make its view visible.
        self.view.label(cx, ids!(typing_label)).set_text(cx, &typing_notice_text);
        self.view.set_visible(cx, true);
        self.is_hiding = false;
        // Animate in the typing notice view (sliding it up from the bottom).
        self.animator_toggle(cx, true, animate, ids!(typing_notice_animator.show), ids!(typing_notice_animator.hide));
    }
}

impl TypingNoticeRef {
    /// Shows or hides the typing notice based on whether there are any typing users.
    pub fn show_or_hide(&self, cx: &mut Cx, typing_users: &[String], animate: Animate) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_or_hide(cx, typing_users, animate);
        }
    }
}

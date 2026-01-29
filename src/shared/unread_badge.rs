//! This module defines a badge that shows the count of unread mentions (in red)
//! or unread messages (in gray).

use makepad_widgets::*;

use crate::shared::styles::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;
    use link::shaders::*;
    use crate::shared::styles::*;

    pub UnreadBadge = {{UnreadBadge}} {
        width: 30, height: 20,
        align: { x: 0.5, y: 0.5 }
        flow: Overlay,

        rounded_view = <View> {
            width: Fill,
            height: Fill,
            show_bg: true,
            draw_bg: {
                instance badge_color: (COLOR_UNREAD_BADGE_MESSAGES),
                instance border_radius: 4.0
                // Adjust this border_size to larger value to make oval smaller 
                instance border_size: 2.0
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                    sdf.box(
                        self.border_size,
                        1.0,
                        self.rect_size.x - (self.border_size * 2.0),
                        self.rect_size.y - 2.0,
                        max(1.0, self.border_radius)
                    )
                    sdf.fill_keep(self.badge_color);
                    return sdf.result;
                }
            }
        }
        // Label that displays the unread message count
        label_count = <Label> {
            padding: 0,
            width: Fit,
            height: Fit,
            flow: Right, // do not wrap
            text: "",
            draw_text: {
                color: #ffffff,
                text_style: {font_size: 8.0},
            }
        }
    }
}


#[derive(Live, LiveHook, Widget)]
pub struct UnreadBadge {
    #[deref] view: View,
    #[live] is_marked_unread: bool,
    #[live] unread_mentions: u64,
    #[live] unread_messages: u64,
}

impl Widget for UnreadBadge {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {

        /// Helper function to format the badge's rounded rectangle.
        ///
        /// The rounded rectangle needs to be wider for longer text.
        /// It also adds a plus sign at the end if the unread count is greater than 99. 
        fn format_border_and_truncation(count: u64) -> (f64, &'static str) {
            let (border_size, plus_sign) = if count > 99 {
                (0.0, "+")
            } else if count > 9 {
                (2.0, "")
            } else {
                (5.0, "")
            };
            (border_size, plus_sign)
        }

        // If there are unread mentions, show red badge and the number of unread mentions
        if self.unread_mentions > 0 {
            let (border_size, plus_sign) = format_border_and_truncation(self.unread_mentions);
            self.label(ids!(label_count))
                .set_text(cx, &format!("{}{plus_sign}", std::cmp::min(self.unread_mentions, 99)));
            self.view(ids!(rounded_view)).apply_over(cx, live!{
                draw_bg: {
                    border_size: (border_size),
                    badge_color: (COLOR_UNREAD_BADGE_MENTIONS)
                }
            });
            self.visible = true;
        }
        // If there are no unread mentions but this is marked as unread, show the badge as a dot.
        else if self.is_marked_unread {
            self.label(ids!(label_count)).set_text(cx, "");
            self.view(ids!(rounded_view)).apply_over(cx, live!{
                draw_bg: {
                    border_size: 6.0, // larger value = smaller dot
                    badge_color: (COLOR_UNREAD_BADGE_MARKED)
                }
            });
            self.visible = true;
        }
        // If there are no unread mentions but there are unread messages, show gray badge and the number of unread messages
        else if self.unread_messages > 0 {
            let (border_size, plus_sign) = format_border_and_truncation(self.unread_messages);
            self.label(ids!(label_count))
                .set_text(cx, &format!("{}{plus_sign}", std::cmp::min(self.unread_messages, 99)));
            self.view(ids!(rounded_view)).apply_over(cx, live!{
                draw_bg: {
                    border_size: (border_size),
                    badge_color: (COLOR_UNREAD_BADGE_MESSAGES)
                }
            });
            self.visible = true;
        }
        else {
            // If there are no unreads of any kind, hide the badge
            self.visible = false;
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl UnreadBadgeRef {
    /// Sets the unread mentions and messages counts without explicitly redrawing the badge.
    pub fn update_counts(&self, is_marked_unread: bool, num_unread_mentions: u64, num_unread_messages: u64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.is_marked_unread = is_marked_unread;
            inner.unread_mentions = num_unread_mentions;
            inner.unread_messages = num_unread_messages;
            inner.visible = is_marked_unread || num_unread_mentions > 0 || num_unread_messages > 0;
        }
    }
}

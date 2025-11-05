use makepad_widgets::*;

const SCROLL_TO_BOTTOM_SPEED: f64 = 90.0;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    ICO_JUMP_TO_BOTTOM = dep("crate://self/resources/icon_jump_to_bottom.svg")

    // A jump to bottom button that appears when the timeline is not at the bottom.
    pub JumpToBottomButton = {{JumpToBottomButton}} {
        width: Fill,
        height: Fill,
        flow: Overlay,
        align: {x: 1.0, y: 1.0},
        visible: false,
        <View> {
            width: 65, height: 75,
            align: {x: 0.5, y: 1.0},
            flow: Overlay,
            jump_to_bottom_button = <IconButton> {
                spacing: 0,
                width: 50, height: 50,
                margin: {bottom: 8},
                draw_icon: {svg_file: (ICO_JUMP_TO_BOTTOM)},
                icon_walk: {width: 20, height: 20, margin: {top: 10, right: 4.5} }
                // draw a circular background for the button
                draw_bg: {
                    instance background_color: #edededce,
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        let c = self.rect_size * 0.5;
                        sdf.circle(c.x, c.x, c.x);
                        sdf.fill_keep(self.background_color);
                        return sdf.result
                    }
                }
            }

            // A badge overlay on the jump to bottom button showing unread messages
            unread_message_badge = <View> {
                width: 25, height: 20,
                align: {
                    x: 0.5,
                    y: 0.5
                }
                visible: false,
                flow: Overlay,
                green_rounded_label = <View> {
                    width: Fill,
                    height: Fill,
                    show_bg: true,
                    draw_bg: {
                        color: (COLOR_UNREAD_MESSAGE_BADGE)
                        instance border_radius: 4.0
                        // Adjust this border_size to larger value to make oval smaller 
                        instance border_size: 2.0
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                            sdf.box(
                                self.border_size,
                                self.border_size,
                                self.rect_size.x - (self.border_size * 2.0),
                                self.rect_size.y - (self.border_size * 2.0),
                                max(1.0, self.border_radius)
                            )
                            sdf.fill_keep(self.color)
                            return sdf.result;
                        }
                    }
                }
                // Label that displays the unread message count
                unread_messages_count = <Label> {
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
        
    }
}


#[derive(LiveHook, Live, Widget)]
pub struct JumpToBottomButton {
    #[deref] view: View,
}

impl Widget for JumpToBottomButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl JumpToBottomButton {
    /// Updates the visibility of the jump to bottom button *without* redrawing.
    ///
    /// * If `is_at_bottom` is `true`, both the main jump to bottom view and
    ///   the unread message badge are made invisible, because we consider all messages
    ///   to be read by the user if the timeline has reached the bottom.
    /// * If `is_at_bottom` is `false`, only the main jump to bottom "parent" view
    ///   is made visible; the unread message badge is *not* made visible, as that is done
    ///   via a separate call to [`JumpToBottomButton::show_unread_message_badge()`].
    pub fn update_visibility(&mut self, cx: &mut Cx, is_at_bottom: bool) {
        if is_at_bottom {
            self.visible = false;
            self.view(ids!(unread_message_badge)).set_visible(cx, false);
        } else {
            self.visible = true;
        }
    }

    /// Sets both the jump to bottom view and its unread message badge to be visible.
    ///
    /// This does not automatically redraw any views.
    /// If unread_message_count is `0`, the unread message badge is hidden.
    pub fn show_unread_message_badge(&mut self, cx: &mut Cx, count: UnreadMessageCount) {
        match count {
            UnreadMessageCount::Unknown => {
                self.visible = true;
                self.view(ids!(unread_message_badge)).set_visible(cx, true);
                self.label(ids!(unread_messages_count)).set_text(cx, "");
            }
            UnreadMessageCount::Known(0) => {
                self.visible = false;
                self.view(ids!(unread_message_badge)).set_visible(cx, false);
                self.label(ids!(unread_messages_count)).set_text(cx, "");
            }
            UnreadMessageCount::Known(unread_message_count) => {
                self.visible = true;
                self.view(ids!(unread_message_badge)).set_visible(cx, true);
                let (border_size, plus_sign) = if unread_message_count > 99 {
                    (0.0, "+")
                } else if unread_message_count > 9 {
                    (1.0, "")
                } else {
                    (2.0, "")
                };
                self.label(ids!(unread_messages_count)).set_text(
                    cx,
                    &format!("{}{plus_sign}", std::cmp::min(unread_message_count, 99))
                );
                self.view(ids!(unread_message_badge.green_rounded_label)).apply_over(cx, live!{
                    draw_bg: {
                        border_size: (border_size),
                    }
                });
            }
        }
        
    }

    /// Updates the visibility of the jump to bottom button and the unread message badge
    /// based on the given actions and the state of the portal list.
    ///
    /// Also handles the click event on the jump to bottom button.
    ///
    /// Redraws the jump to bottom button if its visibility changes.
    pub fn update_from_actions(
        &mut self,
        cx: &mut Cx,
        portal_list: &PortalListRef,
        actions: &Actions,
    ) {
        let was_visible = self.visible;
        // Note: here, we could choose to set visibility ONLY IF the portallist was scrolled.
        //       However, we intentionally skip that check, as it's actually more expensive
        //       to check if the portallist has been scrolled than to just directly
        //       query the portallist's `at_end` state and set the visibility accordingly.

        if self.button(ids!(jump_to_bottom_button)).clicked(actions) {
            portal_list.smooth_scroll_to_end(
                cx,
                SCROLL_TO_BOTTOM_SPEED,
                None,
            );
            self.update_visibility(cx, false);
        } else {
            self.update_visibility(cx, portal_list.is_at_end());
        }

        if self.visible != was_visible {
            self.redraw(cx);
        }
    }

}

impl JumpToBottomButtonRef {
    /// See [`JumpToBottomButton::update_visibility()`].
    pub fn update_visibility(&self, cx: &mut Cx, is_at_bottom: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.update_visibility(cx, is_at_bottom);
        }
    }

    /// See [`JumpToBottomButton::show_unread_message_badge()`].
    pub fn show_unread_message_badge(&self, cx: &mut Cx, count: UnreadMessageCount) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_unread_message_badge(cx, count);
        }
    }

    /// See [`JumpToBottomButton::update_from_actions()`].
    pub fn update_from_actions(
        &self,
        cx: &mut Cx,
        portal_list: &PortalListRef,
        actions: &Actions,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.update_from_actions(cx, portal_list, actions);
        }
    }
}

/// The number of unread messages in a room.
#[derive(Clone, Debug)]
pub enum UnreadMessageCount {
    /// There are unread messages, but we do not know how many.
    Unknown,
    /// There are unread messages, and we know exactly how many.
    Known(u64)
}

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

        jump_to_bottom_button = <IconButton> {
            margin: {right: 15.0, bottom: 15.0},
            width: 50, height: 50,
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
            margin: {right: 25.0, bottom: 8.0},
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
                    instance radius: 4.0
                    // Adjust this border_width to larger value to make oval smaller 
                    instance border_width: 2.0
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                        sdf.box(
                            self.border_width,
                            self.border_width,
                            self.rect_size.x - (self.border_width * 2.0),
                            self.rect_size.y - (self.border_width * 2.0),
                            max(1.0, self.radius)
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
                text: "",
                draw_text: {
                    color: #ffffff,
                    text_style: {font_size: 8.0},
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
    pub fn update_visibility(&mut self, is_at_bottom: bool) {
        if is_at_bottom {
            self.visible = false;
            self.view(id!(unread_message_badge)).set_visible(false);
        } else {
            self.visible = true;
        }
    }

    /// Sets both the jump to bottom view and its unread message badge to be visible.
    ///
    /// This does not automatically redraw any views.
    /// If unread_message_count is `0`, the unread message badge is hidden.
    pub fn show_unread_message_badge(&mut self, cx: &mut Cx, unread_message_count: UnReadMessageCount) {
        match unread_message_count {
            UnReadMessageCount::Unknown => {
                self.visible = true;
                self.view(id!(unread_message_badge)).set_visible(true);
                self.label(id!(unread_messages_count)).set_text("");
            }
            UnReadMessageCount::Known(unread_message_count) => {
                if unread_message_count > 0 {
                    self.visible = true;
                    self.view(id!(unread_message_badge)).set_visible(true);
                    self.label(id!(unread_messages_count)).set_text(&format!(
                        "{}{}",
                        std::cmp::min(unread_message_count, 99),
                        if unread_message_count > 99 { "+" } else { "" }
                    ));
                    if unread_message_count > 99 {
                        self.view(id!(unread_message_badge.green_rounded_label)).apply_over(cx, live!{
                            draw_bg: {
                                border_width: 0.0
                            }
                        });
                    } else if unread_message_count > 9 {
                        self.view(id!(unread_message_badge.green_rounded_label)).apply_over(cx, live!{
                            draw_bg: {
                                border_width: 1.0
                            }
                        });
                    } else {
                        self.view(id!(unread_message_badge.green_rounded_label)).apply_over(cx, live!{
                            draw_bg: {
                                border_width: 2.0
                            }
                        });
                    }
                } else {
                    self.visible = false;
                    self.view(id!(unread_message_badge)).set_visible(false);
                    self.label(id!(unread_messages_count)).set_text("");
                }
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

        if self.button(id!(jump_to_bottom_button)).clicked(actions) {
            portal_list.smooth_scroll_to_end(
                cx,
                SCROLL_TO_BOTTOM_SPEED,
                None,
            );
            self.update_visibility(false);
        } else {
            self.update_visibility(portal_list.is_at_end());
        }

        if self.visible != was_visible {
            self.redraw(cx);
        }
    }

}

impl JumpToBottomButtonRef {
    /// See [`JumpToBottomButton::update_visibility()`].
    pub fn update_visibility(&self, is_at_bottom: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.update_visibility(is_at_bottom);
        }
    }

    /// See [`JumpToBottomButton::show_unread_message_badge()`].
    pub fn show_unread_message_badge(&self, cx: &mut Cx, unread_message_count: UnReadMessageCount) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_unread_message_badge(cx, unread_message_count);
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

/// The number of unread messages in the room
#[derive(Clone, Debug)]
pub enum UnReadMessageCount {
    /// Display green badge without text
    Unknown,
    /// Display green badge and text for the number of unread messages
    Known(u64)
}
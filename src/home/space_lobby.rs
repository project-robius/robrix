//! Contains two widgets related to the top-level view of a space.
//!
//! 1. `SpaceLobby`: shows details about a space, including its name, avatar,
//!    members, topic, and the full list of rooms and sub-spaces within it.
//! 2. `SpaceLobbyEntry`: the button that can be shown in a RoomsList
//!    that allows the user to click on it to show the `SpaceLobby`.
//!

use makepad_widgets::*;


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::avatar::*;

    // An entry in the RoomsList that will show the SpaceLobby when clicked.
    SpaceLobbyEntry = {{SpaceLobbyEntry}}<RoundedView> {
        visible: false, // only visible when a space is selected
        width: Fill,
        height: Fit,
        flow: Right,
        align: {y: 0.5}
        cursor: Hand

        show_bg: true
        draw_bg: {
            instance hover: 0.0
            instance active: 0.0

            color: (COLOR_NAVIGATION_TAB_BG)
            uniform color_hover: (COLOR_NAVIGATION_TAB_BG_HOVER)
            uniform color_active: (COLOR_NAVIGATION_TAB_BG_ACTIVE)

            border_size: 0.0
            border_color: #0000
            uniform inset: vec4(0.0, 0.0, 0.0, 0.0)
            border_radius: 4.0

            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color,
                        self.color_hover,
                        self.hover
                    ),
                    self.color_active,
                    self.active
                )
            }

            fn get_border_color(self) -> vec4 {
                return self.border_color
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                sdf.box(
                    self.inset.x + self.border_size,
                    self.inset.y + self.border_size,
                    self.rect_size.x - (self.inset.x + self.inset.z + self.border_size * 2.0),
                    self.rect_size.y - (self.inset.y + self.inset.w + self.border_size * 2.0),
                    max(1.0, self.border_radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_size > 0.0 {
                    sdf.stroke(self.get_border_color(), self.border_size)
                }
                return sdf.result;
            }
        }

        icon = <Icon> {
            width: 30,
            height: 30,
            align: {x: 0.5, y: 0.5}
            draw_icon: {
                svg_file: (ICON_HOME)

                instance active: 0.0
                instance hover: 0.0
                instance down: 0.0

                color: (COLOR_TEXT)
                uniform color_hover: (COLOR_TEXT)
                uniform color_active: (COLOR_PRIMARY)

                fn get_color(self) -> vec4 {
                    return mix(
                        mix(
                            self.color,
                            self.color_hover,
                            self.hover
                        ),
                        self.color_active,
                        self.active
                    )
                }
            }
            icon_walk: { width: 25, height: 25 }
        }

        space_lobby_label = <Label> {
            width: Fill, height: Fit
            flow: Right,
            padding: 0,

            draw_text: {
                instance active: 0.0
                instance hover: 0.0
                instance down: 0.0

                color: (COLOR_TEXT)
                uniform color_hover: (COLOR_TEXT)
                uniform color_active: (COLOR_PRIMARY)

                text_style: <THEME_FONT_BOLD>{font_size: 9}
                // text_style: <REGULAR_TEXT>{font_size: 9}
                wrap: Ellipsis,

                fn get_color(self) -> vec4 {
                    return mix(
                        mix(
                            self.color,
                            self.color_hover,
                            self.hover
                        ),
                        self.color_active,
                        self.active
                    )
                }
            }
            text: "Space Lobby"
        }

        animator: {
            hover = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.15}}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 0.0}], hover: 0.0}
                        space_lobby_label = { draw_text: {down: [{time: 0.0, value: 0.0}], hover: 0.0} }
                        icon = { draw_icon: {down: [{time: 0.0, value: 0.0}], hover: 0.0} }
                    }
                }
                on = {
                    from: {all: Snap}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 0.0}], hover: 1.0}
                        space_lobby_label = { draw_text: {down: [{time: 0.0, value: 0.0}], hover: 1.0} }
                        icon = { draw_icon: {down: [{time: 0.0, value: 0.0}], hover: 1.0} }
                    }
                }
                down = {
                    from: {all: Forward {duration: 0.2}}
                    apply: {
                        draw_bg: {down: [{time: 0.0, value: 1.0}], hover: 1.0,}
                        space_lobby_label = { draw_text: {down: [{time: 0.0, value: 1.0}], hover: 1.0,} }
                        icon = { draw_icon: {down: [{time: 0.0, value: 1.0}], hover: 1.0,} }
                    }
                }
            }
        }
    }
}



#[derive(Live, LiveHook, Widget)]
pub struct SpaceLobbyEntry {
    #[deref] view: View,
    #[animator] animator: Animator,
}

impl Widget for SpaceLobbyEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        let area = self.draw_bg.area();
        match event.hits(cx, area) {
            Hit::FingerHoverIn(_) => {
                self.animator_play(cx, ids!(hover.on));
            }
            Hit::FingerHoverOut(_) => {
                self.animator_play(cx, ids!(hover.off));
            }
            Hit::FingerDown(fe) => {
                self.animator_play(cx, ids!(hover.down));
            }
            Hit::FingerLongPress(_lp) => {
                self.animator_play(cx, ids!(hover.down));
            }
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                self.animator_play(cx, ids!(hover.on));
                cx.action(SpaceLobbyAction::SpaceLobbyEntryClicked);
            }
            Hit::FingerUp(fe) if !fe.is_over => {
                self.animator_play(cx, ids!(hover.off));
            }
            Hit::FingerMove(_fe) => { }
            _ => {}
        }
    }
    
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}


#[derive(Debug)]
pub enum SpaceLobbyAction {
    SpaceLobbyEntryClicked,
}

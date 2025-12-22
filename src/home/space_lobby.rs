//! Contains two widgets related to the top-level view of a space.
//!
//! 1. `SpaceLobby`: shows details about a space, including its name, avatar,
//!    members, topic, and the full list of rooms and subspaces within it.
//! 2. `SpaceLobbyEntry`: the button that can be shown in a RoomsList
//!    that allows the user to click on it to show the `SpaceLobby`.
//!

use makepad_widgets::*;
use crate::utils::RoomNameId;


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::avatar::*;

    // An entry in the RoomsList that will show the SpaceLobby when clicked.
    pub SpaceLobbyEntry = {{SpaceLobbyEntry}}<RoundedView> {
        visible: false, // only visible when a space is selected
        width: Fill,
        height: 35, // same as CollapsibleHeader
        flow: Right,
        align: {y: 0.5}
        padding: 5,
        margin: {top: 5, bottom: 10}
        cursor: Hand

        show_bg: true
        draw_bg: {
            instance hover: 0.0
            instance active: 0.0

            color: (COLOR_NAVIGATION_TAB_BG)
            uniform color_hover: (COLOR_NAVIGATION_TAB_BG_HOVER)
            uniform color_active: (COLOR_ACTIVE_PRIMARY)

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
            width: 25,
            height: 25,
            margin: {left: 5, right: 3}
            align: {x: 0.5, y: 0.5}
            draw_icon: {
                svg_file: (ICON_HIERARCHY)

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
            icon_walk: { width: 25, height: 20, margin: {left: 0, bottom: 0} }
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

                text_style: <REGULAR_TEXT>{font_size: 11},
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
            text: "Explore this Space"
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

    // The main view that shows the lobby (homepage) for a space.
    pub SpaceLobbyScreen = {{SpaceLobbyScreen}} {
        width: Fill, height: Fill,
        padding: {top: 100}
        align: {x: 0.5}

        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY)
        }

        title = <Label> {
            flow: RightWrap,
            align: {x: 0.5}
            draw_text: {
                text_style: <TITLE_TEXT>{font_size: 13},
                color: #000
                wrap: Word
            }
            text: "Space Lobby Screen is not yet implemented"
        }
    }
}


/// A clickable entry shown in the RoomsList that will show the space lobby when clicked.
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
            Hit::FingerDown(_fe) => {
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


/// The view showing the lobby/homepage for a given space.
#[derive(Live, LiveHook, Widget)]
pub struct SpaceLobbyScreen {
    #[deref] view: View,
    #[rust] space_name_id: Option<RoomNameId>,
}

impl Widget for SpaceLobbyScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SpaceLobbyScreen {
    pub fn set_displayed_space(&mut self, cx: &mut Cx, space_name_id: &RoomNameId) {
        // If this space is already being displayed, then do nothing.
        if self.space_name_id.as_ref().is_some_and(|sni| sni.room_id() == space_name_id.room_id()) { return; }

        self.view.label(ids!(title)).set_text(cx, &format!(
            "Space Lobby Screen is not yet implemented.\n\n\
            Space Name: {}\n\nSpace ID: {}",
            space_name_id,
            space_name_id.room_id(),
        ));

        // TODO: populate the rest of the space lobby screen content

        self.space_name_id = Some(space_name_id.clone());
    }
}

impl SpaceLobbyScreenRef {
    pub fn set_displayed_space(&self, cx: &mut Cx, space_name_id: &RoomNameId) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_displayed_space(cx, space_name_id);
    }
}

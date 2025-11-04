use std::ops::Deref;

use makepad_widgets::*;
use ruma::{OwnedRoomId, OwnedRoomOrAliasId, OwnedServerName, room::JoinRuleSummary};

use crate::{home::room_screen::RoomScreenWidgetRefExt, room::BasicRoomDetails, shared::restore_status_view::RestoreStatusViewWidgetExt, utils::room_name_or_id};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::shared::restore_status_view::*;
    use crate::home::room_screen::RoomScreen;

    pub PreviewScreen = {{PreviewScreen}}<ScrollXYView> {
        width: Fill, height: Fill,
        flow: Down,
        spacing: 0,

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY_DARKER),
        }
        restore_status_view = <RestoreStatusView> {}

        room_preview_screen_wrapper = <View> {
            width: Fill, height: Fill,
            flow: Down,
            show_bg: true,
            draw_bg: {
                color: (COLOR_PRIMARY_DARKER)
            }

            can_not_preview_screen = <View> {
                visible: false,
                width: Fill, height: Fill,
                align: {x: 0.5, y: 0.5},
                flow: Down,
                spacing: 10,

                preview_message = <Label> {
                    margin: {top: 15, bottom: 15},
                    width: Fill, height: Fit,
                    align: {x: 0.5, y: 0},
                    flow: RightWrap,
                    text: "",
                    draw_text: {
                        text_style: <REGULAR_TEXT>{
                            font_size: 15,
                        },
                        color: #000
                        wrap: Word
                    }
                }

                join_button = <RobrixIconButton> {
                    visible: false,
                    align: {x: 0.5, y: 0.5}
                    padding: 15,
                    draw_icon: {
                        svg_file: (ICON_CHECKMARK)
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_FG_ACCEPT_GREEN),
                        color: (COLOR_BG_ACCEPT_GREEN)
                    }
                    text: "Join Room"
                    draw_text:{
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                }
            }

            can_preview_screen = <RoomScreen> {
                visible: false,
            }
        }
    }
}


#[derive(Debug, Clone, DefaultNone)]
pub enum RoomPreviewAction {
    Selected {
        room_or_alias_id: OwnedRoomOrAliasId,
        via: Vec<OwnedServerName>,
    },
    None,
}

#[derive(Clone, Debug)]
pub struct PreviewDetails {
    pub room_basic_details: BasicRoomDetails,
    pub is_world_readable: bool,
    pub join_rule: Option<JoinRuleSummary>,
}

impl Deref for PreviewDetails {
    type Target = BasicRoomDetails;

    fn deref(&self) -> &Self::Target {
        &self.room_basic_details
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreviewState {
    #[default]
    Loading,
    Loaded,
    Error,
}

#[derive(Live, LiveHook, Widget)]
pub struct PreviewScreen {
    #[deref] view: View,

    #[rust] info: Option<PreviewDetails>,
    #[rust] room_id: Option<OwnedRoomId>,
    #[rust] room_name: String,
    #[rust] is_loaded: bool,
}

impl Widget for PreviewScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.is_loaded {
            let mut restore_status_view = self.view.restore_status_view(ids!(restore_status_view));
            restore_status_view.set_content(cx, !self.is_loaded, &self.room_name);
            return restore_status_view.draw(cx, scope);
        }

        let Some(info) = self.info.as_ref() else {
            return self.view.draw_walk(cx, scope, walk);
        };

        let preview_screen_wrapper = self.view(ids!(room_preview_screen_wrapper));
        let can_preview_screen = preview_screen_wrapper.room_screen(ids!(can_preview_screen));
        let can_not_be_previewed_screen = preview_screen_wrapper.view(ids!(can_not_preview_screen));

        if info.is_world_readable {
            can_preview_screen.set_visible(cx, true);
            can_not_be_previewed_screen.set_visible(cx, false);
            can_preview_screen.set_displayed_preview_room(
                cx,
                info.room_id.clone(),
                info.room_name.clone(),
            );
            self.redraw(cx);
        } else {
            can_preview_screen.set_visible(cx, false);
            can_not_be_previewed_screen.set_visible(cx, true);
            if let Some(join_rule) = &info.join_rule {
                match join_rule {
                    JoinRuleSummary::Public => {
                        can_not_be_previewed_screen.label(ids!(preview_message)).set_text(cx, "This is a public room. You can join it by clicking the button");
                        let join_button = can_not_be_previewed_screen.button(ids!(join_button));
                        join_button.set_visible(cx, true);
                    }
                    _ => {
                        can_not_be_previewed_screen.label(ids!(preview_message)).set_text(cx, "You cannot preview this room as it is not world readable.");
                        let join_button = can_not_be_previewed_screen.button(ids!(join_button));
                        join_button.set_visible(cx, false);
                    }
                }
            }
            self.redraw(cx);
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl PreviewScreen {
    pub fn set_displayed_preview<S: Into<Option<String>>>(&mut self, cx: &mut Cx, room_id: OwnedRoomId, room_name: S, info: PreviewDetails) {
        self.room_id = Some(room_id.clone());
        self.room_name = room_name_or_id(room_name.into(), &room_id);
        self.info = Some(info);
        self.is_loaded = true;
        self.redraw(cx);
        self.view.restore_status_view(ids!(restore_status_view)).set_visible(cx, !self.is_loaded);
    }
}

impl PreviewScreenRef {
    pub fn set_displayed_preview<S: Into<Option<String>>>(&mut self, cx: &mut Cx, room_id: OwnedRoomId, room_name: S, info: PreviewDetails) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_displayed_preview(cx, room_id, room_name, info);
        }
    }
}
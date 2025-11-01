use std::ops::Deref;

use makepad_widgets::*;
use ruma::{OwnedRoomId, room::JoinRuleSummary};

use crate::{app::AppStateAction, room::BasicRoomDetails, shared::restore_status_view::RestoreStatusViewWidgetExt, utils::room_name_or_id};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::shared::restore_status_view::*;

    pub CanNotPreviewScreen = {{CanNotPreviewScreen}}<ScrollXYView> {
        width: Fill,
        height: Fill,
        flow: Down,
        align: {x: 0.5, y: 0.5},
        spacing: 0,

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY_DARKER),
        }
        restore_status_view = <RestoreStatusView> {}

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
}

#[derive(Clone, Debug)]
pub struct CanNotPreviewDetails {
    pub room_basic_details: BasicRoomDetails,
    pub join_rule: Option<JoinRuleSummary>,
}

impl Deref for CanNotPreviewDetails {
    type Target = BasicRoomDetails;

    fn deref(&self) -> &Self::Target {
        &self.room_basic_details
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct CanNotPreviewScreen {
    #[deref] view: View,

    #[rust] info: Option<CanNotPreviewDetails>,
    #[rust] room_id: Option<OwnedRoomId>,
    #[rust] room_name: String,
    #[rust] is_loaded: bool,
}

impl Widget for CanNotPreviewScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            // First, we quickly loop over the actions up front to handle the case
            // where this room was restored and has now been successfully loaded from the homeserver.
            for action in actions {
                if let Some(AppStateAction::RoomLoadedSuccessfully(room_id)) = action.downcast_ref() {
                    if self.room_id.as_ref().is_some_and(|inner_room_id| inner_room_id == room_id) {
                        self.set_displayed(cx, room_id.clone(), self.room_name.clone(), self.info.clone().unwrap());
                        break;
                    }
                }
            }

            let Some(info) = self.info.as_ref() else { return; };

            if let Some(modifiers) = self.view.button(ids!(join_button)).clicked_modifiers(actions) {
                if modifiers.shift {
                    log!("Shift-clicked join room button, opening join leave modal without joining directly.");
                } else {
                    log!("Joining room {:?}", info.room_id);
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.is_loaded {
            let mut restore_status_view = self.view.restore_status_view(ids!(restore_status_view));
            restore_status_view.set_content(cx, !self.is_loaded, &self.room_name);
            return restore_status_view.draw(cx, scope);
        }

        let preview_message = self.view.label(ids!(preview_message));
        let join_button = self.view.button(ids!(join_button));


        let Some(info) = self.info.as_ref() else { return DrawStep::done(); };

        match info.join_rule {
            Some(JoinRuleSummary::Public) => {
                preview_message.set_text(cx, "This is room is not world readable, you need to join to see its contents.");
                join_button.set_visible(cx, true);
            }
            _ => {
                preview_message.set_text(cx, "This room is not world readable and you cannot join it. Unless you have an invite, you will not be able to see its contents.");
                join_button.set_visible(cx, false);
            }
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl CanNotPreviewScreen {
    pub fn set_displayed<S: Into<Option<String>>>(&mut self, cx: &mut Cx, room_id: OwnedRoomId, room_name: S, info: CanNotPreviewDetails) {
        self.room_id = Some(room_id.clone());
        self.room_name = room_name_or_id(room_name.into(), &room_id);
        self.info = Some(info);
        self.is_loaded = true;
        self.redraw(cx);
        self.view
            .restore_status_view(ids!(restore_status_view))
            .set_visible(cx, !self.is_loaded);
    }
}

impl CanNotPreviewScreenRef {
    pub fn set_displayed<S: Into<Option<String>>>(&mut self, cx: &mut Cx, room_id: OwnedRoomId, room_name: S, info: CanNotPreviewDetails) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_displayed(cx, room_id, room_name, info);
        }
    }
}
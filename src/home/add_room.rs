//! A top-level view for adding (joining) or exploring new rooms and spaces.

use makepad_widgets::*;
use matrix_sdk::RoomState;
use ruma::{IdParseError, MatrixToUri, MatrixUri, OwnedRoomOrAliasId, OwnedServerName, matrix_uri::MatrixId, room::{JoinRuleSummary, RoomType}};

use crate::{app::AppStateAction, home::invite_screen::JoinRoomResultAction, room::{FetchedRoomAvatar, FetchedRoomPreview, RoomPreviewAction}, shared::{avatar::AvatarWidgetRefExt, popup_list::{PopupItem, PopupKind, enqueue_popup_notification}}, sliding_sync::{MatrixRequest, submit_async_request}, utils};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::shared::html_or_plaintext::*;


    // The main view that allows the user to add (join) or explore new rooms/spaces.
    pub AddRoomScreen = {{AddRoomScreen}}<ScrollXYView> {
        width: Fill, height: Fill,
        flow: Down,
        padding: {top: 5, left: 15, right: 15, bottom: 0},

        // show_bg: true
        // draw_bg: {
        //     color: (COLOR_PRIMARY)
        // }

        title = <TitleLabel> {
            flow: RightWrap,
            draw_text: {
                text_style: <TITLE_TEXT>{font_size: 13},
                color: #000
                wrap: Word
            }
            text: "Add/Explore Rooms and Spaces"
            draw_text: {
                text_style: {font_size: 18},
            }
        }
        
        <LineH> { padding: 10, margin: {top: 10, right: 2} }

        <SubsectionLabel> {
            text: "Join an existing room or space:"
        }

        // TODO: support showing/hiding this help with a collapsible widget wrapper
        //       (Accordion widget, once it's added to Makepad upstream)

        help_info = <MessageHtml> {
            padding: 7
            width: Fill, height: Fit
            font_size: 10.
            font_color: #3
            body: "<p>You can enter a room/space address using either:</p>
                <ul>
                  <li> An <i>alias</i>, starting with <code>#</code>, like <code>#robrix:matrix.org</code>.</li>
                  <li> An <i>ID</i>, starting with <code>!</code>, like <code>!moVNEIUPxJZpxRHDUv:matrix.org</code>.</li>
                  <li> A Matrix link, like <code>https:matrix.to/...</code> or <code>matrix:...</code>.</li>
                </ul>
            "
        }

        join_room_view = <View> {
            width: Fill,
            height: Fit,
            margin: { top: 3 }
            align: {y: 0.5}
            spacing: 5
            flow: Right

            room_alias_id_input = <SimpleTextInput> {
                margin: {top: 0, left: 5, right: 5, bottom: 0},
                width: Fill { max: 400 } // same width as the above `help_info`
                height: Fit
                empty_text: "Enter alias, ID, or Matrix link..."
            }

            search_for_room_button = <RobrixIconButton> {
                padding: {top: 10, bottom: 10, left: 12, right: 14}
                height: Fit
                margin: { bottom: 4 },
                draw_bg: {
                    color: (COLOR_ACTIVE_PRIMARY)
                }
                draw_icon: {
                    svg_file: (ICON_SEARCH)
                    color: (COLOR_PRIMARY)
                }
                draw_text: {
                    color: (COLOR_PRIMARY)
                    text_style: <REGULAR_TEXT> {}
                }
                icon_walk: {width: 16, height: 16}
                text: "Go"
            }
        }

        loading_room_view = <View> {
            visible: false
            spacing: 5,
            padding: 10,
            width: Fill
            height: Fit
            align: {y: 0.5}
            flow: Right

            loading_spinner = <LoadingSpinner> {
                width: 25,
                height: 25,
                draw_bg: {
                    color: (COLOR_ACTIVE_PRIMARY)
                    border_size: 3.0,
                }
            }

            loading_text = <Label> {
                width: Fill, height: Fit
                flow: RightWrap,
                draw_text: {
                    wrap: Line,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                }
            }
        }

        error_view = <View> {
            padding: 10
            error_text = <Label> {
                width: Fill, height: Fit
                flow: RightWrap,
                draw_text: {
                    wrap: Line,
                    color: (COLOR_FG_DANGER_RED),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                }
            }
        }

        fetched_room_summary = <RoundedView> {
            visible: false
            padding: 15
            margin: {top: 10, bottom: 5, left: 5, right: 5}
            flow: Down
            width: Fill, height: Fit

            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY)
                border_radius: 4.0,
                border_size: 1.0
                border_color: (COLOR_BG_DISABLED)
                // shadow_color: #0005
                // shadow_radius: 15.0
                // shadow_offset: vec2(1.0, 0.0), //5.0,5.0)
            }

            room_name_avatar_view = <View> {
                width: Fill, height: Fit
                spacing: 10
                align: {y: 0.5}
                flow: Right,

                room_avatar = <Avatar> {
                    width: 45, height: 45,
                    cursor: Default,
                    text_view = { text = { draw_text: {
                        text_style: <TITLE_TEXT>{ font_size: 15.0 }
                    }}}
                }

                room_name = <Label> {
                    width: Fill, height: Fit,
                    flow: RightWrap,
                    draw_text: {
                        text_style: <TITLE_TEXT>{ font_size: 15 }
                        color: (COLOR_TEXT)
                        wrap: Word,
                    }
                }
            }

            // Something like "This is a [regular|direct] [room|space] with N members."
            room_summary = <Label> {
                width: Fill, height: Fit
                flow: RightWrap,
                margin: {top: 10}
                draw_text: {
                    wrap: Line,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                }
            }

            subsection_alias_id = <SubsectionLabel> {
                draw_text: { text_style: { font_size: 12 } }
            }

            room_alias_and_id_view = <View> {
                padding: {left: 15}
                width: Fill, height: Fit
                spacing: 8.4 // to line up the colons if the ID wraps to the next line
                align: {y: 0.5}
                flow: RightWrap,

                room_alias = <Label> {
                    width: Fit, height: Fit
                    flow: RightWrap,
                    draw_text: {
                        wrap: Line,
                        color: (MESSAGE_TEXT_COLOR),
                        text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                    }
                }

                room_id = <Label> {
                    width: Fit, height: Fit
                    flow: RightWrap,
                    draw_text: {
                        wrap: Line,
                        color: (SMALL_STATE_TEXT_COLOR),
                        text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                    }
                }
            }

            subsection_topic = <SubsectionLabel> {
                draw_text: { text_style: { font_size: 12 } }
            }

            room_topic = <MessageHtml> {
                padding: {left: 20, top: 5, right: 10, bottom: 10}
                width: Fill,
                height: Fit { max: 200 }
                font_size: 11
                font_color: (MESSAGE_TEXT_COLOR)
            }

            buttons_view = <View> {
                width: Fill
                height: Fit,
                flow: RightWrap,
                align: {y: 0.5}
                spacing: 15
                margin: {top: 15}

                // This button's text is based on the room state (e.g., joined, left, invited)
                // the room's join rules (e.g., public, can knock, invite-only (in which we disable it)).
                join_room_button = <RobrixIconButton> {
                    padding: 15,
                    draw_icon: {
                        svg_file: (ICON_JOIN_ROOM),
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                    icon_walk: {width: 17, height: 17, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_FG_ACCEPT_GREEN),
                        color: #f0fff0 // light green
                    }
                    draw_text: {
                        color: (COLOR_FG_ACCEPT_GREEN),
                    }
                }

                cancel_button = <RobrixIconButton> {
                    align: {x: 0.5, y: 0.5}
                    padding: 15
                    draw_icon: {
                        svg_file: (ICON_FORBIDDEN)
                        color: (COLOR_FG_DANGER_RED),
                    }
                    icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                    draw_bg: {
                        border_color: (COLOR_FG_DANGER_RED),
                        color: (COLOR_BG_DANGER_RED)
                    }
                    text: "Cancel"
                    draw_text:{
                        color: (COLOR_FG_DANGER_RED),
                    }
                }
            }
        }
        
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct AddRoomScreen {
    #[deref] view: View,
    #[rust] state: AddRoomState,
    /// The function to perform when the user clicks the `join_room_button`.
    #[rust(JoinButtonFunction::None)] join_function: JoinButtonFunction,
}

#[derive(Default)]
#[allow(clippy::large_enum_variant)]
enum AddRoomState {
    /// We're waiting for the user to input a room ID, alias, or matrix link.
    #[default]
    WaitingOnUserInput,
    /// We successfully parsed the user input and have sent a request.
    /// We're now waiting for the room preview to be fetched and returned.
    Parsed {
        room_or_alias_id: OwnedRoomOrAliasId,
        via: Vec<OwnedServerName>,
    },
    /// The user entered invalid input that we couldn't parse into a room address.
    ParseError(String),
    /// We successfully fetched the room preview and have displayed it,
    /// and are waiting for the user to join the room.
    FetchedRoomPreview {
        frp: FetchedRoomPreview,
        room_or_alias_id: OwnedRoomOrAliasId,
        via: Vec<OwnedServerName>,
    },
    /// We failed to fetch the room preview, likely because it couldn't be found
    /// or because of connectivity issues or something else.
    FetchError(String),
    /// We successfully knocked on the room or space, and are waiting for
    /// a member of that room/space to acknowledge our knock.
    Knocked {
        frp: FetchedRoomPreview,
    },
    /// We successfully joined the room or space, and are waiting for it
    /// to be loaded from the homeserver.
    Joined {
        frp: FetchedRoomPreview,
    },
}
impl AddRoomState {
    fn transition_to_knocked(&mut self) {
        let prev = std::mem::take(self);
        if let Self::FetchedRoomPreview { frp, .. } = prev {
            *self = Self::Knocked { frp };
        } else {
            *self = prev;
        }
    }

    fn transition_to_joined(&mut self) {
        let prev = std::mem::take(self);
        if let Self::FetchedRoomPreview { frp, .. } = prev {
            *self = Self::Joined { frp };
        } else {
            *self = prev;
        }
    }
}

impl Widget for AddRoomScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        
        if let Event::Actions(actions) = event {
            let room_alias_id_input = self.view.text_input(ids!(room_alias_id_input));
            let search_for_room_button = self.view.button(ids!(search_for_room_button));
            let cancel_button = self.view.button(ids!(fetched_room_summary.buttons_view.cancel_button));
            let join_room_button = self.view.button(ids!(fetched_room_summary.buttons_view.join_room_button));

            // Enable or disable the button based on if the text input is empty.
            if let Some(text) = room_alias_id_input.changed(actions) {
                search_for_room_button.set_enabled(cx, !text.trim().is_empty());
            }

            // If the cancel button was clicked, hide the room preview and return to default state.
            if cancel_button.clicked(actions) {
                self.state = AddRoomState::WaitingOnUserInput;
                room_alias_id_input.set_text(cx, "");
                self.redraw(cx);
            }

            // If the join button was clicked, perform the appropriate action.
            if join_room_button.clicked(actions) {
                match (&self.join_function, &self.state) {
                    (JoinButtonFunction::NavigateOrJoin, AddRoomState::FetchedRoomPreview { frp, .. }) => {
                        cx.action(AppStateAction::NavigateToRoom {
                            room_to_close: None,
                            destination_room: frp.clone().into(),
                        });
                    }
                    (JoinButtonFunction::Knock, AddRoomState::FetchedRoomPreview { frp, room_or_alias_id, via }) => {
                        submit_async_request(MatrixRequest::Knock {
                            room_or_alias_id: frp.canonical_alias.clone().map_or_else(
                                || room_or_alias_id.clone(),
                                Into::into
                            ),
                            reason: None,
                            server_names: via.clone(),
                        });
                    }
                    _ => {
                        error!("BUG: shouldn't be able to press join button with no action set.");
                    }
                }
            }

            // If the button was clicked or enter was pressed, try to parse the room address.
            let new_room_query = search_for_room_button.clicked(actions)
                .then(|| room_alias_id_input.text())
                .or_else(|| room_alias_id_input.returned(actions).map(|(t, _)| t));
            if let Some(t) = new_room_query {
                match parse_address(t.trim()) {
                    Ok((room_or_alias_id, via)) => {
                        self.state = AddRoomState::Parsed {
                            room_or_alias_id: room_or_alias_id.clone(),
                            via: via.clone(),
                        };
                        submit_async_request(MatrixRequest::GetRoomPreview { room_or_alias_id, via });
                    }
                    Err(e) => {
                        let err_str = format!("Could not parse the text as a valid room address.\nError: {e}.");
                        enqueue_popup_notification(PopupItem {
                            message: err_str.clone(),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Error,
                        });
                        self.state = AddRoomState::ParseError(err_str);
                    }
                }
                self.redraw(cx);
            }

            // If we're waiting for the room preview to be fetched (i.e., in the Parsed state),
            // then check if we've received it via an action.
            if let AddRoomState::Parsed { room_or_alias_id, via } = &self.state {
                for action in actions {
                    match action.downcast_ref() {
                        Some(RoomPreviewAction::Fetched(Ok(frp))) => {
                            let room_or_alias_id = room_or_alias_id.clone();
                            let via = via.clone();
                            self.state = AddRoomState::FetchedRoomPreview {
                                frp: frp.clone(),
                                room_or_alias_id,
                                via,
                            };
                            self.redraw(cx);
                            break;
                        }
                        Some(RoomPreviewAction::Fetched(Err(e))) => {
                            let err_str = format!("Failed to fetch room info.\n\nError: {e}.");
                            enqueue_popup_notification(PopupItem {
                                message: err_str.clone(),
                                auto_dismissal_duration: None,
                                kind: PopupKind::Error,
                            });
                            self.state = AddRoomState::FetchError(err_str);
                            self.redraw(cx);
                            break;
                        }
                        _ => {}
                    }
                }
            }


            // If we've fetched and displayed the room preview, handle any responses to
            // the user clicking the join button (e.g., knocked on or joined the room/space).
            let mut transition_to_knocked = false;
            let mut transition_to_joined  = false;
            if let AddRoomState::FetchedRoomPreview { frp, room_or_alias_id, .. } = &self.state {
                for action in actions {
                    match action.downcast_ref() {
                        Some(KnockResultAction::Knocked { room, .. }) if room.room_id() == frp.room_name_id.room_id() => {
                            let room_type = match room.room_type() {
                                Some(RoomType::Space) => "space",
                                _ => "room",
                            };
                            enqueue_popup_notification(PopupItem {
                                message: format!("Successfully knocked on {room_type} {}.", frp.room_name_id),
                                auto_dismissal_duration: Some(4.0),
                                kind: PopupKind::Success,
                            });
                            transition_to_knocked = true;
                            break;
                        }
                        Some(KnockResultAction::Failed { error, room_or_alias_id: roai }) if room_or_alias_id == roai => {
                            enqueue_popup_notification(PopupItem {
                                message: format!("Failed to knock on room.\n\nError: {error}."),
                                auto_dismissal_duration: None,
                                kind: PopupKind::Error,
                            });
                            break;
                        }
                        _ => { }
                    }

                    match action.downcast_ref() {
                        Some(JoinRoomResultAction::Joined { room_id }) if room_id == frp.room_name_id.room_id() => {
                            let room_type = match &frp.room_type {
                                Some(RoomType::Space) => "space",
                                _ => "room",
                            };
                            enqueue_popup_notification(PopupItem {
                                message: format!("Successfully joined {room_type} {}.", frp.room_name_id),
                                auto_dismissal_duration: Some(4.0),
                                kind: PopupKind::Success,
                            });
                            transition_to_joined = true;
                            break;
                        }
                        Some(JoinRoomResultAction::Failed { room_id, error }) if room_id == frp.room_name_id.room_id() => {
                            enqueue_popup_notification(PopupItem {
                                message: format!("Failed to join room.\n\nError: {error}."),
                                auto_dismissal_duration: None,
                                kind: PopupKind::Error,
                            });
                            break;
                        }
                        _ => {}
                    }
                }
            }
            if transition_to_knocked { self.state.transition_to_knocked(); }
            if transition_to_joined { self.state.transition_to_joined(); }

        }
    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let loading_room_view = self.view.view(ids!(loading_room_view));
        let fetched_room_summary = self.view.view(ids!(fetched_room_summary));
        let error_view = self.view.view(ids!(error_view));

        match &self.state {
            AddRoomState::WaitingOnUserInput => {
                loading_room_view.set_visible(cx, false);
                fetched_room_summary.set_visible(cx, false);
                error_view.set_visible(cx, false);
            }
            AddRoomState::ParseError(err_str) | AddRoomState::FetchError(err_str) => {
                loading_room_view.set_visible(cx, false);
                fetched_room_summary.set_visible(cx, false); 
                error_view.set_visible(cx, true);
                error_view.label(ids!(error_text)).set_text(cx, err_str);
            }
            AddRoomState::Parsed { room_or_alias_id, .. } => {
                loading_room_view.set_visible(cx, true);
                loading_room_view.label(ids!(loading_text)).set_text(
                    cx,
                    &format!("Fetching {room_or_alias_id}..."),
                );
                fetched_room_summary.set_visible(cx, false); 
                error_view.set_visible(cx, false);
            }
            ars @ AddRoomState::FetchedRoomPreview { frp, .. } 
            | ars @ AddRoomState::Knocked { frp }
            | ars @ AddRoomState::Joined { frp } => {
                loading_room_view.set_visible(cx, false);
                fetched_room_summary.set_visible(cx, true);
                error_view.set_visible(cx, false);

                // Populate the content of the fetched room preview.
                let room_avatar = fetched_room_summary.avatar(ids!(room_avatar));
                match &frp.room_avatar {
                    FetchedRoomAvatar::Text(text) => {
                        room_avatar.show_text(cx, None, None, text);
                    }
                    FetchedRoomAvatar::Image(image_data) => {
                        let res = room_avatar.show_image(
                            cx,
                            None,
                            |cx, img_ref| utils::load_png_or_jpg(&img_ref, cx, image_data),
                        );
                        if res.is_err() {
                            room_avatar.show_text(
                                cx,
                                None,
                                None,
                                frp.room_name_id.name_for_avatar().as_deref().unwrap_or("?"),
                            );
                        }
                    }
                }

                let (room_or_space_lc, room_or_space_uc) = match &frp.room_type {
                    Some(RoomType::Space) => ("space", "Space"),
                    _ => ("room", "Room"),
                };
                let room_name = fetched_room_summary.label(ids!(room_name));
                match frp.room_name_id.name_for_avatar().as_deref() {
                    Some(n) => room_name.set_text(cx, n),
                    _ => room_name.set_text(cx, &format!("Unnamed {room_or_space_uc}, ID: {}", frp.room_name_id.room_id())),
                }

                fetched_room_summary.label(ids!(subsection_alias_id)).set_text(
                    cx,
                    &format!("Main {room_or_space_uc} Alias and ID"),
                );
                fetched_room_summary.label(ids!(room_alias)).set_text(
                    cx,
                    &format!("Alias: {}", frp.canonical_alias.as_ref().map_or("not set", |a| a.as_str())),
                );
                fetched_room_summary.label(ids!(room_id)).set_text(
                    cx,
                    &format!("ID: {}", frp.room_name_id.room_id().as_str()),
                );
                fetched_room_summary.label(ids!(subsection_topic)).set_text(
                    cx,
                    &format!("{room_or_space_uc} Topic"),
                );
                fetched_room_summary.html(ids!(room_topic)).set_text(
                    cx,
                    frp.topic.as_deref().unwrap_or("<i>No topic set</i>"),
                );

                let room_summary = fetched_room_summary.label(ids!(room_summary));
                let join_room_button = fetched_room_summary.button(ids!(join_room_button));
                let join_function = match (&frp.state, &frp.join_rule) {
                    (Some(RoomState::Joined), _) => {
                        room_summary.set_text(cx, &format!("You have already joined this {room_or_space_lc}."));
                        join_room_button.set_text(cx, &format!("Go to {room_or_space_lc}"));
                        JoinButtonFunction::NavigateOrJoin
                    }
                    (Some(RoomState::Banned), _) => {
                        room_summary.set_text(cx, &format!("You have been banned from this {room_or_space_lc}."));
                        join_room_button.set_text(cx, "Cannot join until un-banned");
                        JoinButtonFunction::None
                    }
                    (Some(RoomState::Invited), _) => {
                        room_summary.set_text(cx, &format!("You have already been invited to this {room_or_space_lc}."));
                        join_room_button.set_text(cx, "Go to invitation");
                        JoinButtonFunction::NavigateOrJoin
                    }
                    (Some(RoomState::Knocked), _) => {
                        room_summary.set_text(cx, &format!("You have already knocked on this {room_or_space_lc}."));
                        join_room_button.set_text(cx, "Knock again (be nice!)");
                        JoinButtonFunction::Knock
                    }
                    (Some(RoomState::Left), join_rule) => {
                        room_summary.set_text(cx, &format!("You previously left this {room_or_space_lc}."));
                        let (join_room_text, join_function) = match join_rule {
                            Some(JoinRuleSummary::Public) => (
                                format!("Re-join this {room_or_space_lc}"),
                                JoinButtonFunction::NavigateOrJoin,
                            ),
                            Some(JoinRuleSummary::Invite) => (
                                format!("Re-joining {room_or_space_lc} requires an invite"),
                                JoinButtonFunction::None,
                            ),
                            Some(JoinRuleSummary::Knock | JoinRuleSummary::KnockRestricted(_)) => (
                                format!("Knock to re-join {room_or_space_lc}"),
                                JoinButtonFunction::Knock,
                            ),
                            // TODO: handle this after we update matrix-sdk to the new `JoinRule` enum.
                            Some(JoinRuleSummary::Restricted(_)) => (
                                format!("Re-joining {room_or_space_lc} requires an invite or other room membership"),
                                JoinButtonFunction::None,
                            ),
                            _ => (
                                format!("Not allowed to re-join this {room_or_space_lc}"),
                                JoinButtonFunction::None,
                            ),
                        };
                        join_room_button.set_text(cx, &join_room_text);
                        join_function
                    }
                    // This room is not yet known to the user.
                    (None, join_rule) => {
                        let direct = if frp.is_direct == Some(true) { "direct" } else { "regular" }; 
                        room_summary.set_text(cx, &format!(
                            "This is a {direct} {room_or_space_lc} with {} members.",
                            frp.num_joined_members,
                        ));

                        let (join_room_text, join_function) = match join_rule {
                            Some(JoinRuleSummary::Public) => (
                                format!("Join this {room_or_space_lc}"),
                                JoinButtonFunction::NavigateOrJoin,
                            ),
                            Some(JoinRuleSummary::Invite) => (
                                format!("Joining {room_or_space_lc} requires an invite"),
                                JoinButtonFunction::None,
                            ),
                            Some(JoinRuleSummary::Knock | JoinRuleSummary::KnockRestricted(_)) => (
                                format!("Knock to join {room_or_space_lc}"),
                                JoinButtonFunction::Knock,
                            ),
                            // TODO: handle this after we update matrix-sdk to the new `JoinRule` enum.
                            Some(JoinRuleSummary::Restricted(_)) => (
                                format!("Joining {room_or_space_lc} requires an invite or other room membership"),
                                JoinButtonFunction::None,
                            ),
                            _ => ( 
                                format!("Not allowed to join this {room_or_space_lc}"),
                                JoinButtonFunction::None,
                            ),
                        };
                        join_room_button.set_text(cx, &join_room_text);
                        join_function
                    }
                };

                match ars {
                    AddRoomState::FetchedRoomPreview { .. } => {
                        join_room_button.set_enabled(cx, !matches!(join_function, JoinButtonFunction::None));
                        self.join_function = join_function;
                        join_room_button.reset_hover(cx);
                        fetched_room_summary.button(ids!(cancel_button)).reset_hover(cx);
                    }
                    AddRoomState::Knocked { .. } => {
                        join_room_button.set_text(cx, "Successfully knocked!");
                        join_room_button.set_enabled(cx, false);
                    }
                    AddRoomState::Joined { .. } => {
                        join_room_button.set_text(cx, "Successfully joined!");
                        join_room_button.set_enabled(cx, false);
                    }
                    _ => {}
                }
            }
        }

        self.view.draw_walk(cx, scope, walk)
    }
}


/// The function to perform when the user clicks the join button in the fetched room preview.
enum JoinButtonFunction {
    None,
    /// Navigate to an already-known room/space, or join it if possible.
    NavigateOrJoin,
    /// Knock on (request to join) a room/space.
    Knock,
}
 

/// Actions sent from the backend task as a result of a [`MatrixRequest::Knock`].
#[derive(Debug)]
pub enum KnockResultAction {
    /// The user successfully knocked on the room/space.
    Knocked {
        /// The room alias/ID that was originally sent with the knock request.
        room_or_alias_id: OwnedRoomOrAliasId,
        /// The room that was knocked on.
        room: matrix_sdk::Room,
    },
    /// There was an error attempting to knock on the room.
    Failed {
        /// The room alias/ID that was originally sent with the knock request.
        room_or_alias_id: OwnedRoomOrAliasId,
        error: matrix_sdk::Error,
    }
}


/// Tries to extract a room address (Alias or ID) from the given text.
///
/// This function is quite flexible and will attempt to parse `text` as:
/// * A Room ID (with a leading `!`).
/// * A Room Alias (with a leading `#`).
/// * A `https://matrix.to` URI, which includes either a room alias, or a room ID plus `via` servers.
/// * A `matrix:` scheme URI, which is similar to above.
fn parse_address(text: &str) -> Result<(OwnedRoomOrAliasId, Vec<OwnedServerName>), IdParseError> {
    match OwnedRoomOrAliasId::try_from(text) {
        Ok(room_or_alias_id) => Ok((room_or_alias_id, Vec::new())),
        Err(e) => {
            let uri_result = MatrixToUri::parse(text)
                .map(|uri| (uri.id().clone(), uri.via().to_owned()))
                .or_else(|_| MatrixUri::parse(text).map(|uri| (uri.id().clone(), uri.via().to_owned())));
            
            if let Ok((matrix_id, via)) = uri_result {
                if let Some(room_or_alias_id) = match matrix_id {
                    MatrixId::Room(room_id) => Some(room_id.into()),
                    MatrixId::RoomAlias(alias) => Some(alias.into()),
                    MatrixId::Event(room_or_alias_id, _) => Some(room_or_alias_id),
                    _ => None,
                } {
                    return Ok((room_or_alias_id, via));
                }
            }
            Err(e)
        }
    }    
}

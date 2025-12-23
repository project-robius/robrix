use crate::home::room_screen::RoomScreenTooltipActions;
use crate::profile::user_profile_cache::get_user_profile_and_room_member;
use crate::sliding_sync::{current_user_id, submit_async_request, MatrixRequest};
use indexmap::IndexMap;
use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use matrix_sdk_ui::timeline::{ReactionInfo, ReactionsByKeyBySender, TimelineEventItemId};

const EMOJI_BORDER_COLOR_INCLUDE_SELF: Vec4 = Vec4 {
    x: 0.0,
    y: 0.6,
    z: 0.47,
    w: 1.0,
}; // DarkGreen
const EMOJI_BORDER_COLOR_NOT_INCLUDE_SELF: Vec4 = Vec4 {
    x: 0.714,
    y: 0.73,
    z: 0.75,
    w: 1.0,
}; // Grey

const EMOJI_BG_COLOR_INCLUDE_SELF: Vec4 = Vec4 {
    x: 0.89,
    y: 0.967,
    z: 0.929,
    w: 1.0,
}; // LightGreen
const EMOJI_BG_COLOR_NOT_INCLUDE_SELF: Vec4 = Vec4 {
    x: 0.968,
    y: 0.976,
    z: 0.98,
    w: 1.0,
}; // LightGrey

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    COLOR_BUTTON_GREY = #B6BABF
    REACTION_LIST_PADDING_RIGHT = 30.0;

    pub ReactionList = {{ReactionList}} {
        width: Fill,
        height: Fit,
        flow: RightWrap,
        margin: {top: 5.0}
        padding:{
            right: (REACTION_LIST_PADDING_RIGHT)
        }
        item: <Button> {
            width: Fit,
            height: Fit,
            padding: 6,
            // Use a zero margin on the left because we want the first reaction
            // to be flush with the left edge of the message text.
            margin: { top: 3, bottom: 3, left: 0, right: 6 },

            draw_bg: {
                // Anything that we apply over must be an `instance`,
                // and their names must be distinct from the base Button type.
                instance reaction_bg_color: (COLOR_BUTTON_GREY)
                instance reaction_border_color: #001A11
                // Override values from the base Button type.
                color_hover: #fef65b
                hover: 0.0
                border_size: 1.5
                border_radius: 3.0

                fn get_color(self) -> vec4 {
                    return mix(self.reaction_bg_color, mix(self.reaction_bg_color, self.color_hover, 0.2), self.hover)
                }

                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                    sdf.box(
                        self.border_size,
                        self.border_size,
                        self.rect_size.x - self.border_size * 2.0,
                        self.rect_size.y - self.border_size * 2.0,
                        max(1.0, self.border_radius)
                    )
                    sdf.fill_keep(self.get_color())
                    if self.border_size > 0.0 {
                        sdf.stroke(self.reaction_border_color, self.border_size)
                    }
                    return sdf.result;
                }
            }
            draw_text: {
                text_style: <REGULAR_TEXT>{font_size: 9},
                color: #000000
                fn get_color(self) -> vec4 {
                    return self.color;
                }
            }
        }
    }

}
#[derive(Clone, Debug)]
pub struct ReactionData {
    /// Original reaction string from the backend before emoji shortcode conversion.
    pub reaction: String,
    /// Boolean indicating if the current user is also a sender of this reaction.
    pub includes_user: bool,
    /// List of all users who have reacted to the emoji.
    pub reaction_senders: IndexMap<OwnedUserId, ReactionInfo>,
    /// The ID of the room that the reaction is for
    pub room_id: OwnedRoomId,
}

#[derive(Live, LiveHook, Widget)]
pub struct ReactionList {
    #[redraw]
    #[rust]
    area: Area,
    #[live]
    item: Option<LivePtr>,
    #[rust]
    children: Vec<(ButtonRef, ReactionData)>,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    #[rust]
    room_id: Option<OwnedRoomId>,
    #[rust]
    timeline_event_id: Option<TimelineEventItemId>,
}
impl Widget for ReactionList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        for (button, _) in self.children.iter_mut() {
            let _ = button.draw(cx, scope);
        }
        cx.end_turtle();
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        for (button_ref, reaction_data) in self.children.iter() {
            let button_area = button_ref.area();
            // Note: the `break` statements are used to break out of the loop over
            // all reaction buttons, since a hit event can only occur on one button.
            match event.hits(cx, button_area) {
                Hit::FingerDown(..) => {
                    cx.set_key_focus(button_area);
                    break;
                }
                Hit::FingerHoverOver(..) // TODO: remove once CalloutTooltip bug is fixed
                | Hit::FingerHoverIn(..) => {
                    self.do_hover_in(cx, scope, button_ref, reaction_data.clone());
                    break;
                }
                Hit::FingerHoverOut(_) => {
                    self.do_hover_out(cx, scope, button_ref);
                    break;
                }
                // A long press is treated as a hover-in.
                Hit::FingerLongPress(_) => {
                    self.do_hover_in(cx, scope, button_ref, reaction_data.clone());
                    break;
                }
                Hit::FingerUp(fue) => {
                    // If the finger is not over the button, treat it as a hover-out.
                    if !fue.is_over {
                        self.do_hover_out(cx, scope, button_ref);
                    }
                    // Otherwise, a primary click/press over the button should toggle the reaction.
                    else if fue.is_primary_hit() && fue.was_tap() {
                        let Some(room_id) = &self.room_id else { return };
                        let Some(timeline_event_id) = &self.timeline_event_id else {
                            return;
                        };
                        submit_async_request(MatrixRequest::ToggleReaction {
                            room_id: room_id.clone(),
                            timeline_event_id: timeline_event_id.clone(),
                            reaction: reaction_data.reaction.clone(),
                        });
                        // update the reaction button before the timeline is updated
                        let (bg_color, border_color) = if !reaction_data.includes_user {
                            (EMOJI_BG_COLOR_INCLUDE_SELF, EMOJI_BORDER_COLOR_INCLUDE_SELF)
                        } else {
                            (EMOJI_BG_COLOR_NOT_INCLUDE_SELF, EMOJI_BORDER_COLOR_NOT_INCLUDE_SELF)
                        };
                        button_ref.apply_over(cx, live! {
                            draw_bg: { reaction_bg_color: (bg_color) , reaction_border_color: (border_color) }
                        });
                        self.do_hover_out(cx, scope, button_ref);
                    }
                    break;
                }
                Hit::FingerScroll(_) => {
                    self.do_hover_out(cx, scope, button_ref);
                    break;
                }
                _ => {}
            }
        }
    }
}

impl ReactionList {
    /// Deals with to any event/hit that triggers a hover-in action.
    fn do_hover_in(
        &self,
        cx: &mut Cx,
        scope: &mut Scope,
        button_ref: &ButtonRef,
        reaction_data: ReactionData,
    ) {
        cx.widget_action(
            self.widget_uid(),
            &scope.path,
            RoomScreenTooltipActions::HoverInReactionButton {
                widget_rect: button_ref.area().rect(cx),
                reaction_data,
            },
        );
        button_ref.apply_over(cx, live!(draw_bg: {hover: 1.0}));
        cx.set_cursor(MouseCursor::Hand);
    }

    /// Deals with to any event/hit that triggers a hover-out action.
    fn do_hover_out(
        &self,
        cx: &mut Cx,
        scope: &mut Scope,
        button_ref: &ButtonRef,
    ) {
        cx.widget_action(self.widget_uid(), &scope.path, RoomScreenTooltipActions::HoverOut);
        button_ref.apply_over(cx, live!(draw_bg: {hover: 0.0}));
        cx.set_cursor(MouseCursor::Default);
    }
}


impl ReactionListRef {
    /// Set the list of reactions and their counts to display in the ReactionList widget,
    /// along with the room ID and event ID that these reactions are for.
    ///
    /// This will clear any existing list of reactions and replace it with the given one.
    ///
    /// The given `event_tl_item_reactions` is a map from each reaction's raw string (including any variant selectors)
    /// to the list of users who have reacted with that reaction.
    ///
    /// The given `room_id` is the ID of the room that these reactions are for.
    ///
    /// The given `timeline_event_item_id` is the ID of the event that these reactions are for.
    /// Required by Matrix API
    pub fn set_list(
        &mut self,
        cx: &mut Cx,
        event_tl_item_reactions: Option<&ReactionsByKeyBySender>,
        room_id: OwnedRoomId,
        timeline_event_item_id: TimelineEventItemId,
        _id: usize,
    ) {
        let Some(client_user_id) = current_user_id() else {
            return;
        };
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        let Some(event_tl_item_reactions) = event_tl_item_reactions else {
            inner.children.clear();
            return;
        };
        inner.children.clear(); //Inefficient but we don't want to compare the event_tl_item_reactions
        for (reaction_text, reaction_senders) in event_tl_item_reactions.iter() {
            // // Just take the first char of the emoji, which ignores any variant selectors.
            // let reaction_first_char = reaction_text.chars().next().map(|c| c.to_string());
            // let reaction_str = reaction_first_char.as_deref().unwrap_or(reaction_text);
            let mut includes_user: bool = false;
            for (sender, _) in reaction_senders.iter() {
                if sender == &client_user_id {
                    includes_user = true;
                }
                // Cache the reaction sender's user profile so that tooltip will show displayable name
                let _ = get_user_profile_and_room_member(cx, sender.clone(), &room_id, true);
            }

            let reaction_data = ReactionData {
                reaction: reaction_text.to_string(),
                includes_user,
                reaction_senders: reaction_senders.clone(),
                room_id: room_id.clone(),
            };
            let button = WidgetRef::new_from_ptr(cx, inner.item).as_button();
            button.set_text(
                cx,
                &format!(
                    "{}  {}",
                    reaction_data.reaction,
                    reaction_senders.len()
                ),
            );
            let (bg_color, border_color) = if reaction_data.includes_user {
                (EMOJI_BG_COLOR_INCLUDE_SELF, EMOJI_BORDER_COLOR_INCLUDE_SELF)
            } else {
                (
                    EMOJI_BG_COLOR_NOT_INCLUDE_SELF,
                    EMOJI_BORDER_COLOR_NOT_INCLUDE_SELF,
                )
            };
            button.apply_over(
                cx,
                live! {
                    draw_bg: { reaction_bg_color: (bg_color) , reaction_border_color: (border_color) }
                },
            );
            inner.children.push((button, reaction_data));
        }
        inner.room_id = Some(room_id);
        inner.timeline_event_id = Some(timeline_event_item_id);
    }

    /// Returns any `RoomScreenTooltipActions` that occurred in the given list of `actions`.
    ///
    /// This function checks if there is a widget action associated with the current
    /// widget's unique identifier in the provided `actions`.
    /// If an action exists, it is cast to `RoomScreenTooltipActions` and returned.
    /// Otherwise, it returns `RoomScreenTooltipActions::None`.
    pub fn hovered_in(&self, actions: &Actions) -> RoomScreenTooltipActions {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            item.cast()
        } else {
            RoomScreenTooltipActions::None
        }
    }
    /// Returns whether the given `actions` contained a `RoomScreenTooltipActions::HoverOut` action.
    pub fn hovered_out(&self, actions: &Actions) -> bool {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            matches!(item.cast(), RoomScreenTooltipActions::HoverOut)
        } else {
            false
        }
    }
}

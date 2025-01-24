use crate::sliding_sync::{current_user_id, submit_async_request, MatrixRequest};
use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use matrix_sdk_ui::timeline::{ReactionInfo, ReactionsByKeyBySender, TimelineEventItemId};
use crate::profile::user_profile_cache::get_user_profile_and_room_member;
use crate::home::room_screen::RoomScreenTooltipActions;
use indexmap::IndexMap;

use super::room_screen::room_screen_tooltip_position_helper;

const TOOLTIP_WIDTH: f64 = 200.0;
const EMOJI_BORDER_COLOR_INCLUDE_SELF: Vec4 = Vec4 { x: 0.0, y: 0.6, z: 0.47, w: 1.0 }; // DarkGreen
const EMOJI_BORDER_COLOR_NOT_INCLUDE_SELF: Vec4 = Vec4 { x: 0.714, y: 0.73, z: 0.75, w: 1.0 }; // Grey

const EMOJI_BG_COLOR_INCLUDE_SELF: Vec4 = Vec4 { x: 0.89, y: 0.967, z: 0.929, w: 1.0 }; // LightGreen
const EMOJI_BG_COLOR_NOT_INCLUDE_SELF: Vec4 = Vec4 { x: 0.968, y: 0.976, z: 0.98, w: 1.0 }; // LightGrey

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
                instance color: (COLOR_BUTTON_GREY)
                instance color_hover: #fef65b
                instance border_width: 1.5
                instance border_color: #001A11
                instance radius: 3.0
                instance hover: 0.0
                fn get_color(self) -> vec4 {
                    return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
                }

                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                    sdf.box(
                        self.border_width,
                        self.border_width,
                        self.rect_size.x - self.border_width * 2.0,
                        self.rect_size.y - self.border_width * 2.0,
                        max(1.0, self.radius)
                    )
                    sdf.fill_keep(self.get_color())
                    if self.border_width > 0.0 {
                        //let stroke_color = mix(self.get_color(), self.border_color, 0.2);
                        sdf.stroke(self.border_color, self.border_width)
                    }
                    return sdf.result;
                }
            }
            draw_text: {
                text_style: <REGULAR_TEXT>{font_size: 9},
                color: #000
                fn get_color(self) -> vec4 {
                    return self.color;
                }
            }
        }
    }
    
}
#[derive(Clone, Debug)]
pub struct ReactionData {
    /// Refers to an emoji "shortcode" string, which is a temporary hack
    /// because Makepad does not yet support drawing actual emoji.
    pub emoji_shortcode: String,
    /// Original reaction string from the backend before emoji shortcode conversion.
    pub reaction_raw: String,
    /// Boolean indicating if the current user is also a sender of this reaction.
    pub includes_user: bool,
    /// List of all users who have reacted to the emoji.
    pub reaction_senders: IndexMap<OwnedUserId, ReactionInfo>,
    /// The ID of the room that the reaction is for
    pub room_id: OwnedRoomId
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
        self.children.iter_mut().for_each(|(target, _)| {
            let _ = target.draw(cx, scope); 
        });
        cx.end_turtle();
        DrawStep::done()
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid: WidgetUid = self.widget_uid();
        let app_state = scope.data.get::<crate::app::AppState>().unwrap();
        let Some(window_geom) = &app_state.window_geom else { return };
        for (widget_ref, reaction_data) in self.children.iter() {
            match event.hits(cx, widget_ref.area()) {
                Hit::FingerHoverIn(_) => {
                    let widget_rect = widget_ref.area().rect(cx);
                    let (tooltip_pos, 
                        callout_offset, 
                        too_close_to_right, 
                    ) = room_screen_tooltip_position_helper(widget_rect, window_geom, TOOLTIP_WIDTH);
                    cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverInReactionButton {
                        tooltip_pos, 
                        tooltip_width: TOOLTIP_WIDTH, 
                        callout_offset,
                        reaction_data: reaction_data.clone(),
                        pointing_up: too_close_to_right,
                    });
                    cx.set_cursor(MouseCursor::Hand);
                    widget_ref.apply_over(cx, live!(draw_bg: {hover: 1.0}));
                    break;
                }
                Hit::FingerHoverOut(_) => {
                    cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverOut);
                    widget_ref.apply_over(cx, live!(draw_bg: {hover: 0.0}));
                    cx.set_cursor(MouseCursor::Default);
                    break;
                }
                Hit::FingerDown(_) => {
                    let Some(room_id) = &self.room_id else { return };
                    let Some(timeline_event_id) = &self.timeline_event_id else {
                        return;
                    };
                    submit_async_request(MatrixRequest::ToggleReaction {
                        room_id: room_id.clone(),
                        timeline_event_id: timeline_event_id.clone(),
                        reaction: reaction_data.reaction_raw.clone(),
                    });
                    // update the reaction button before the timeline is updated
                    let (bg_color, border_color) = if !reaction_data.includes_user {
                        (EMOJI_BG_COLOR_INCLUDE_SELF, EMOJI_BORDER_COLOR_INCLUDE_SELF)
                    } else {
                        (EMOJI_BG_COLOR_NOT_INCLUDE_SELF, EMOJI_BORDER_COLOR_NOT_INCLUDE_SELF)
                    };
                    widget_ref.apply_over(cx, live! {
                        draw_bg: { color: (bg_color) , border_color: (border_color) }
                    });
                    cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverOut);
                    cx.set_cursor(MouseCursor::Hand);
                    break;
                },
                Hit::FingerUp(_) => {
                    cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverOut);
                    cx.set_cursor(MouseCursor::Hand);
                    break;
                }
                Hit::FingerScroll(_) => {
                    cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverOut);
                    cx.set_cursor(MouseCursor::Default);
                }
                _ => { }
            }
        }
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
        event_tl_item_reactions: &ReactionsByKeyBySender,
        room_id: OwnedRoomId,
        timeline_event_item_id: TimelineEventItemId,
        id: usize,
    ) {
        const DRAW_ITEM_ID_REACTION: bool = false;
        
        let Some(client_user_id) = current_user_id() else { return };
        let Some(mut inner) = self.borrow_mut() else { return };
        if event_tl_item_reactions.is_empty() && !DRAW_ITEM_ID_REACTION {
            inner.children.clear();
            return;
        }
        inner.children.clear(); //Inefficient but we don't want to compare the event_tl_item_reactions
        for (reaction_raw, reaction_senders) in event_tl_item_reactions.iter() {
            // Just take the first char of the emoji, which ignores any variant selectors.
            let reaction_first_char = reaction_raw.chars().next().map(|c| c.to_string());
            let reaction_str = reaction_first_char.as_deref().unwrap_or(reaction_raw);
            let mut includes_user: bool = false;
            let emoji_text = emojis::get(reaction_str)
                .and_then(|e| e.shortcode())
                .unwrap_or(reaction_raw);
            for (sender, _) in reaction_senders.iter() {
                if sender == &client_user_id {
                    includes_user = true;
                }
                // Cache the reaction sender's user profile so that tooltip will show displayable name 
                let _ = get_user_profile_and_room_member(cx, sender.clone(), &room_id, true);
            }
            let mut emoji_text = emoji_text.to_string();

            // Debugging: draw the item ID as a reaction
            if DRAW_ITEM_ID_REACTION {
                emoji_text = format!("{emoji_text}\n ID: {}", id);
            }
            let reaction_data = ReactionData {
                reaction_raw: reaction_raw.to_string(),
                emoji_shortcode: emoji_text.to_string(),
                includes_user,
                reaction_senders: reaction_senders.clone(),
                room_id: room_id.clone(),
            };
            let button = WidgetRef::new_from_ptr(cx, inner.item).as_button();
            button.set_text(
                cx,
                &format!("{}  {}", reaction_data.emoji_shortcode, reaction_senders.len()),
            );
            let (bg_color, border_color) = if reaction_data.includes_user {
                (EMOJI_BG_COLOR_INCLUDE_SELF, EMOJI_BORDER_COLOR_INCLUDE_SELF)
            } else {
                (EMOJI_BG_COLOR_NOT_INCLUDE_SELF, EMOJI_BORDER_COLOR_NOT_INCLUDE_SELF)
            };
            button.apply_over(cx, live! {
                draw_bg: { color: (bg_color) , border_color: (border_color) }
            });
            inner.children.push((button, reaction_data));
        }
        inner.room_id = Some(room_id);
        inner.timeline_event_id = Some(timeline_event_item_id);
    }


    /// Handles hover in action and returns the appropriate `RoomScreenTooltipActions`.
    /// 
    /// This function checks if there is a widget action associated with the current
    /// widget's unique identifier in the provided `actions`. If an action exists,
    /// it is cast to `RoomScreenTooltipActions` and returned. Otherwise, it returns
    /// `RoomScreenTooltipActions::None`.
    ///
    /// # Arguments
    ///
    /// * `actions` - A reference to the `Actions` that may contain widget actions
    ///   relevant to this widget.
    pub fn hover_in(&self, actions: &Actions) -> RoomScreenTooltipActions {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            item.cast()
        } else {
            RoomScreenTooltipActions::None
        }
    }
    /// Handles widget actions and returns `true` if the hover out action was found in the provided `actions`.
    /// Otherwise, returns `false`.
    pub fn hover_out(&self, actions: &Actions) -> bool {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            matches!(item.cast(), RoomScreenTooltipActions::HoverOut)
        } else {
            false
        }
    }
}
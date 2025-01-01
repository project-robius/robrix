use crate::sliding_sync::{get_client, submit_async_request, MatrixRequest};
use crate::utils::human_readable_list;
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk_ui::timeline::{ReactionsByKeyBySender, TimelineEventItemId};
use crate::profile::user_profile_cache::get_user_profile_and_room_member;
use crate::home::room_screen::RoomScreenTooltipActions;

const TOOLTIP_WIDTH: f64 = 100.0;
const EMOJI_BG_COLOR_INCLUDE_SELF: Vec4 = Vec4 { x: 0.0, y: 0.6, z: 0.47, w: 1.0 }; // DarkGreen
const EMOJI_BG_COLOR_NOT_INCLUDE_SELF: Vec4 = Vec4 { x: 0.714, y: 0.73, z: 0.75, w: 1.0 }; // Grey

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
        margin: {top: (5.0)}
        padding:{
            right: (REACTION_LIST_PADDING_RIGHT)
        }
        item: <Button> {
            width: Fit,
            height: Fit,
            spacing: 20,
            padding: 6,
            margin: {
                top:3,
                bottom:3,
                left:3,
                right:3

            },
            draw_bg: {
                instance color: (COLOR_BUTTON_GREY)
                instance color_hover: (#fef65b)
                instance border_width: 1.5
                instance border_color: (#001A11)
                instance radius: 3.0
                // The first draw is to get the width of the button, so that we can use it in the second draw
                // If hide >= 0.5, the button is hidden.
                // Without hiding, the buttons layout may appear glitched at the start
                instance hide: 0.0
                fn get_color(self) -> vec4 {
                    return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
                }

                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                    if self.hide >= 0.5 {
                        return sdf.result;
                    }
                    sdf.box(
                        self.border_width,
                        self.border_width,
                        self.rect_size.x - (self.border_width * 2.0),
                        self.rect_size.y - (self.border_width * 2.0),
                        max(1.0, self.radius)
                    )
                    sdf.fill_keep(self.get_color())
                    if self.border_width > 0.0 {
                        let stroke_color = mix(self.get_color(), self.border_color, 0.2);
                        sdf.stroke(stroke_color, self.border_width)
                    }
                    return sdf.result;
                }
            }
            draw_text: {
                text_style: <REGULAR_TEXT>{font_size: 8},
                color: #000
                fn get_color(self) -> vec4 {
                    return self.color;
                }
            }
        }
    }
    
}
struct ReactionData {
    emoji: String,
    /// Total number of people reacted to the emoji
    total_num_react: usize,
    /// Tooltip text display when mouse over the reaction button
    tooltip_text: String,
    /// Boolean indicating if the current user is also a sender of the reaction
    includes_user: bool,
    /// Calculated of the width of the reaction button
    width: f64,
}

#[derive(Live, LiveHook, Widget)]
pub struct ReactionList {
    #[redraw]
    #[rust]
    area: Area,
    #[live]
    item: Option<LivePtr>,
    #[rust]
    children: ComponentMap<LiveId, ButtonRef>,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    /// A list of ReactionData which includes data required to draw the reaction buttons and their tooltips.
    /// After the first draw, the button widths will be stored in this vector
    #[rust]
    event_reaction_list: Vec<ReactionData>,
    #[rust]
    room_id: Option<OwnedRoomId>,
    #[rust]
    timeline_event_id: Option<TimelineEventItemId>,
    /// Has the width of the emoji buttons already been drawn and calculated beforehand?
    #[rust]
    width_calculated: bool,
    /// Tooltip that appears when hovering over a reaction button, (Index in event_reaction_list, tooltip rendering rectangle's area, tooltip's text, callout's y offset)
    #[rust]
    tooltip_state: Option<(u64, RoomScreenTooltipActions)>
}
impl Widget for ReactionList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        let width: f64 = rect.size.x;
        if !self.width_calculated {
            // Records the button widths after the first draw
            let mut prev_width: f64 = 0.0;
            for (index, reaction_data) in
                self.event_reaction_list.iter_mut().enumerate()
            {
                let target = self.children.get_or_insert(cx, LiveId(index as u64), |cx| {
                    WidgetRef::new_from_ptr(cx, self.item).as_button()
                });
                target.set_text(&format!("{} {}", reaction_data.emoji, reaction_data.total_num_react));
                // Hide the button until the first draw
                target.apply_over(
                    cx,
                    live! {
                        draw_bg: { hide: 1.0 }
                    },
                );
                let _ = target.draw(cx, scope);
                let used = cx.turtle().used();
                reaction_data.width = used.x - prev_width;
                prev_width = used.x;
            }
    
            self.width_calculated = true;
        } else {
            // With the width calculated from the first draw, 
            let mut acc_width: f64 = 0.0;
            for (index, reaction_data) in
                self.event_reaction_list.iter().enumerate()
            {
                let target = self.children.get_or_insert(cx, LiveId(index as u64), |cx| {
                    WidgetRef::new_from_ptr(cx, self.item).as_button()
                });
                target.set_text(&format!("{} {}", reaction_data.emoji, reaction_data.total_num_react));
                // Renders Green button for reaction that includes the client user
                // Renders Grey button for reaction that does not include client user
                let node_to_apply = if reaction_data.includes_user {
                    live! {
                        draw_bg: { hide: 0.0 , color: (EMOJI_BG_COLOR_INCLUDE_SELF) }
                    }
                } else {
                    live! {
                        draw_bg: { hide: 0.0, color: (EMOJI_BG_COLOR_NOT_INCLUDE_SELF) }
                    }
                };
                // Unhide the button as we have the width of the buttons
                target.apply_over(
                    cx,
                    node_to_apply
                );
                acc_width += reaction_data.width;
                // Creates a new line if the accumulated width exceeds the available space
                if acc_width > width {
                    cx.turtle_new_line();
                    acc_width = reaction_data.width;
                    let used: DVec2 = cx.turtle().used();
                    // Resets the turtle's width after each new line
                    cx.turtle_mut().set_used(0.0, used.y);
                }
                let _ = target.draw(cx, scope);                
            }
        }
    
        cx.end_turtle();
        self.children.retain_visible();
        DrawStep::done()
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let Some(room_id) = &self.room_id else { return };
        let Some(timeline_event_id) = &self.timeline_event_id else {
            return;
        };
        // Apply mouse-in tooltip effect on the reaction buttons
        // Currently handling mouse-in effect using "event.hits(cx, widget_ref.area())" does not work.
        if let Event::MouseMove(e) = event {
            let uid = self.widget_uid();
            if self.tooltip_state.is_none() {
                for (id, widget_ref) in self.children.iter() {
                    // Widget.handle_event here does not cause the button to be highlighted when mouse over
                    // To make the button highlighted when mouse over, the iteration over the children needs to be done 
                    // outside Event::MouseMove.
                    let widget_rect = widget_ref.area().rect(cx);
                    if widget_rect.contains(e.abs) {
                        if let Some(reaction_data) = self.event_reaction_list.get(id.0 as usize) {
                            let tooltip_pos =  DVec2 {
                                x: widget_rect.pos.x + widget_rect.size.x,
                                y: widget_rect.pos.y - widget_rect.size.y / 2.0
                            };
                            // Stores the event_reaction_list index together with the tooltip area and tooltip text into tooltip state
                            // The index will be used later to reset the tooltip state if the mouse leaves this particular reaction button
                            self.tooltip_state = Some((id.0, RoomScreenTooltipActions::HoverIn {
                                tooltip_pos, 
                                tooltip_text: reaction_data.tooltip_text.clone(), 
                                tooltip_width: TOOLTIP_WIDTH, 
                                callout_y_offset: (widget_rect.size.y - 5.0) / 2.0 + 10.0
                            }));
                        }
                    }
                }
            } else {
                let mut reset_tooltip_state = false;
                if let Some((ref index, hover_in_data)) = &self.tooltip_state {
                    self.children
                    .iter()
                    .for_each(|(id, widget_ref)| {
                        // Search for the children with the same index as the tooltip state and check if the mouse leaves this particular reaction button
                        // If so, post a HoverOut action to make the tooltip disable
                        if id.0 != *index {
                            return;
                        }
                        if !widget_ref.area().rect(cx).contains(e.abs) {
                            if self.event_reaction_list.get(id.0 as usize).is_some() {
                                reset_tooltip_state = true;
                                cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverOut);
                            }
                        }
                    });
                    // If the mouse does not leave this particular reaction button, post a HoverIn action
                    if !reset_tooltip_state {
                        cx.widget_action(uid, &scope.path, hover_in_data.clone());
                    }
                }
                if reset_tooltip_state {
                    self.tooltip_state = None;
                }
            } 
        }
        if let Event::Actions(actions) = event {
            self.children
            .iter()
            .for_each(|(_id, widget_ref)| {
                if widget_ref.clicked(actions) {
                    let text = widget_ref.text().clone();
                    let reaction_string = text.rsplit_once(' ')
                    .map(|(prefix, _)| prefix)
                    .unwrap_or(&text);
                    if let Some(key) = emojis::get_by_shortcode(reaction_string) {
                        submit_async_request(MatrixRequest::ToggleReaction {
                            room_id: room_id.clone(),
                            timeline_event_id: timeline_event_id.clone(),
                            reaction: key.as_str().to_string(),
                        });
                    }
                }
            });
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
    ) {
        let Some(client_user_id) = get_client().and_then(|c| c.user_id().map(|user_id| user_id.to_owned()) ) else { return };
        if let Some(mut instance) = self.borrow_mut() {
            instance.event_reaction_list = Vec::with_capacity(event_tl_item_reactions.len());
            for (reaction_raw, reaction_senders) in event_tl_item_reactions.iter() {
                // Just take the first char of the emoji, which ignores any variant selectors.
                let reaction_str_option = reaction_raw.chars().next().map(|c| c.to_string());
                let reaction_str = reaction_str_option.as_deref().unwrap_or(reaction_raw);
                let emoji_text = emojis::get(reaction_str)
                    .and_then(|e| e.shortcode())
                    .unwrap_or_else(|| {
                        log!("Failed to parse emoji: {}", reaction_raw);
                        reaction_raw
                    });
                let total_num_react = reaction_senders.len();
                let mut includes_user = false;
                let mut user_id_list = Vec::with_capacity(5);
                for (index, (sender, _react_info)) in reaction_senders.iter().enumerate() {
                    if sender == &client_user_id {
                        includes_user = true;
                    }
                    if index < 5 {
                        user_id_list.push(sender.clone());
                    }
                }
                let tooltip_text_arr:Vec<String> = reaction_senders.iter().map(|(sender, _react_info)|{
                    if sender == &client_user_id {
                        includes_user = true;
                    }
                    get_user_profile_and_room_member(cx, sender.clone(), &room_id, true).0
                        .map(|user_profile| user_profile.displayable_name().to_string())
                        .unwrap_or(sender.to_string())
                }).collect();
               let mut tooltip_text = human_readable_list(&tooltip_text_arr);                
                tooltip_text.push_str(&format!("\nreacted with: {}", emoji_text));
                instance.event_reaction_list.push(ReactionData{
                    emoji: emoji_text.to_string(),
                    total_num_react,
                    tooltip_text,
                    includes_user,
                    width: 0.0,
                });
            }
            instance.room_id = Some(room_id);
            instance.timeline_event_id = Some(timeline_event_item_id);
            instance.width_calculated = false;
        }
    }
    pub fn hover_in(&self, actions: &Actions) -> RoomScreenTooltipActions {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            item.cast()
        } else {
            RoomScreenTooltipActions::None
        }
    }
    /// Handles hover out action
    pub fn hover_out(&self, actions: &Actions) -> bool {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            matches!(item.cast(), RoomScreenTooltipActions::HoverOut)
        } else {
            false
        }
    }
}
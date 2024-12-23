use crate::sliding_sync::{get_client, submit_async_request, MatrixRequest};
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk_ui::timeline::{ReactionsByKeyBySender, TimelineEventItemId};
use crate::profile::user_profile_cache::get_user_profile;
use crate::home::room_screen::RoomScreenTooltipActions;
const TOOLTIP_WIDTH: f64 = 100.0;
const REACTION_LIST_PADDING_RIGHT: f64 = 3.0;
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
    width: f64
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
    /// A list of ReactionData which includes data required to draw the reaction buttons and their tooltips
    /// After the first draw, the buttons will be stored in this vector
    #[rust]
    event_reaction_list: Vec<ReactionData>,
    #[rust]
    room_id: Option<OwnedRoomId>,
    #[rust]
    timeline_event_id: Option<TimelineEventItemId>,
    /// Has the width of the emoji buttons already been drawn and calculated beforehand?
    #[rust]
    width_calculated: bool,
    /// Tooltip that appears when hovering over a reaction button, (Index in event_reaction_list, tooltip rendering rectangle's area, tooltip's text)
    #[rust]
    tooltip_state: Option<(u64, Rect, String)>
}
impl Widget for ReactionList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        let width: f64 = rect.size.x;
        if !self.width_calculated {
            // Records the buttons' width after the first draw
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
                        draw_bg: { hide: 0.0 , color: (vec4(0.0, 0.6, 0.47, 1.0)) }
                    }
                } else {
                    live! {
                        draw_bg: { hide: 0.0, color: (vec4(0.714, 0.73, 0.75, 1.0)) }
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
                            // Temporary hack to improve the issue that the tooltip is cut off by the right side of the screen
                            // As the width of the tooltip not currently calculated, it is difficult to prevent the tooltip from being cut off
                            // If the mouse position is too close to right side of the screen, the tooltip will be left-aligned to the reaction button 
                            let rect =  Rect {
                                pos: DVec2 {
                                    x: widget_rect.pos.x + widget_rect.size.x - REACTION_LIST_PADDING_RIGHT,
                                    y: widget_rect.pos.y - widget_rect.size.y / 2.0
                                },
                                size: DVec2::new(),
                            };
                            // Stores the event_reaction_list index together with the tooltip area and tooltip text into tooltip state
                            // The index will be used later to reset the tooltip state if the mouse leaves this particular reaction button
                            self.tooltip_state = Some((id.0, rect, reaction_data.tooltip_text.clone()));
                        }
                    }
                }
            } else {
                let mut reset_tooltip_state = false;
                if let Some((ref index, ref tooltip_area, ref tooltip_text)) = &self.tooltip_state {
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
                        cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverIn(*tooltip_area, tooltip_text.clone(), TOOLTIP_WIDTH));
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
        _cx: &mut Cx,
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
                let tooltip_text_arr:Vec<String> = reaction_senders.iter().map(|(sender, _react_info)|{
                    if sender == &client_user_id {
                        includes_user = true;
                    }
                    let sender_name = get_user_profile(_cx, sender).map(|profile| {
                        profile.displayable_name().to_owned()
                    }).unwrap_or(sender.to_string());
                    sender_name
                }).collect();
                let mut tooltip_text = human_readable_list(tooltip_text_arr);
                // Manually create new line to manage the tooltip width as the width is set as Fit
                // TODO: Find a better way to manage the tooltip width
                // The tooltip width follows the length of the first line instead of the longest line
                tooltip_text.insert_str(0, &format!("{} \n ", emoji_text));
                instance.event_reaction_list.push(ReactionData{
                    emoji: emoji_text.to_string(),
                    total_num_react,
                    tooltip_text,
                    includes_user,
                    width: 0.0
                });
            }
            instance.room_id = Some(room_id);
            instance.timeline_event_id = Some(timeline_event_item_id);
            instance.width_calculated = false;
        }
    }
    pub fn hover_in(&self, actions: &Actions) -> Option<(Rect, String, f64)> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            match item.cast() {
                RoomScreenTooltipActions::HoverIn(rect, tooltip_text, tooltip_width) => Some((rect, tooltip_text, tooltip_width)),
                _ => None,
            }
        } else {
            None
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
/// Converts a list of names into a human-readable string.
///
/// # Examples
/// ```
/// assert_eq!(human_readable_list(vec!["Alice"]), "Alice");
/// assert_eq!(human_readable_list(vec!["Alice", "Bob"]), "Alice and Bob");
/// assert_eq!(human_readable_list(vec!["Alice", "Bob", "Charlie"]), "Alice, Bob and Charlie");
/// ```
fn human_readable_list(names: Vec<String>) -> String {
    match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        2 => format!("{} and {}", names[0], names[1]),
        _ => {
            let last = names.last().unwrap();
            let rest = &names[..names.len() - 1];
            format!("{}, and {}", rest.join(", "), last)
        }
    }
}
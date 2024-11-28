use crate::sliding_sync::{submit_async_request, MatrixRequest};
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk_ui::timeline::{ReactionsByKeyBySender, TimelineEventItemId};

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;
    import crate::shared::styles::*;
    COLOR_BUTTON_DARKER = #454343
    ReactionList = {{ReactionList}} {
        margin: {
            top:3,
            bottom:3,
            left:3,
            right:3

        },
        padding:{
            right:30
        }
        item: <Button> {
            width: Fit,
            height: Fit,
            spacing: 20,
            padding: {top: 3, bottom: 3, left: 3, right: 3}
            margin: {
                top:3,
                bottom:3,
                left:3,
                right:3

            },
            draw_bg: {
                instance color: (COLOR_BUTTON_DARKER)
                instance color_hover: #fef65b
                instance border_width: 0.0
                instance border_color: #D0D5DD
                instance radius: 3.0
                // The first draw is to get the width of the button, so that we can use it in the second draw
                // If hide >= 0.5, the button is hidden.
                // Without hidding, the buttons layout may appear glitch at the start
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
                        sdf.stroke(self.border_color, self.border_width)
                    }
                    return sdf.result;
                }
            }

            draw_icon: {
                instance color: #000
                instance color_hover: #000
                uniform rotation_angle: 0.0,

                fn get_color(self) -> vec4 {
                    return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
                }


            }
            icon_walk: {width: 16, height: 16}

            draw_text: {
                text_style: <REGULAR_TEXT>{font_size: 8},
                color: #ffffff
                fn get_color(self) -> vec4 {
                    return self.color;
                }
            }
        }
    }
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
    // A list of tuples of (emoji, it's sender count, tooltip_header, it's width)
    // After the first draw, the buttons' will be stored in this vector
    // TODO: Add Tooltip display over the reaction buttons after https://github.com/project-robius/robrix/pull/162 is merged
    #[rust]
    event_reaction_list: Vec<(String, usize, String, f64)>,
    #[rust]
    room_id: Option<OwnedRoomId>,
    #[rust]
    timeline_event_id: Option<TimelineEventItemId>,
    // Has the width of the emoji buttons already been drawn and calculated beforehand?
    #[rust]
    width_calculated: bool
}
impl Widget for ReactionList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        let width: f64 = rect.size.x;
        if !self.width_calculated {
            // Records the buttons' width after the first draw
            let mut prev_width: f64 = 0.0;
            for (index, (emoji, count, _tooltip, item_width)) in
                self.event_reaction_list.iter_mut().enumerate()
            {
                let target = self.children.get_or_insert(cx, LiveId(index as u64), |cx| {
                    WidgetRef::new_from_ptr(cx, self.item).as_button()
                });
                target.set_text(&format!("{} {}", emoji, count));
                // Hide the button until the first draw
                target.apply_over(
                    cx,
                    live! {
                        draw_bg: { hide: 1.0 }
                    },
                );
                target.draw_all(cx, scope);
                let used = cx.turtle().used();
                *item_width = used.x - prev_width;
                prev_width = used.x;
            }
    
            self.width_calculated = true;
        } else {
            let mut acc_width: f64 = 0.0;
            for (index, (emoji, count, _tooltip, item_width)) in
                self.event_reaction_list.iter().enumerate()
            {
                let target = self.children.get_or_insert(cx, LiveId(index as u64), |cx| {
                    WidgetRef::new_from_ptr(cx, self.item).as_button()
                });
                target.set_text(&format!("{} {}", emoji, count));
                // Unhide the button as we have the width of the buttons
                target.apply_over(
                    cx,
                    live! {
                        draw_bg: { hide: 0.0 }
                    },
                );
                acc_width += item_width;
                // Creates a new line if the accumulated width exceeds the available space
                if acc_width > width {
                    cx.turtle_new_line();
                    acc_width = *item_width;
                    let used: DVec2 = cx.turtle().used();
                    // Resets the turtle's width after each new line
                    cx.turtle_mut().set_used(0.0, used.y);
                }
                target.draw_all(cx, scope);
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
        self.children
            .iter()
            .enumerate()
            .for_each(|(_index, (_id, widget_ref))| {
                widget_ref.handle_event(cx, event, scope);
                match event {
                    Event::Actions(actions) => {
                        if widget_ref.clicked(&actions) {
                            let text = widget_ref.text().clone();
                            let mut reaction_string_arr: Vec<&str> = text.split(" ").collect();
                            reaction_string_arr.pop();
                            let reaction_string = reaction_string_arr.join(" ");
                            if let Some(key) = emojis::get_by_shortcode(&reaction_string) {
                                submit_async_request(MatrixRequest::ToggleReaction {
                                    room_id: room_id.clone(),
                                    timeline_event_id: timeline_event_id.clone(),
                                    reaction_key: key.as_str().to_string(),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            });
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
        if let Some(mut instance) = self.borrow_mut() {
            instance.event_reaction_list = Vec::with_capacity(event_tl_item_reactions.len());
            for (reaction_raw, reaction_senders) in event_tl_item_reactions.iter() {
                // Just take the first char of the emoji, which ignores any variant selectors.
                let reaction_str_option = reaction_raw.chars().next().map(|c| c.to_string());
                let reaction_str = reaction_str_option.as_deref().unwrap_or(reaction_raw);
                let text_to_display = emojis::get(reaction_str)
                    .and_then(|e| e.shortcode())
                    .unwrap_or_else(|| {
                        log!("Failed to parse emoji: {}", reaction_raw);
                        reaction_raw
                    });
                let count = reaction_senders.len();
                let tooltip_header_arr:Vec<&str> = reaction_senders.iter().map(|(sender, _react_info)|{
                    sender.as_str()
                }).collect();
                let tooltip_header = human_readable_list(tooltip_header_arr);
                instance.event_reaction_list.push((text_to_display.to_string(), count, tooltip_header, 0.0));
            }
            //instance.event_reaction_list = text_to_display_vec;
            instance.room_id = Some(room_id);
            instance.timeline_event_id = Some(timeline_event_item_id);
            instance.width_calculated = false;
            instance.redraw(cx);
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
fn human_readable_list(names: Vec<&str>) -> String {
    match names.len() {
        0 => String::new(),
        1 => names[0].to_string(),
        2 => format!("{} and {}", names[0], names[1]),
        _ => {
            let last = names.last().unwrap();
            let rest = &names[..names.len() - 1];
            format!("{}, and {}", rest.join(", "), last)
        }
    }
}
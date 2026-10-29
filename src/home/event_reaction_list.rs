use crate::home::room_screen::RoomScreenTooltipActions;
use crate::profile::user_profile_cache;
use crate::sliding_sync::{current_user_id, submit_async_request, MatrixRequest, TimelineKind};
use indexmap::IndexMap;
use makepad_widgets::*;
use crate::{LivePtr, widget_ref_from_live_ptr};
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use matrix_sdk_ui::timeline::{ReactionInfo, ReactionsByKeyBySender, TimelineEventItemId};
use unicode_segmentation::UnicodeSegmentation;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    let ReactionLabel = Label {
        padding: 0
        draw_text +: {
            text_style: REGULAR_TEXT {font_size: 10},
            color: (mod.widgets.COLOR_TEXT_BODY)
        }
    }

    mod.widgets.ReactionList = #(ReactionList::register_widget(vm)) {
        width: Fill,
        height: Fit,
        flow: Flow.Right{wrap: true},
        margin: Inset{top: 5.0}
        item: View {
            width: Fit,
            // A fixed height, so emoji-only reactions (one point larger) aren't taller than text ones.
            height: 28,
            padding: Inset{left: 6, right: 6},
            // Use a zero margin on the left because we want the first reaction
            // to be flush with the left edge of the message text.
            margin: Inset{ top: 3, bottom: 3, left: 0, right: 6 },
            flow: Right,
            align: Align{y: 0.5},
            spacing: 6,

            show_bg: true
            draw_bg +: {
                // Anything that we apply over must be an `instance`.
                reaction_bg_color: instance(mod.widgets.COLOR_REACTION_BG)
                reaction_border_color: instance(mod.widgets.COLOR_REACTION_BORDER)
                color_hover: uniform((mod.widgets.COLOR_REACTION_HOVER_TINT))
                hover: instance(0.0)
                border_size: uniform(1.5)
                border_radius: uniform(3.0)

                get_color: fn() -> vec4 {
                    return mix(self.reaction_bg_color, mix(self.reaction_bg_color, self.color_hover, 0.2), self.hover)
                }

                pixel: fn() {
                    let sdf = Sdf2d.viewport(self.pos * self.rect_size)
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

            animator: Animator{
                hover: {
                    default: @off
                    off: AnimatorState{
                        from: {all: Snap}
                        apply: { draw_bg: {hover: 0.0} }
                    }
                    on: AnimatorState{
                        from: {all: Snap}
                        apply: { draw_bg: {hover: 1.0} }
                    }
                }
            }

            reaction := ReactionLabel {
                flow: Flow.Right { wrap: false },
                width: Fit{max: FitBound.Rel{base: Base.Line, factor: 1.0}},
                max_lines: 1,
                text_overflow: Ellipsis,
            }
            count := ReactionLabel {}
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

impl ReactionData {
    /// Returns this reaction with each emoji's shortcode in parentheses after it, e.g., "👍 (+1)".
    ///
    /// A run of the same emoji only gets one shortcode, after its last emoji.
    pub fn format_with_shortcodes(&self) -> String {
        let mut text = String::new();
        let mut graphemes = self.reaction.graphemes(true).peekable();
        while let Some(grapheme) = graphemes.next() {
            text.push_str(grapheme);
            let Some(emoji) = emojis::get(grapheme) else { continue };
            let next_emoji = graphemes.peek().and_then(|next| emojis::get(next));
            // Skin tone variants don't have their own shortcode, so we use the default one's.
            let shortcode = emoji.shortcode()
                .or_else(|| emoji.with_skin_tone(emojis::SkinTone::Default)?.shortcode());
            if next_emoji != Some(emoji)
                && let Some(shortcode) = shortcode
            {
                text.push_str(&format!(" ({shortcode})"));
                if next_emoji.is_some() {
                    text.push(' ');
                }
            }
        }
        text
    }
}

#[derive(Script, Widget)]
pub struct ReactionList {
    #[uid] uid: WidgetUid,
    #[redraw] #[rust] area: Area,
    #[live] item: Option<LivePtr>,
    #[rust] children: Vec<(ViewRef, ReactionData)>,
    #[layout] layout: Layout,
    #[walk] walk: Walk,

    #[rust] timeline_kind: Option<TimelineKind>,
    #[rust] timeline_event_id: Option<TimelineEventItemId>,

    /// A cheap hash of the last reaction list populuated in this widget.
    /// Consists of: `(reaction count, total senders, num sent by me)`.
    #[rust] last_reaction_counts: Option<(usize, usize, usize)>,
}

impl ScriptHook for ReactionList {
    fn on_after_reload(&mut self, _vm: &mut ScriptVm) {
        // A reload never reaches the buttons we built from the template,
        // so we forget the last counts to make the next `set_list()` rebuild them.
        self.last_reaction_counts = None;
    }
}

impl Widget for ReactionList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.children.is_empty() {
            self.area = Area::Empty;
            return DrawStep::done();
        }
        cx.begin_turtle(walk, self.layout);
        for (button, _) in self.children.iter_mut() {
            let _ = button.draw(cx, scope);
        }
        cx.end_turtle_with_area(&mut self.area);
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
                Hit::FingerHoverIn(..) => {
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
                        let Some(kind) = &self.timeline_kind else { return };
                        let Some(timeline_event_id) = &self.timeline_event_id else {
                            return;
                        };
                        submit_async_request(MatrixRequest::ToggleReaction {
                            timeline_kind: kind.clone(),
                            timeline_event_id: timeline_event_id.clone(),
                            reaction: reaction_data.reaction.clone(),
                        });
                        // update the reaction button before the timeline is updated
                        let mut reaction_button = button_ref.clone();
                        apply_reaction_button_colors(cx, &mut reaction_button, !reaction_data.includes_user);
                        self.do_hover_out(cx, scope, button_ref);
                    }
                    // Touch never sends a hover-out, so clear the highlight the long press set.
                    else if fue.has_long_press_occurred {
                        button_ref.animator_cut(cx, ids!(hover.off));
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
        _scope: &mut Scope,
        button_ref: &ViewRef,
        reaction_data: ReactionData,
    ) {
        cx.widget_action(
            self.widget_uid(), 
            RoomScreenTooltipActions::HoverInReactionButton {
                widget_rect: button_ref.area().rect(cx),
                reaction_data,
            },
        );
        button_ref.animator_cut(cx, ids!(hover.on));
        cx.set_cursor(MouseCursor::Hand);
    }

    /// Deals with to any event/hit that triggers a hover-out action.
    fn do_hover_out(
        &self,
        cx: &mut Cx,
        _scope: &mut Scope,
        button_ref: &ViewRef,
    ) {
        cx.widget_action(self.widget_uid(),  RoomScreenTooltipActions::HoverOut);
        button_ref.animator_cut(cx, ids!(hover.off));
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
        timeline_kind: &TimelineKind,
        timeline_event_item_id: &TimelineEventItemId,
        _id: usize,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let Some(curr_user_id) = current_user_id() else { return };
        let Some(event_tl_item_reactions) = event_tl_item_reactions else {
            inner.children.clear();
            inner.last_reaction_counts = None;
            return;
        };

        // try to see if we can skip populating/redrawing if nothing's changed.
        let mut total_senders = 0;
        let mut num_sent_by_me = 0;
        for (_, reaction_senders) in event_tl_item_reactions.iter() {
            total_senders += reaction_senders.len();
            num_sent_by_me += reaction_senders.contains_key(&curr_user_id) as usize;
        }
        let new_counts = (event_tl_item_reactions.len(), total_senders, num_sent_by_me);
        if inner.last_reaction_counts == Some(new_counts)
            && inner.timeline_event_id.as_ref() == Some(timeline_event_item_id)
        {
            return;
        }
        // here, things have changed, so we need to repopulate the list of reactions and redraw it.
        inner.last_reaction_counts = Some(new_counts);
        inner.children.clear();
        for (reaction_text, reaction_senders) in event_tl_item_reactions.iter() {
            let mut includes_user: bool = false;
            for (sender, _) in reaction_senders.iter() {
                if sender == &curr_user_id {
                    includes_user = true;
                }
                // Pre-fetch the profile of each reaction sender into the cache for future use in the tooltip.
                // TODO: we shouldn't do this, it's wasteful. Instead, fetch it on demand
                //       when showing the tooltip; we just need to add a way to update a live tooltip.
                //       And now that we have room member lists, we can just use that (especially once
                //       we convert them to a hashmap in the async backend, in the future)
                let _ = user_profile_cache::with_user_profile(
                    cx,
                    sender.clone(),
                    Some(timeline_kind.room_id()),
                    true, |_, _| { },
                );
            }

            let reaction_data = ReactionData {
                reaction: reaction_text.to_string(),
                includes_user,
                reaction_senders: reaction_senders.clone(),
                room_id: timeline_kind.room_id().clone(),
            };
            let mut button = widget_ref_from_live_ptr(cx, inner.item).as_view();
            let reaction_label = button.label(cx, ids!(reaction));
            reaction_label.set_text(cx, &reaction_data.reaction);
            let is_emoji_only = reaction_text.graphemes(true).all(|g| emojis::get(g).is_some());
            if is_emoji_only && let Some(mut label) = reaction_label.borrow_mut() {
                label.draw_text.text_style.font_size += 1.0;
            }
            button.label(cx, ids!(count)).set_text(cx, &reaction_senders.len().to_string());
            apply_reaction_button_colors(cx, &mut button, reaction_data.includes_user);
            inner.children.push((button, reaction_data));
        }
        inner.timeline_kind = Some(timeline_kind.clone());
        inner.timeline_event_id = Some(timeline_event_item_id.clone());
    }

    /// Returns `true` if the `abs` coord is within one of the reaction buttons.
    pub fn contains_button(&self, cx: &Cx, abs: DVec2) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.children.iter().any(|(button, _)|
            button.area().clipped_rect(cx).contains(abs)
        )
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

fn apply_reaction_button_colors(cx: &mut Cx, button: &mut ViewRef, includes_user: bool) {
    if includes_user {
        script_apply_eval!(cx, button, {
            draw_bg +: {
                reaction_bg_color: mod.widgets.COLOR_REACTION_BG_INCLUDES_USER,
                reaction_border_color: mod.widgets.COLOR_REACTION_BORDER_INCLUDES_USER,
            }
        });
    } else {
        script_apply_eval!(cx, button, {
            draw_bg +: {
                reaction_bg_color: mod.widgets.COLOR_REACTION_BG,
                reaction_border_color: mod.widgets.COLOR_REACTION_BORDER,
            }
        });
    }
}

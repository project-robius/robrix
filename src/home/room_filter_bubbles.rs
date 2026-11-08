//! `RoomFilterBubbles`: a row of filter bubbles above the rooms list, like "Unreads" and "People".
//!
//! Selecting a bubble shows only the rooms that match it, and selecting several shows only the rooms that match all of them.
//! Selecting a bubble also hides the others that can't be combined with it, like "Rooms" while "People" is selected.
//! The selected bubbles slide to the front of the row, right after an "X" bubble that deselects them all.
//! The "Invites" bubble only shows while there are invites (or while it's selected).

use bitflags::bitflags;
use makepad_widgets::*;
use matrix_sdk::ruma::events::tag::TagName;

use crate::{
    home::rooms_list::SIDE_PADDING,
    room::room_display_filter::FilterableRoom,
    shared::styles::apply_lavender_button_style,
    sliding_sync::UserPowerLevels,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A toggle button that filters the rooms list, which is purple while it's selected.
    mod.widgets.RoomFilterBubble = RobrixIconButton {
        height: 28
        margin: 0
        padding: Inset{left: 12, right: 12}
        spacing: 0
        align: Align{x: 0.5, y: 0.5}
        // Pressing one leaves the filter bar focused, so swiping the bubbles keeps the soft keyboard up (a tap still hides it).
        grab_key_focus: false
        // Only a tap toggles it, not a drag that scrolls the bubbles.
        enable_long_press: true
        draw_bg +: {
            border_radius: 14.0
            border_size: 1.0
            color: (COLOR_BG_LAVENDER)
            color_hover: (COLOR_BG_LAVENDER_HOVER)
            color_down: (COLOR_BG_LAVENDER_DOWN)
            border_color: (COLOR_LAVENDER_BUTTON_BORDER)
            border_color_hover: (COLOR_LAVENDER_BUTTON_BORDER)
            border_color_down: (COLOR_LAVENDER_BUTTON_BORDER)
        }
        draw_text +: {
            color: (ROOM_NAME_TEXT_COLOR)
            color_hover: (ROOM_NAME_TEXT_COLOR)
            color_down: (ROOM_NAME_TEXT_COLOR)
            text_style: REGULAR_TEXT {font_size: 9.5}
        }
    }

    // The round "X" bubble in front of the selected ones, which deselects them all.
    mod.widgets.RoomFilterClearBubble = mod.widgets.RoomFilterBubble {
        width: 28
        padding: 0
        // It disappears once clicked, so it shouldn't still look hovered when it comes back.
        reset_hover_on_click: true
        draw_icon +: {
            svg: (ICON_CLOSE)
            color: (ROOM_NAME_TEXT_COLOR)
        }
        icon_walk: Walk{width: 9, height: 9}
    }

    // Holds one bubble, so we can slide that bubble to a new place with this slot's margin.
    mod.widgets.RoomFilterBubbleSlot = View { width: Fit, height: Fit }

    mod.widgets.RoomFilterBubbles = #(RoomFilterBubbles::register_widget(vm)) {
        width: Fill, height: Fit

        // Scrolls sideways when the bubbles don't all fit.
        bubbles := ScrollXView {
            width: Fill, height: 28
            scroll_bars +: {
                scroll_bar_x +: {
                    show_handle: false
                    smoothing: 0.15
                    // Lets a mouse wheel scroll it sideways.
                    use_vertical_finger_scroll: true
                    // Lets a click-drag that starts on a bubble scroll it too, like a touch drag does.
                    mouse_drags_from_widgets: true
                }
            }
            // A scroll view's own right padding doesn't count towards how far it scrolls, but this one's does.
            row := View {
                width: Fit, height: Fit
                flow: Right, spacing: 6
                padding: Inset{left: #(SIDE_PADDING), right: #(SIDE_PADDING)}
                clear := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterClearBubble {} }
                unreads := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble { text: "Unreads" } }
                mentions := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble { text: "Mentions" } }
                people := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble { text: "People" } }
                rooms := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble { text: "Rooms" } }
                mod_admin := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble { text: "Mod/Admin" } }
                favorites := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble { text: "Favorites" } }
                invites := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble { text: "Invites" } }
                low_priority := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble { text: "Low Priority" } }
            }
        }
    }
}

bitflags! {
    /// The filters that the selected bubbles apply to the rooms list.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub struct BubbleFilters: u8 {
        const Unreads     = 0b0000_0001;
        const Mentions    = 0b0000_0010;
        const People      = 0b0000_0100;
        const Rooms       = 0b0000_1000;
        const Favorites   = 0b0001_0000;
        const Invites     = 0b0010_0000;
        const LowPriority = 0b0100_0000;
        const ModAdmin    = 0b1000_0000;
    }
}

impl BubbleFilters {
    /// Returns whether the given room matches all of these filters.
    pub fn matches(self, room: &dyn FilterableRoom, show_invites_in_unread_filters: bool) -> bool {
        self.iter().all(|filter| match filter {
            Self::Unreads | Self::Mentions if room.is_invite() => show_invites_in_unread_filters,
            Self::Unreads => room.has_unreads(),
            Self::Mentions => room.unread_mentions() > 0,
            Self::People => room.is_direct(),
            Self::Rooms => !room.is_direct() && !room.is_space(),
            Self::ModAdmin => !room.is_direct() && !room.is_space()
                && room.user_power_levels().is_some_and(UserPowerLevels::is_mod_or_admin),
            Self::Favorites => room.tags().contains_key(&TagName::Favorite),
            Self::Invites => room.is_invite(),
            Self::LowPriority => room.tags().contains_key(&TagName::LowPriority),
            _ => true,
        })
    }

    /// Returns the other filters that can't be combined with these ones, so we hide their bubbles.
    fn excluded(self) -> Self {
        EXCLUSIVE_GROUPS.iter()
            .filter(|group| group.intersects(self))
            .fold(Self::empty(), |excluded, group| excluded | *group)
            - self
    }
}

/// Groups of filters where selecting one hides the rest of its group.
const EXCLUSIVE_GROUPS: [BubbleFilters; 4] = [
    // Mentions are always unread, and the unread filters show either all invites or none of them.
    BubbleFilters::Unreads.union(BubbleFilters::Mentions).union(BubbleFilters::Invites),
    // Mod/Admin rooms are always Rooms, never People.
    BubbleFilters::People.union(BubbleFilters::Rooms).union(BubbleFilters::ModAdmin),
    // Invites don't have tags, and the SDK never lets a room be both a favorite and low priority.
    BubbleFilters::Favorites.union(BubbleFilters::LowPriority).union(BubbleFilters::Invites),
    // We don't know our power levels in rooms we haven't joined.
    BubbleFilters::ModAdmin.union(BubbleFilters::Invites),
];

/// The ID of each bubble's slot, and the bubble's filter.
const BUBBLES: [(LiveId, BubbleFilters); 8] = [
    (id!(unreads), BubbleFilters::Unreads),
    (id!(mentions), BubbleFilters::Mentions),
    (id!(people), BubbleFilters::People),
    (id!(rooms), BubbleFilters::Rooms),
    (id!(mod_admin), BubbleFilters::ModAdmin),
    (id!(favorites), BubbleFilters::Favorites),
    (id!(invites), BubbleFilters::Invites),
    (id!(low_priority), BubbleFilters::LowPriority),
];

/// How long the bubbles take to slide to their new places, in seconds.
const SLIDE_DURATION: f64 = 0.25;

/// Actions emitted by or sent to the [`RoomFilterBubbles`].
#[derive(Debug)]
pub enum RoomFilterBubblesAction {
    /// The selected bubbles changed, so these filters now apply.
    Changed(BubbleFilters),
    /// Request to deselect all bubbles, which then emits an empty `Changed` action.
    Clear,
    /// Tells the bubbles whether there are any invites, since we only show the Invites bubble when there are.
    HasInvites(bool),
}

/// How the bubbles get to their new places the next time we draw them.
enum Relayout {
    /// They slide there from where each shown bubble's slot was, relative to our left edge.
    Slide(Vec<(LiveId, f64)>),
    /// They jump straight there, like after a reload put them back where the DSL has them.
    Jump,
}

/// A shown bubble sliding to its new place.
struct BubbleSlide {
    slot_id: LiveId,
    /// The bubble's new place, relative to our left edge.
    x: f64,
    width: f64,
    /// How far from its new place the bubble starts.
    distance: f64,
}

/// A row of toggleable filter bubbles that the rooms list shows above its rooms.
///
/// See the module-level docs for more detail.
#[derive(Script, Widget)]
pub struct RoomFilterBubbles {
    #[deref] view: View,

    #[rust] selected: BubbleFilters,
    /// The selected filters in the order they were selected, which is also the order of their bubbles.
    #[rust] selection_order: Vec<BubbleFilters>,
    #[rust] has_invites: bool,
    /// The selection that the bubbles' styles show, which a reload resets to none.
    #[rust] shown_selection: BubbleFilters,
    #[rust(Some(Relayout::Jump))] relayout: Option<Relayout>,
    /// The shown bubbles while they're sliding, in the order we draw them.
    #[rust] slides: Vec<BubbleSlide>,
    #[rust] slide_start_time: Option<f64>,
    #[rust] next_frame: NextFrame,
}

impl ScriptHook for RoomFilterBubbles {
    fn on_after_apply(&mut self, _vm: &mut ScriptVm, apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        if apply.is_reload() {
            self.shown_selection = BubbleFilters::empty();
            self.relayout = Some(Relayout::Jump);
            self.slides.clear();
            self.slide_start_time = None;
        }
    }
}

impl Widget for RoomFilterBubbles {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Some(ne) = self.next_frame.is_event(event)
            && !self.slides.is_empty()
        {
            let start_time = *self.slide_start_time.get_or_insert(ne.time);
            let progress = ((ne.time - start_time) / SLIDE_DURATION).min(1.0);
            // This eases out, so the bubbles slow down as they reach their new places.
            self.shift_bubbles(cx, (1.0 - progress).powi(3));
            if progress < 1.0 {
                self.next_frame = cx.new_next_frame();
            } else {
                // They're in place now, so we put them back in the row's order without any margins.
                self.slides.clear();
                self.slide_start_time = None;
                self.relayout.get_or_insert(Relayout::Jump);
            }
            self.redraw(cx);
        }

        let bubble_actions = cx.capture_actions(|cx| self.view.handle_event(cx, event, scope));
        let mut selected = self.selected;
        if !bubble_actions.is_empty() {
            if self.view.button(cx, ids!(clear.bubble)).clicked(&bubble_actions) {
                selected = BubbleFilters::empty();
            }
            for (id, filter) in BUBBLES {
                if self.view.button(cx, &[id, id!(bubble)]).clicked(&bubble_actions) {
                    selected.toggle(filter);
                }
            }
        }
        if let Event::Actions(actions) = event {
            for action in actions {
                match action.downcast_ref() {
                    Some(RoomFilterBubblesAction::Clear) => selected = BubbleFilters::empty(),
                    Some(RoomFilterBubblesAction::HasInvites(has_invites)) if *has_invites != self.has_invites => {
                        self.has_invites = *has_invites;
                        self.slide_on_next_draw(cx);
                        self.redraw(cx);
                    }
                    _ => {}
                }
            }
        }
        if selected != self.selected {
            self.selection_order.retain(|filter| selected.contains(*filter));
            self.selection_order.extend((selected - self.selected).iter());
            self.selected = selected;
            self.slide_on_next_draw(cx);
            cx.action(RoomFilterBubblesAction::Changed(selected));
            self.redraw(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(relayout) = self.relayout.take() else {
            return self.view.draw_walk(cx, scope, walk);
        };
        let (shown, selected) = (self.shown_selection, self.selected);
        for (id, filter) in BUBBLES {
            if shown.contains(filter) != selected.contains(filter) {
                let mut bubble = self.view.button(cx, &[id, id!(bubble)]);
                apply_lavender_button_style(cx, &mut bubble, selected.contains(filter));
            }
        }
        self.shown_selection = selected;
        // The selected bubbles go at the start of the row, so that's where we scroll to.
        if matches!(relayout, Relayout::Slide(_)) && shown != selected {
            self.view.view(cx, ids!(bubbles)).set_scroll_pos(cx, DVec2::default());
        }

        // The X goes first, then the selected bubbles in the order they were selected, then the rest.
        let excluded = selected.excluded();
        let order: Vec<LiveId> = (!selected.is_empty()).then_some(id!(clear)).into_iter()
            .chain(self.selection_order.iter().filter_map(|selected_filter| {
                BUBBLES.iter().find(|(_, filter)| filter == selected_filter).map(|(id, _)| *id)
            }))
            .chain(BUBBLES.iter()
                .filter(|(_, filter)| !selected.contains(*filter)
                    && !excluded.contains(*filter)
                    && (*filter != BubbleFilters::Invites || self.has_invites)
                )
                .map(|(id, _)| *id)
            )
            .collect();

        let row = self.view.view(cx, ids!(row));
        if let Some(mut row) = row.borrow_mut() {
            for (id, slot) in &row.children {
                slot.set_visible(cx, order.contains(id));
                if let Some(mut slot) = slot.as_view().borrow_mut() {
                    slot.walk.margin.left = 0.0;
                }
            }
            row.children.sort_by_key(|(id, _)| order.iter().position(|shown_id| shown_id == id).unwrap_or(usize::MAX));
        }

        let step = self.view.draw_walk(cx, scope, walk);

        // Now that the bubbles are laid out in their new places, we move them back to where they were
        // and draw them again, so they can slide over from there.
        if let Relayout::Slide(old_xs) = relayout
            && !old_xs.is_empty()
            && let Some(mut row) = row.borrow_mut()
        {
            let origin_x = self.view.area().rect(cx).pos.x;
            let mut slides: Vec<BubbleSlide> = row.children.iter()
                .filter(|(id, _)| order.contains(id))
                .map(|(id, slot)| {
                    let rect = slot.area().rect(cx);
                    let x = rect.pos.x - origin_x;
                    let old_x = old_xs.iter().find(|(old_id, _)| old_id == id).map(|(_, old_x)| *old_x);
                    BubbleSlide { slot_id: *id, x, width: rect.size.x, distance: old_x.map_or(0.0, |old_x| old_x - x) }
                })
                .collect();
            if slides.iter().any(|slide| slide.distance != 0.0) {
                // The bubbles that were just selected or deselected slide over the others.
                let toggled = shown ^ selected;
                slides.sort_by_key(|slide| BUBBLES.iter().any(|(id, filter)| *id == slide.slot_id && toggled.contains(*filter)));
                row.children.sort_by_key(|(id, _)| slides.iter().position(|slide| slide.slot_id == *id).unwrap_or(usize::MAX));
                drop(row);
                self.slides = slides;
                self.shift_bubbles(cx, 1.0);
                cx.redraw_area_in_draw(self.view.area());
                self.slide_start_time = None;
                self.next_frame = cx.new_next_frame();
            }
        }
        step
    }
}

impl RoomFilterBubbles {
    /// Remembers where the shown bubbles are, so they can slide from there once we lay them out again.
    ///
    /// This must happen before that redraw, which makes the areas we read their places from stale.
    fn slide_on_next_draw(&mut self, cx: &mut Cx) {
        if self.relayout.is_some() {
            return;
        }
        let origin_x = self.view.area().rect(cx).pos.x;
        let old_xs = self.view.view(cx, ids!(row)).borrow().map(|row| row.children.iter()
            .filter(|(_, slot)| slot.visible() && slot.area().is_valid(cx))
            .map(|(id, slot)| (*id, slot.area().rect(cx).pos.x - origin_x))
            .collect()
        );
        self.relayout = Some(Relayout::Slide(old_xs.unwrap_or_default()));
    }

    /// Moves each sliding bubble away from its new place by the given fraction of its slide's distance.
    fn shift_bubbles(&mut self, cx: &mut Cx, fraction: f64) {
        let row = self.view.view(cx, ids!(row));
        let Some(row) = row.borrow() else { return };
        // The row puts each slot after the one drawn before it, so a slot's margin is the gap from there.
        let mut next_x = self.slides.iter().map(|slide| slide.x).fold(f64::INFINITY, f64::min);
        for slide in &self.slides {
            let x = slide.x + slide.distance * fraction;
            if let Some((_, slot)) = row.children.iter().find(|(id, _)| *id == slide.slot_id)
                && let Some(mut slot) = slot.as_view().borrow_mut()
            {
                slot.walk.margin.left = x - next_x;
            }
            next_x = x + slide.width + row.layout.spacing;
        }
    }
}

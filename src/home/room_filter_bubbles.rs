//! `RoomFilterBubbles`: a row of filter bubbles above the rooms list, like "Unreads" and "People".
//!
//! Tapping a bubble includes its filter, and long-pressing or right-clicking it excludes its filter ("Not Unreads"),
//! so the rooms list only shows the rooms that match every included filter and none of the excluded ones.
//! We hide the bubbles that can't be combined with the active ones, like "Rooms" while "People" is included,
//! and the ones that would add nothing, like "Unreads" while "Mentions" is included.
//! The active bubbles slide to the front of the row, right after an "X" bubble that deactivates them all.
//! The "Invites" bubble only shows while there are invites (or while it's active).

use bitflags::bitflags;
use makepad_widgets::{makepad_platform::event::{TAP_COUNT_DISTANCE, TAP_COUNT_TIME}, *};
use matrix_sdk::ruma::events::tag::TagName;

use crate::{
    home::rooms_list::SIDE_PADDING,
    room::room_display_filter::FilterableRoom,
    settings::app_preferences::{AppPreferencesAction, AppPreferencesGlobal},
    shared::styles::apply_lavender_button_style,
    sliding_sync::UserPowerLevels,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A button that filters the rooms list, which is purple while its filter is included and red while it's excluded.
    mod.widgets.RoomFilterBubble = RobrixIconButton {
        height: 28
        margin: 0
        padding: Inset{left: 12, right: 12}
        spacing: 0
        align: Align{x: 0.5, y: 0.5}
        // Pressing one leaves the filter bar focused, so swiping the bubbles keeps the soft keyboard up (a tap still hides it).
        grab_key_focus: false
        // A long press excludes its filter, and only a tap toggles it, not a drag that scrolls the bubbles.
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

    // The round "X" bubble in front of the active ones, which deactivates them all.
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
                // Each bubble's text comes from `BUBBLES`, since an excluded one starts with "Not".
                unreads := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble {} }
                mentions := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble {} }
                people := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble {} }
                rooms := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble {} }
                mod_admin := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble {} }
                favorites := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble {} }
                invites := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble {} }
                low_priority := mod.widgets.RoomFilterBubbleSlot { bubble := mod.widgets.RoomFilterBubble {} }
            }
        }
    }
}

bitflags! {
    /// A set of the filters that the bubbles can apply to the rooms list.
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
    fn matches(self, room: &dyn FilterableRoom, show_invites_in_unread_filters: bool) -> bool {
        self.iter().all(|filter| match filter {
            Self::Unreads | Self::Mentions if room.is_invite() => show_invites_in_unread_filters,
            Self::Unreads => room.has_unreads(),
            Self::Mentions => room.unread_mentions() > 0,
            Self::People => room.is_direct(),
            Self::Rooms => !room.is_direct(),
            Self::ModAdmin => !room.is_direct() && room.user_power_levels().is_some_and(UserPowerLevels::is_mod_or_admin),
            Self::Favorites => room.tags().contains_key(&TagName::Favorite),
            Self::Invites => room.is_invite(),
            Self::LowPriority => room.tags().contains_key(&TagName::LowPriority),
            _ => true,
        })
    }
}

/// The filters of the active bubbles.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct BubbleSelection {
    included: BubbleFilters,
    excluded: BubbleFilters,
}

impl BubbleSelection {
    pub fn is_empty(self) -> bool {
        self.active().is_empty()
    }

    /// Returns the filters that are either included or excluded.
    pub fn active(self) -> BubbleFilters {
        self.included | self.excluded
    }

    pub fn matches(self, room: &dyn FilterableRoom, show_invites_in_unread_filters: bool) -> bool {
        self.included.matches(room, show_invites_in_unread_filters)
            && !self.excluded.iter().any(|filter| filter.matches(room, show_invites_in_unread_filters))
    }

    /// Excludes the given filter, or deactivates it if it's already excluded.
    fn toggle_excluded(&mut self, filter: BubbleFilters) {
        self.included.remove(filter);
        self.excluded.toggle(filter);
    }

    /// Returns whether no room could ever match these filters, like "People" with "Rooms",
    /// as opposed to none of our rooms matching them right now.
    fn is_contradictory(self, show_invites_in_unread_filters: bool) -> bool {
        use BubbleFilters as F;
        // Each kind of room we could show, as the filters it matches. Mentions are always unread,
        // and the SDK never lets a room be both a favorite and low priority.
        let joined_rooms = [F::People, F::Rooms, F::Rooms | F::ModAdmin].into_iter()
            .flat_map(|kind| [F::empty(), F::Unreads, F::Unreads | F::Mentions].map(|unreads| kind | unreads))
            .flat_map(|room| [F::empty(), F::Favorites, F::LowPriority].map(|tag| room | tag));
        // Invites can be to a DM or a room (spaces count as rooms), and they don't have tags or power levels.
        let invite_unreads = if show_invites_in_unread_filters { F::Unreads | F::Mentions } else { F::empty() };
        let invites = [F::People, F::Rooms].map(|kind| kind | F::Invites | invite_unreads);
        !joined_rooms.chain(invites).any(|room| room.contains(self.included) && !room.intersects(self.excluded))
    }

    /// Returns whether including or excluding the given filter alongside the others would always show
    /// either no rooms or the same rooms, so its bubble isn't worth showing.
    fn is_pointless(self, filter: BubbleFilters, show_invites_in_unread_filters: bool) -> bool {
        let others = BubbleSelection { included: self.included - filter, excluded: self.excluded - filter };
        BubbleSelection { included: others.included | filter, ..others }.is_contradictory(show_invites_in_unread_filters)
            || BubbleSelection { excluded: others.excluded | filter, ..others }.is_contradictory(show_invites_in_unread_filters)
    }
}

/// The ID of each bubble's slot, the bubble's filter, and its text.
const BUBBLES: [(LiveId, BubbleFilters, &str); 8] = [
    (id!(unreads), BubbleFilters::Unreads, "Unreads"),
    (id!(mentions), BubbleFilters::Mentions, "Mentions"),
    (id!(people), BubbleFilters::People, "People"),
    (id!(rooms), BubbleFilters::Rooms, "Rooms"),
    (id!(mod_admin), BubbleFilters::ModAdmin, "Mod/Admin"),
    (id!(favorites), BubbleFilters::Favorites, "Favorites"),
    (id!(invites), BubbleFilters::Invites, "Invites"),
    (id!(low_priority), BubbleFilters::LowPriority, "Low Priority"),
];

/// How long the bubbles take to slide to their new places, in seconds.
const SLIDE_DURATION: f64 = 0.25;

/// Actions emitted by or sent to the [`RoomFilterBubbles`].
#[derive(Debug)]
pub enum RoomFilterBubblesAction {
    /// The active bubbles changed, so these filters now apply.
    Changed(BubbleSelection),
    /// Request to deactivate all bubbles, which then emits an empty `Changed` action.
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

/// A bubble held down with the mouse, which we time ourselves since desktops don't send long presses.
struct MouseHold {
    filter: BubbleFilters,
    abs_start: DVec2,
    timer: Timer,
}

/// A row of toggleable filter bubbles that the rooms list shows above its rooms.
///
/// See the module-level docs for more detail.
#[derive(Script, Widget)]
pub struct RoomFilterBubbles {
    #[deref] view: View,

    #[rust] selection: BubbleSelection,
    /// The active filters in the order they were activated, which is also the order of their bubbles.
    #[rust] selection_order: Vec<BubbleFilters>,
    #[rust] has_invites: bool,
    /// The selection that the bubbles' texts and styles show, or `None` before we set them, like after a reload.
    #[rust] shown_selection: Option<BubbleSelection>,
    #[rust(Some(Relayout::Jump))] relayout: Option<Relayout>,
    /// The shown bubbles while they're sliding, in the order we draw them.
    #[rust] slides: Vec<BubbleSlide>,
    #[rust] slide_start_time: Option<f64>,
    #[rust] next_frame: NextFrame,
    #[rust] mouse_hold: Option<MouseHold>,
}

impl ScriptHook for RoomFilterBubbles {
    fn on_after_apply(&mut self, _vm: &mut ScriptVm, apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        if apply.is_reload() {
            self.shown_selection = None;
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
        // A bubble's button claims a right-click without acting on it, so we check which bubble claimed it.
        let right_clicked_area = match event {
            Event::MouseDown(mde) if mde.button.is_secondary() && !mde.handled.get().is_empty() => Some(mde.handled.get()),
            _ => None,
        };
        // A mouse held on a bubble without moving excludes its filter once it's held as long as a long press.
        let mut long_held_filter = None;
        match event {
            Event::MouseDown(mde) if mde.button.is_primary() => {
                self.mouse_hold = BUBBLES.iter()
                    .find(|(id, ..)| self.view.button(cx, &[*id, id!(bubble)]).pressed(&bubble_actions))
                    .map(|&(_, filter, _)| MouseHold { filter, abs_start: mde.abs, timer: cx.start_timeout(TAP_COUNT_TIME) });
            }
            Event::MouseMove(mme) if self.mouse_hold.as_ref().is_some_and(|hold| (mme.abs - hold.abs_start).length() < TAP_COUNT_DISTANCE) => {}
            Event::MouseMove(_) | Event::MouseUp(_) => {
                if let Some(hold) = self.mouse_hold.take() {
                    cx.stop_timer(hold.timer);
                }
            }
            _ => {
                if let Some(hold) = self.mouse_hold.take_if(|hold| hold.timer.is_event(event).is_some()) {
                    long_held_filter = Some(hold.filter);
                }
            }
        }
        let mut selection = self.selection;
        let mut changed_filter = None;
        if !bubble_actions.is_empty() || right_clicked_area.is_some() || long_held_filter.is_some() {
            if self.view.button(cx, ids!(clear.bubble)).clicked(&bubble_actions) {
                selection = BubbleSelection::default();
            }
            for (id, filter, _) in BUBBLES {
                let bubble = self.view.button(cx, &[id, id!(bubble)]);
                if bubble.clicked(&bubble_actions) {
                    // A tap includes an inactive bubble's filter, or deactivates an active bubble.
                    if selection.excluded.contains(filter) {
                        selection.excluded.remove(filter);
                    } else {
                        selection.included.toggle(filter);
                    }
                } else if bubble.long_pressed(&bubble_actions) || long_held_filter == Some(filter) {
                    selection.toggle_excluded(filter);
                } else if right_clicked_area == Some(bubble.area()) {
                    // It can slide out from under the pointer before the release, which would leave it looking hovered.
                    bubble.reset_hover(cx);
                    selection.toggle_excluded(filter);
                } else {
                    continue;
                }
                changed_filter = Some(filter);
            }
        }
        let mut did_show_invites_setting_change = false;
        if let Event::Actions(actions) = event {
            for action in actions {
                match action.downcast_ref() {
                    Some(RoomFilterBubblesAction::Clear) => selection = BubbleSelection::default(),
                    Some(RoomFilterBubblesAction::HasInvites(has_invites)) if *has_invites != self.has_invites => {
                        self.has_invites = *has_invites;
                        self.slide_on_next_draw(cx);
                        self.redraw(cx);
                    }
                    _ => {}
                }
                did_show_invites_setting_change |= matches!(
                    action.downcast_ref(),
                    Some(AppPreferencesAction::ShowInvitesInUnreadFiltersChanged(_))
                );
            }
        }
        if selection != self.selection || did_show_invites_setting_change {
            self.selection_order.extend((selection.active() - self.selection.active()).iter());
            // We deactivate the filters that the others make pointless, keeping the filter that just changed
            // and then the newest ones, so including Mentions while Unreads is included turns off Unreads.
            let show_invites_in_unread_filters = cx.global::<AppPreferencesGlobal>().0.show_invites_in_unread_filters;
            let mut kept = BubbleSelection::default();
            for &filter in changed_filter.iter().chain(self.selection_order.iter().rev()) {
                let with_filter = BubbleSelection {
                    included: kept.included | (selection.included & filter),
                    excluded: kept.excluded | (selection.excluded & filter),
                };
                if !with_filter.active().iter().any(|active_filter| with_filter.is_pointless(active_filter, show_invites_in_unread_filters)) {
                    kept = with_filter;
                }
            }
            selection = kept;
            self.selection_order.retain(|filter| selection.active().contains(*filter));
            self.slide_on_next_draw(cx);
            if selection != self.selection {
                self.selection = selection;
                cx.action(RoomFilterBubblesAction::Changed(selection));
            }
            self.redraw(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(relayout) = self.relayout.take() else {
            return self.view.draw_walk(cx, scope, walk);
        };
        let selection = self.selection;
        let (shown, restyled) = match self.shown_selection.replace(selection) {
            Some(shown) => (shown, (shown.included ^ selection.included) | (shown.excluded ^ selection.excluded)),
            None => (BubbleSelection::default(), BubbleFilters::all()),
        };
        for (id, filter, text) in BUBBLES {
            if !restyled.contains(filter) {
                continue;
            }
            let mut bubble = self.view.button(cx, &[id, id!(bubble)]);
            if selection.excluded.contains(filter) {
                script_apply_eval!(cx, bubble, {
                    draw_bg +: {
                        color: mod.widgets.COLOR_BG_DANGER_RED_SOLID,
                        color_hover: mod.widgets.COLOR_BG_DANGER_RED_SOLID_HOVER,
                        color_down: mod.widgets.COLOR_BG_DANGER_RED_SOLID_DOWN,
                        border_color: mod.widgets.COLOR_BG_DANGER_RED_SOLID,
                        border_color_hover: mod.widgets.COLOR_BG_DANGER_RED_SOLID_HOVER,
                        border_color_down: mod.widgets.COLOR_BG_DANGER_RED_SOLID_DOWN,
                    }
                    draw_text +: {
                        color: mod.widgets.COLOR_TEXT_ON_ACCENT,
                        color_hover: mod.widgets.COLOR_TEXT_ON_ACCENT,
                        color_down: mod.widgets.COLOR_TEXT_ON_ACCENT,
                    }
                });
                bubble.set_text(cx, &format!("Not {text}"));
            } else {
                apply_lavender_button_style(cx, &mut bubble, selection.included.contains(filter));
                bubble.set_text(cx, text);
            }
        }
        // The active bubbles go at the start of the row, so that's where we scroll to.
        if matches!(relayout, Relayout::Slide(_)) && shown != selection {
            self.view.view(cx, ids!(bubbles)).set_scroll_pos(cx, DVec2::default());
        }

        // The X goes first, then the active bubbles in the order they were activated, then the rest.
        let show_invites_in_unread_filters = cx.global::<AppPreferencesGlobal>().0.show_invites_in_unread_filters;
        let order: Vec<LiveId> = (!selection.is_empty()).then_some(id!(clear)).into_iter()
            .chain(self.selection_order.iter().filter_map(|active_filter| {
                BUBBLES.iter().find(|(_, filter, _)| filter == active_filter).map(|(id, ..)| *id)
            }))
            .chain(BUBBLES.iter()
                .filter(|(_, filter, _)| !selection.active().contains(*filter)
                    && !selection.is_pointless(*filter, show_invites_in_unread_filters)
                    && (*filter != BubbleFilters::Invites || self.has_invites)
                )
                .map(|(id, ..)| *id)
            )
            .collect();

        let row = self.view.view(cx, ids!(row));
        if let Some(mut row) = row.borrow_mut() {
            for (id, slot) in &row.children {
                let is_shown = order.contains(id);
                // A bubble that hides while it's pressed or hovered would come back still looking that way.
                if !is_shown && slot.visible() {
                    slot.button(cx, ids!(bubble)).reset_hover(cx);
                }
                slot.set_visible(cx, is_shown);
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
                // The bubbles that were just activated or deactivated slide over the others.
                let toggled = shown.active() ^ selection.active();
                slides.sort_by_key(|slide| BUBBLES.iter().any(|(id, filter, _)| *id == slide.slot_id && toggled.contains(*filter)));
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

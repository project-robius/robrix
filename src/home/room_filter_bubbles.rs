//! `RoomFilterBubbles`: a row of filter bubbles above the rooms list, like "Unreads" and "People".
//!
//! Selecting a bubble shows only the rooms that match it, and selecting several shows only the rooms that match all of them.
//! Selecting a bubble also hides the others that can't be combined with it, like "Rooms" while "People" is selected.

use bitflags::bitflags;
use makepad_widgets::*;
use matrix_sdk::ruma::events::tag::TagName;

use crate::{
    home::rooms_list::SIDE_PADDING,
    room::room_display_filter::FilterableRoom,
    shared::styles::apply_lavender_button_style,
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
        // Tapping one shouldn't take the key focus away from the filter bar, which would hide the soft keyboard.
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
            View {
                width: Fit, height: Fit
                flow: Right, spacing: 6
                padding: Inset{left: #(SIDE_PADDING), right: #(SIDE_PADDING)}
                unreads := mod.widgets.RoomFilterBubble { text: "Unreads" }
                mentions := mod.widgets.RoomFilterBubble { text: "Mentions" }
                people := mod.widgets.RoomFilterBubble { text: "People" }
                rooms := mod.widgets.RoomFilterBubble { text: "Rooms" }
                favorites := mod.widgets.RoomFilterBubble { text: "Favorites" }
                invites := mod.widgets.RoomFilterBubble { text: "Invites" }
                low_priority := mod.widgets.RoomFilterBubble { text: "Low Priority" }
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
const EXCLUSIVE_GROUPS: [BubbleFilters; 3] = [
    // Mentions are always unread, and the unread filters show either all invites or none of them.
    BubbleFilters::Unreads.union(BubbleFilters::Mentions).union(BubbleFilters::Invites),
    BubbleFilters::People.union(BubbleFilters::Rooms),
    // Invites don't have tags, and the SDK never lets a room be both a favorite and low priority.
    BubbleFilters::Favorites.union(BubbleFilters::LowPriority).union(BubbleFilters::Invites),
];

/// Each bubble's ID and its filter.
const BUBBLES: [(LiveId, BubbleFilters); 7] = [
    (id!(unreads), BubbleFilters::Unreads),
    (id!(mentions), BubbleFilters::Mentions),
    (id!(people), BubbleFilters::People),
    (id!(rooms), BubbleFilters::Rooms),
    (id!(favorites), BubbleFilters::Favorites),
    (id!(invites), BubbleFilters::Invites),
    (id!(low_priority), BubbleFilters::LowPriority),
];

/// Actions emitted by or sent to the [`RoomFilterBubbles`].
#[derive(Debug)]
pub enum RoomFilterBubblesAction {
    /// The selected bubbles changed, so these filters now apply.
    Changed(BubbleFilters),
    /// Request to deselect all bubbles, which then emits an empty `Changed` action.
    Clear,
}

/// A row of toggleable filter bubbles that the rooms list shows above its rooms.
///
/// See the module-level docs for more detail.
#[derive(Script, Widget)]
pub struct RoomFilterBubbles {
    #[deref] view: View,

    #[rust] selected: BubbleFilters,
    /// The selection that the bubbles' styles and visibility show, which a reload resets to none.
    #[rust] shown_selection: BubbleFilters,
}

impl ScriptHook for RoomFilterBubbles {
    fn on_after_apply(&mut self, _vm: &mut ScriptVm, apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        if apply.is_reload() {
            self.shown_selection = BubbleFilters::empty();
        }
    }
}

impl Widget for RoomFilterBubbles {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let bubble_actions = cx.capture_actions(|cx| self.view.handle_event(cx, event, scope));
        let mut selected = self.selected;
        if !bubble_actions.is_empty() {
            for (id, filter) in BUBBLES {
                if self.view.button(cx, &[id]).clicked(&bubble_actions) {
                    selected.toggle(filter);
                }
            }
        }
        if let Event::Actions(actions) = event
            && actions.iter().any(|action| matches!(action.downcast_ref(), Some(RoomFilterBubblesAction::Clear)))
        {
            selected = BubbleFilters::empty();
        }
        if selected != self.selected {
            self.selected = selected;
            cx.action(RoomFilterBubblesAction::Changed(selected));
            self.redraw(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let (shown, selected) = (self.shown_selection, self.selected);
        if shown != selected {
            let (was_hidden, hidden) = (shown.excluded(), selected.excluded());
            for (id, filter) in BUBBLES {
                let mut bubble = self.view.button(cx, &[id]);
                if shown.contains(filter) != selected.contains(filter) {
                    apply_lavender_button_style(cx, &mut bubble, selected.contains(filter));
                }
                if was_hidden.contains(filter) != hidden.contains(filter) {
                    bubble.set_visible(cx, !hidden.contains(filter));
                }
            }
            self.shown_selection = selected;
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

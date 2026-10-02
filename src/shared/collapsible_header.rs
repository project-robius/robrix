//! This module defines a collapsible header wrapper with a triangle icon
//! that indicates whether the header is expanded or collapsed.
//!
//! This widget can be clicked to toggle between expanded and collapsed.
//!
//! The collapsible header is *just* the header, it doesn't actually contain any content.
//! This design is necessary because the header is drawn within a PortalList,
//! and its content is also drawn within that PortalList separately from its content.

use makepad_widgets::*;
use makepad_widgets::animator::Animate;


use super::expand_arrow::ExpandArrow;
use super::unread_badge::UnreadBadgeWidgetRefExt as _;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // Calm section-group header (no longer the legacy purple pill). The label is
    // a secondary-tone section title on a transparent background; the collapse
    // arrow is a faint tertiary chevron.
    mod.widgets.COLOR_HEADER_FG = #x5A6B86;

    mod.widgets.COLOR_HEADER_BG = #xF7F9FC;

    mod.widgets.CollapsibleHeader = set_type_default() do #(CollapsibleHeader::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill,
        height: 34,
        align: Align{ x: 0.0, y: 0.5 },
        margin: Inset{top: 6, bottom: 2, left: 0, right: 0},
        padding: Inset{left: 4, right: 4, top: 0, bottom: 0}
        flow: Right,

        cursor: MouseCursor.Hand,
        // Transparent group header — it sits directly on the page canvas.
        show_bg: false,
        draw_bg +: {
            border_radius: 0.0
        }

        collapse_icon := mod.widgets.ExpandArrow {
            width: 18, height: 18,
            margin: Inset{left: 2, right: 6, top: 0, bottom: 0},
            draw_bg.color: #x687283
        }

        label := Label {
            padding: 0,
            width: Fill,
            height: Fit,
            flow: Flow.Right { wrap: false }
            text: "",
            text_overflow: TextOverflow.Ellipsis,
            max_lines: 1
            draw_text +: {
                // Lighter weight than the bold section-title token — calmer group label.
                text_style: REGULAR_TEXT { font_size: 12.5 },
                color: #x5A6B86,
            }
        }

        unread_badge := UnreadBadge {
            // Bottom margin nudges the badge up so its center lines up with the
            // group label's glyphs (the label's Fit box sits low due to descent).
            margin: Inset{right: 5.5, bottom: 4},
        }
    }
}

/// The categories of collapsible headers in the rooms list.
#[derive(Copy, Clone, Debug, Default)]
pub enum HeaderCategory {
    /// Rooms the user has been invited to but has not yet joined.
    Invites,
    /// Joined rooms that the user has marked as favorites.
    Favorites,
    /// Joined rooms that are direct messages with other users.
    DirectRooms,
    /// Joined rooms that are not direct messages or favorites.
    RegularRooms,
    /// Joined rooms that the user has marked as low priority.
    LowPriority,
    /// Rooms that the user has left.
    LeftRooms,
    #[default]
    None,
}
impl HeaderCategory {
    fn as_str(&self) -> &'static str {
        match self {
            HeaderCategory::Invites => "Invites",
            HeaderCategory::Favorites => "Favorites",
            HeaderCategory::RegularRooms => "Rooms",
            HeaderCategory::DirectRooms => "People",
            HeaderCategory::LowPriority => "Low Priority",
            HeaderCategory::LeftRooms => "Left Rooms",
            HeaderCategory::None => "",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum CollapsibleHeaderAction {
    /// The header was clicked to toggled its expanded/collapsed state.
    Toggled {
        category: HeaderCategory,
    },
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct CollapsibleHeader {
    #[deref] view: View,
    #[rust(true)] is_expanded: bool,
    #[rust] category: HeaderCategory,
    #[rust] num_unread_mentions: u64,
    #[rust] num_unread_messages: u64,
}

impl Widget for CollapsibleHeader {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Handle hits on this view as a whole before passing the event to the inner view.
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(..) => {
                cx.set_key_focus(self.view.area());
            }
            Hit::FingerUp(fe)
                if fe.is_over && fe.is_primary_hit() && fe.was_tap() =>
            {
                self.toggle_collapse(cx, scope);
            }
            _ => { }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Set arrow and label state during draw to ensure child widgets are available.
        if let Some(mut arrow) = self.view.child_by_path(ids!(collapse_icon)).borrow_mut::<ExpandArrow>() {
            arrow.set_is_open_no_animate(self.is_expanded);
        }
        self.view.child_by_path(ids!(label)).set_text(cx, self.category.as_str());
        self.view.child_by_path(ids!(unread_badge))
            .as_unread_badge()
            .update_counts(false, self.num_unread_mentions, self.num_unread_messages);
        self.view.draw_walk(cx, scope, walk)
    }
}

impl CollapsibleHeader {
    fn toggle_collapse(&mut self, cx: &mut Cx, _scope: &mut Scope) {
        self.is_expanded = !self.is_expanded;
        if let Some(mut arrow) = self.view.child_by_path(ids!(collapse_icon)).borrow_mut::<ExpandArrow>() {
            arrow.set_is_open(cx, self.is_expanded, Animate::Yes);
        }
        self.redraw(cx);
        cx.widget_action(
            self.widget_uid(), 
            CollapsibleHeaderAction::Toggled {
                category: self.category,
            },
        );
    }
}

impl CollapsibleHeaderRef {
    /// Sets the category and expanded state of the header.
    pub fn set_details(
        &self,
        _cx: &mut Cx,
        is_expanded: bool,
        category: HeaderCategory,
        num_unread_mentions: u64,
        num_unread_messages: u64,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.is_expanded = is_expanded;
            inner.category = category;
            inner.num_unread_mentions = num_unread_mentions;
            inner.num_unread_messages = num_unread_messages;
        }
    }
}

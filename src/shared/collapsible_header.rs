//! This module defines a collapsible header wrapper with a triangle icon
//! that indicates whether the header is expanded or collapsed.
//!
//! This widget can be clicked to toggle between expanded and collapsed.
//!
//! The collapsible header is *just* the header, it doesn't actually contain any content.
//! This design is necessary because the header is drawn within a PortalList,
//! and its content is also drawn within that PortalList separately from its content.

use makepad_widgets::*;

use crate::home::rooms_list::RoomsListScopeProps;

use super::unread_badge::UnreadBadgeWidgetExt;

live_design! {
    use link::theme::*;
    use link::widgets::*;
    use link::shaders::*;

    use crate::shared::styles::*;
    use crate::shared::unread_badge::*;

    ICON_COLLAPSE = dep("crate://self/resources/icons/triangle_fill.svg")

    COLOR_HEADER_FG = #F;
    COLOR_HEADER_BG = (COLOR_ROBRIX_PURPLE); // the purple color from the Robrix logo


    pub CollapsibleHeader = {{CollapsibleHeader}}<RoundedView> {
        width: Fill,
        height: 35,
        align: { x: 0.0, y: 0.5 },
        margin: {top: 3, bottom: 3, left: 0, right: 0},
        padding: 5
        flow: Right,

        cursor: Hand,
        draw_bg: {
            border_radius: 4.0,
            color: (COLOR_HEADER_BG)
        }

        collapse_icon = <IconRotated> {
            margin: {left: 5, right: 8, top: 0, bottom: 0},
            draw_icon: {
                svg_file: (ICON_COLLAPSE),
                rotation_angle: 180.0, // start in the "expanded" state
                color: (COLOR_HEADER_FG),
            }
            icon_walk: { width: 14, height: Fit, margin: 0, }
        }
        label = <Label> {
            padding: 0,
            width: Fill,
            height: Fit,
            text: "",
            draw_text: {
                text_style: <REGULAR_TEXT>{font_size: 11},
                color: (COLOR_HEADER_FG),
            }
        }
        unread_badge = <UnreadBadge> {
            margin: {right: 5.5},
        }
    }
}

/// The categories of collapsible headers in the rooms list.
#[derive(Copy, Clone, Debug, DefaultNone)]
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

#[derive(Clone, Debug, DefaultNone)]
pub enum CollapsibleHeaderAction {
    /// The header was clicked to toggled its expanded/collapsed state.
    Toggled {
        category: HeaderCategory,
    },
    None,
}

#[derive(Live, LiveHook, Widget)]
pub struct CollapsibleHeader {
    #[deref] view: View,
    #[rust(true)] is_expanded: bool,
    #[rust] category: HeaderCategory,
}

impl Widget for CollapsibleHeader {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Handle hits on this view as a whole before passing the event to the inner view.
        let rooms_list_props = scope.props.get::<RoomsListScopeProps>().unwrap();
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(..) => {
                cx.set_key_focus(self.view.area());
            }
            Hit::FingerUp(fe) => {
                if !rooms_list_props.was_scrolling && fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                    self.toggle_collapse(cx, scope);
                }
            }
            _ => { }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let angle = if self.is_expanded {
            180.0
        } else {
            90.0
        };
        self.icon(ids!(collapse_icon)).apply_over(
            cx,
            live! {
                draw_icon: { rotation_angle: (angle) }
            },
        );
        self.view.draw_walk(cx, scope, walk)
    }
}

impl CollapsibleHeader {
    fn toggle_collapse(&mut self, cx: &mut Cx, scope: &mut Scope) {
        self.is_expanded = !self.is_expanded;
        self.redraw(cx);
        cx.widget_action(
            self.widget_uid(),
            &scope.path,
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
        cx: &mut Cx,
        is_expanded: bool,
        category: HeaderCategory,
        num_unread_mentions: u64,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.is_expanded = is_expanded;
            inner.category = category;
            inner.label(ids!(label)).set_text(cx, category.as_str());
            inner.unread_badge(ids!(unread_badge)).update_counts(num_unread_mentions, 0);
        }
    }
}

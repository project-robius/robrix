//! The RoomsSideBar is the widget that contains the RoomsList and other items.
//!
//! It differs in what content it includes based on the adaptive view:
//! * On a narrow mobile view, it acts as the root_view of StackNavigation
//!   * It includes a title label, a search bar, and the RoomsList.
//! * On a wide desktop view, it acts as a permanent tab that is on the left side of the dock.
//!   * It only includes a title label and the RoomsList, because the SearcBar
//!     is at the top of the HomeScreen in Desktop view.

use makepad_widgets::*;

use crate::home::rooms_list::RoomsListWidgetExt;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::room_filter_input_bar::RoomFilterInputBar;
    use crate::home::search_messages::*;
    use crate::home::rooms_list::RoomsList;
    use crate::home::rooms_list_header::RoomsListHeader;

    pub RoomsSideBar = {{RoomsSideBar}}<AdaptiveView> {
        Desktop = <View> {
            padding: {top: 20, left: 10, right: 10}
            flow: Down, spacing: 10
            width: Fill, height: Fill

            show_bg: true,
            draw_bg: {
                instance bg_color: (COLOR_PRIMARY_DARKER)
                instance border_color: #f2f2f2
                instance border_size: 0.003

                // Draws a right-side border
                fn pixel(self) -> vec4 {
                    if self.pos.x > 1.0 - self.border_size {
                        return self.border_color;
                    } else {
                        return self.bg_color;
                    }
                }
            }

            <CachedWidget> {
                rooms_list_header = <RoomsListHeader> {}
            }
            <CachedWidget> {
                rooms_list = <RoomsList> {}
            }
        },

        Mobile = <View> {
            padding: {top: 17, left: 17, right: 17}
            flow: Down, spacing: 7
            width: Fill, height: Fill

            <CachedWidget> {
                rooms_list_header = <RoomsListHeader> {}
            }

            <View> {
                width: Fill,
                height: 39,
                flow: Right
                padding: {top: 2, bottom: 2}
                spacing: 5 
                align: {y: 0.5}

                <CachedWidget> {
                    room_filter_input_bar = <RoomFilterInputBar> {}
                }

                search_messages_button = <SearchMessagesButton> { }
            }

            <CachedWidget> {
                rooms_list = <RoomsList> {}
            }
        }
    }
}

/// A simple wrapper around `AdaptiveView` that contains several global singleton widgets.
///
/// * In the mobile view, it serves as the root view of the StackNavigation,
///   showing the title label, the search bar, and the RoomsList.
/// * In the desktop view, it is a permanent tab in the dock,
///   showing only the title label and the RoomsList
///   (because the search bar is at the top of the HomeScreen).
#[derive(Live, Widget)]
pub struct RoomsSideBar {
    #[deref] view: AdaptiveView,
}

impl LiveHook for RoomsSideBar {
    fn after_new_from_doc(&mut self, cx: &mut Cx) {
        // Here we set the global singleton for the RoomsList widget,
        // which is used to access the list of rooms from anywhere in the app.
        Cx::set_global(cx, self.view.rooms_list(ids!(rooms_list)));
    }
}

impl Widget for RoomsSideBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}


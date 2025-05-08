//! The RoomsSideBar is the widget that contains the RoomsList and other items.
//!
//! It differs in what content it includes based on the adaptive view:
//! * On a narrow mobile view, it acts as the root_view of StackNavigation
//!   * It includes a title label, a search bar, and the RoomsList.
//! * On a wide desktop view, it acts as a permanent tab that is on the left side of the dock.
//!   * It only includes a title label and the RoomsList, because the SearcBar
//!     is at the top of the HomeScreen in Desktop view.

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::room_filter_input_bar::RoomFilterInputBar;

    use crate::home::rooms_list::RoomsList;

    pub RoomsSideBar = <AdaptiveView> {
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

            sidebar_title = <Label> {
                flow: Right, // do not wrap
                text: "All Rooms"
                draw_text: {
                    color: #x0
                    text_style: <TITLE_TEXT>{}
                }
            }
            <CachedWidget> {
                rooms_list = <RoomsList> {}
            }
        },

        Mobile = <View> {
            padding: {top: 17, left: 17, right: 17}
            flow: Down, spacing: 7
            width: Fill, height: Fill

            sidebar_title = <Label> {
                text: "All Rooms"
                flow: Right, // do not wrap
                draw_text: {
                    color: #x0
                    text_style: <TITLE_TEXT>{}
                }
            }
            <CachedWidget> {
                <RoomFilterInputBar> {
                    draw_bg: {
                        border_size: 1.0,
                    }
                }
            }
            <CachedWidget> {
                rooms_list = <RoomsList> {}
            }
        }
    }
}

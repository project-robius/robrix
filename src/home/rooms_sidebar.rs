//! The RoomsSideBar is the widget that contains the RoomsList and other items.
//!
//! It differs in what content it includes based on the adaptive view:
//! * On a narrow mobile view, it acts as the root_view of StackNavigation
//!   * It includes a title label and the RoomsList.
//! * On a wide desktop view, it acts as a permanent tab that is on the left side of the dock.
//!   * It only includes a title label and the RoomsList.

use makepad_widgets::*;

use crate::home::rooms_list::RoomsListWidgetExt;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.RoomsSideBar = #(RoomsSideBar::register_widget(vm)) {
        Desktop := SolidView {
            padding: Inset{top: 20, left: 10, right: 10}
            flow: Down, spacing: 5
            width: Fill, height: Fill

            draw_bg.color: (COLOR_PRIMARY_DARKER)

            CachedWidget {
                rooms_list_header := RoomsListHeader {}
            }
            CachedWidget {
                rooms_list := RoomsList {}
            }
        },

        Mobile := View {
            width: Fill, height: Fill
            flow: Down,

            RoundedShadowView {
                width: Fill, height: Fit
                padding: Inset{top: 15, left: 15, right: 15, bottom: 10}
                flow: Down,

                show_bg: true
                draw_bg +: {
                    color: (COLOR_PRIMARY_DARKER)
                    border_radius: 4.0
                    border_size: 0.0
                    shadow_color: #0005
                    shadow_radius: 15.0
                    shadow_offset: vec2(1.0, 0.0)
                }

                View { height: 23 }

                CachedWidget {
                    rooms_list_header := RoomsListHeader {}
                }
            }

            View {
                padding: Inset{left: 15, right: 15}

                CachedWidget {
                    rooms_list := RoomsList {}
                }
            }
        }
    }
}

/// A simple wrapper around `AdaptiveView` that contains several global singleton widgets.
///
/// * In the mobile view, it serves as the root view of the StackNavigation,
///   showing the title label and the RoomsList.
/// * In the desktop view, it is a permanent tab in the dock,
///   showing only the title label and the RoomsList
///   while search is provided by the navigation side bar.
#[derive(Script, Widget)]
pub struct RoomsSideBar {
    #[deref]
    view: AdaptiveView,
}

impl ScriptHook for RoomsSideBar {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            // Here we set the global singleton for the RoomsList widget,
            // which is used to access the list of rooms from anywhere in the app.
            cx.set_global(self.view.rooms_list(cx, ids!(rooms_list)));
        });
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

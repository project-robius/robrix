//! The RoomsSideBar is the widget that contains the RoomsList and other items.
//!
//! It differs in what content it includes based on the adaptive view:
//! * On a narrow mobile view, it acts as the root_view of StackNavigation
//!   * It only includes the RoomsList, because the `HideOnScrollView`
//!     draws the title label, search bar, and filter bubbles over the top of it.
//! * On a wide desktop view, it acts as a permanent tab that is on the left side of the dock.
//!   * It only includes a title label, the filter bubbles, and the RoomsList, because the SearcBar
//!     is at the top of the HomeScreen in Desktop view.

use makepad_widgets::*;

use crate::home::rooms_list::{SIDE_PADDING, RoomsListWidgetExt};
use crate::settings::app_preferences::{AppPreferencesGlobal, AppPreferencesAction, ViewModeOverride};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.RoomsSideBar = #(RoomsSideBar::register_widget(vm)) {
        Desktop := SolidView {
            padding: Inset{top: 20, left: #(SIDE_PADDING)}
            flow: Down, spacing: 5
            width: Fill, height: Fill

            draw_bg.color: (COLOR_PRIMARY_DARKER)

            // Matches the right padding that the rooms list has itself.
            View {
                width: Fill, height: Fit
                padding: Inset{right: #(SIDE_PADDING)}
                CachedWidget {
                    rooms_list_header := RoomsListHeader {}
                }
            }
            // Reaches out to the sidebar's left edge, so the bubbles scroll out of view right at its edge.
            View {
                width: Fill, height: Fit
                margin: Inset{left: #(-SIDE_PADDING)}
                CachedWidget {
                    room_filter_bubbles := RoomFilterBubbles {}
                }
            }
            CachedWidget {
                rooms_list := RoomsList {}
            }
        },

        Mobile := View {
            width: Fill, height: Fill
            padding: Inset{left: #(SIDE_PADDING)}

            CachedWidget {
                rooms_list := RoomsList {}
            }
        }
    }
}

/// A simple wrapper around `AdaptiveView` that contains several global singleton widgets.
///
/// * In the mobile view, it serves as the root view of the StackNavigation,
///   showing only the RoomsList (see [`HideOnScrollView`](crate::home::hide_on_scroll::HideOnScrollView)).
/// * In the desktop view, it is a permanent tab in the dock,
///   showing only the title label, the filter bubbles, and the RoomsList
///   (because the search bar is at the top of the HomeScreen).
#[derive(Script, Widget)]
pub struct RoomsSideBar {
    #[deref] view: AdaptiveView,

    /// The most recently applied view-mode override.
    #[rust] applied_view_mode: ViewModeOverride,
}

impl ScriptHook for RoomsSideBar {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            // Here we set the global singleton for the RoomsList widget,
            // which is used to access the list of rooms from anywhere in the app.
            cx.set_global(self.view.rooms_list(cx, ids!(rooms_list)));

            // The RoomsSideBar is re-instantiated every time the HomeScreen's
            // AdaptiveView switches between Desktop and Mobile view modes
            // (cuz it's not wrapped in a CachedWidget).
            // Thus we just re-read the current value here and apply it.
            let mode = cx.global::<AppPreferencesGlobal>().0.view_mode;
            self.apply_view_mode(mode);
        });
    }
}

impl RoomsSideBar {
    fn apply_view_mode(&mut self, mode: ViewModeOverride) {
        self.view.set_variant_selector(mode.variant_selector());
        self.applied_view_mode = mode;
    }
}

impl Widget for RoomsSideBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(AppPreferencesAction::ViewModeChanged(new_mode)) = action.downcast_ref() {
                    if *new_mode != self.applied_view_mode {
                        self.apply_view_mode(*new_mode);
                        self.view.redraw(cx);
                    }
                }
            }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

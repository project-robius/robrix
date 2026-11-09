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
            flow: Down
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
                margin: Inset{top: 5, left: #(-SIDE_PADDING)}
                CachedWidget {
                    room_filter_bubbles := RoomFilterBubbles {}
                }
            }
            // The rooms list goes first, so the header's rounded bottom edge and shadow get drawn over it.
            // This doesn't clip sideways, so they still reach the sidebar's left edge.
            View {
                width: Fill, height: Fill
                padding: Inset{top: 10}
                flow: Overlay
                clip_x: false

                CachedWidget {
                    rooms_list := RoomsList {}
                }
                // About as tall as the header, and clipped so only its rounded bottom edge shows below the bubbles.
                View {
                    width: Fill, height: 90
                    margin: Inset{top: -90, left: #(-SIDE_PADDING)}
                    clip_x: false, clip_y: false
                    show_bg: true
                    draw_bg: mod.widgets.RobrixHeaderBackground {}
                }
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
    /// Whether the desktop dock had collapsed us to zero width when we were last drawn.
    #[rust] is_collapsed: bool,
    /// How much of our full width the desktop dock shows while it animates us collapsing or expanding.
    #[rust] visible_fraction: Option<f64>,
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
        // A hidden widget of ours could still have the key focus, but it shouldn't act on keys.
        if self.is_collapsed && matches!(event, Event::KeyDown(_) | Event::KeyUp(_)) {
            return;
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, mut walk: Walk) -> DrawStep {
        // When the desktop dock collapses us to zero width, we don't draw anything,
        // which also keeps Tab from moving the key focus to any of our hidden widgets.
        let width = cx.peek_walk_turtle(walk).size.x;
        self.is_collapsed = width <= 0.0;
        if self.is_collapsed {
            return DrawStep::done();
        }
        // While the dock animates our width, we lay out our content at its full width
        // and slide it past the dock's left edge, so it doesn't reflow on every frame.
        if let Some(visible_fraction) = self.visible_fraction {
            let full_width = width / visible_fraction;
            walk.width = Size::Fixed(full_width);
            walk.margin.left -= full_width - width;
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomsSideBarRef {
    /// Sets how much of our full width the desktop dock shows while it animates us, or `None` once it's done.
    pub fn set_visible_fraction(&self, visible_fraction: Option<f64>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible_fraction = visible_fraction;
        }
    }
}

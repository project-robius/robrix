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
                    shadow_radius: 12.0
                    shadow_offset: vec2(0.0, 0.0)

                    pixel: fn() {
                        let sdf = Sdf2d.viewport(self.pos * self.rect_size3)

                        let mut fill_color = self.color
                        if self.color_2.x > -0.5 {
                            let dither = Math.random_2d(self.pos.xy) * 0.04 * self.color_dither
                            let dir = if self.gradient_fill_horizontal > 0.5 self.pos.x else self.pos.y
                            fill_color = mix(self.color self.color_2 dir + dither)
                        }

                        let mut stroke_color = self.border_color
                        if self.border_color_2.x > -0.5 {
                            let dither = Math.random_2d(self.pos.xy) * 0.04 * self.color_dither
                            let dir = if self.gradient_border_horizontal > 0.5 self.pos.x else self.pos.y
                            stroke_color = mix(self.border_color self.border_color_2 dir + dither)
                        }

                        sdf.box(
                            self.sdf_rect_pos.x
                            self.sdf_rect_pos.y
                            self.sdf_rect_size.x
                            self.sdf_rect_size.y
                            max(1.0 self.border_radius)
                        )
                        if sdf.shape > -1.0 {
                            let m = self.shadow_radius
                            let o = self.shadow_offset + self.rect_shift
                            let v = GaussShadow.rounded_box_shadow(vec2(m) + o self.rect_size2+o self.pos * (self.rect_size3+vec2(m)) self.shadow_radius*0.5 self.border_radius*2.0)
                            // Only draw shadow on the bottom half of the view
                            let pixel_y = self.pos.y * self.rect_size3.y
                            let mid_y = self.sdf_rect_pos.y + self.sdf_rect_size.y * 0.5
                            let bottom_mask = smoothstep(mid_y - m * 0.3 mid_y + m * 0.3 pixel_y)
                            sdf.clear(self.shadow_color * v * bottom_mask)
                        }

                        sdf.fill_keep(fill_color)

                        if self.border_size > 0.0 {
                            sdf.stroke(stroke_color self.border_size)
                        }
                        return sdf.result
                    }
                }

                View { height: 23 }

                CachedWidget {
                    rooms_list_header := RoomsListHeader {}
                }

                View {
                    width: Fill,
                    height: 45,
                    flow: Right
                    padding: Inset{top: 5, bottom: 2}
                    spacing: 5 
                    align: Align{y: 0.5}

                    CachedWidget {
                        room_filter_input_bar := RoomFilterInputBar {}
                    }

                    search_messages_button := SearchMessagesButton { }
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
///   showing the title label, the search bar, and the RoomsList.
/// * In the desktop view, it is a permanent tab in the dock,
///   showing only the title label and the RoomsList
///   (because the search bar is at the top of the HomeScreen).
#[derive(Script, Widget)]
pub struct RoomsSideBar {
    #[deref] view: AdaptiveView,
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

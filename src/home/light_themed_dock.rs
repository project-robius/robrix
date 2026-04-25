use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RobrixSplitter = Splitter {
        // size: theme.splitter_size
        // min_horizontal: theme.splitter_min_horizontal
        // max_horizontal: theme.splitter_max_horizontal
        // min_vertical: theme.splitter_min_vertical
        // max_vertical: theme.splitter_max_vertical

        draw_bg +: {
            color: COLOR_SECONDARY
            color_hover: COLOR_ROBRIX_PURPLE
            color_drag: COLOR_ROBRIX_PURPLE

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)

                // Body: dark gray by default (matches the default dark theme's
                // `color_bg_app`), transitions to purple on hover/drag.
                // Mildly rounded corners soften the edges where panels meet.
                let body_color = mix(
                    #4D4D4D
                    mix(self.color_hover, self.color_drag, self.drag)
                    self.hover
                )
                sdf.box(
                    0.0,
                    0.0,
                    self.rect_size.x,
                    self.rect_size.y,
                    1.5
                )
                sdf.fill(body_color)

                // Draw the grab bar shape
                if self.is_vertical > 0.5 {
                    sdf.box(
                        self.splitter_pad
                        self.rect_size.y * 0.5 - self.bar_size * 0.5
                        self.rect_size.x - 2.0 * self.splitter_pad
                        self.bar_size
                        self.border_radius
                    )
                }
                else {
                    sdf.box(
                        self.rect_size.x * 0.5 - self.bar_size * 0.5
                        self.splitter_pad
                        self.bar_size
                        self.rect_size.y - 2.0 * self.splitter_pad
                        self.border_radius
                    )
                }

                // Grab bar: white when hovered/dragged, otherwise matches body
                let grab_color = mix(self.color, #fff, self.hover)
                return sdf.fill_keep(grab_color)
            }
        }

        animator: Animator{
            hover: {
                default: @off
                off: AnimatorState{
                    from: {all: Forward {duration: 0.1}}
                    apply: {
                        draw_bg: {drag: 0.0, hover: 0.0}
                    }
                }

                on: AnimatorState{
                    from: {
                        all: Forward {duration: 0.1}
                        drag: Forward {duration: 0.01}
                    }
                    apply: {
                        draw_bg: {
                            drag: 0.0,
                            hover: snap(1.0)
                        }
                    }
                }

                drag: AnimatorState{
                    from: { all: Forward { duration: 0.1 }}
                    apply: {
                        draw_bg: {
                            drag: snap(1.0),
                            hover: 1.0
                        }
                    }
                }
            }
        }
    }

    mod.widgets.RobrixTabCloseButton = TabCloseButton {
        height: 10.0
        width: 10.0
        margin: Inset{ right: theme.space_2, left: -1 }
        draw_button +: {
            color: #0
            color_hover: #FE8610
            color_active: #FE8610
        }

        animator: Animator{
            hover: {
                default: @off
                off: AnimatorState{
                    from: {all: Forward {duration: 0.1}}
                    apply: {
                        draw_button: {hover: 0.0}
                    }
                }

                on: AnimatorState{
                    cursor: MouseCursor.Hand
                    from: {all: Snap}
                    apply: {
                        draw_button: {hover: 1.0}
                    }
                }
            }
        }
    }

    mod.widgets.RobrixTab = Tab {
        width: Fit
        height: Fill

        align: Align{x: 0.0, y: 0.5}
        padding: 9
        margin: 0

        close_button: mod.widgets.RobrixTabCloseButton {}
        draw_text +: {
            text_style: theme.font_regular {}

            color: #000
            color_hover: #fe8610
            color_active: COLOR_PRIMARY
        }

        draw_bg +: {
            // Light blue-ish color, de-saturated from COLOR_ACTIVE_PRIMARY
            color: #E1EEFA
            // A slightly darker shade of the tab color for hover visibility
            color_hover: #C8DDEF
            color_active: COLOR_ACTIVE_PRIMARY
            // Remove the border and rounded corners from the default Tab style
            border_size: 0.0
            border_radius: 3.0
        }

        animator: Animator{
            hover: {
                default: @off
                off: AnimatorState{
                    from: {all: Forward {duration: 0.2}}
                    apply: {
                        draw_bg: {hover: 0.0}
                        draw_text: {hover: 0.0}
                    }
                }

                on: AnimatorState{
                    cursor: MouseCursor.Hand
                    from: {all: Forward {duration: 0.1}}
                    apply: {
                        draw_bg: {hover: snap(1.0)}
                        draw_text: {hover: snap(1.0)}
                    }
                }
            }

            active: {
                default: @off
                off: AnimatorState{
                    from: {all: Forward {duration: 0.3}}
                    apply: {
                        close_button: {draw_button: {active: 0.0}}
                        draw_bg: {active: 0.0}
                        draw_text: {active: 0.0}
                    }
                }

                on: AnimatorState{
                    from: {all: Snap}
                    apply: {
                        close_button: {draw_button: {active: 1.0}}
                        draw_bg: {active: 1.0}
                        draw_text: {active: 1.0}
                    }
                }
            }
        }
    }

    mod.widgets.RobrixTabBar = TabBar {
        CloseableTab := mod.widgets.RobrixTab {closeable: true}
        PermanentTab := mod.widgets.RobrixTab {closeable: false}

        draw_drag +: {
            draw_depth: 10
            color: #x0
        }
        draw_fill +: {
            color: COLOR_PRIMARY * 0.96
        }
        draw_bg +: {
            color: COLOR_PRIMARY * 0.96
        }

        width: Fill
        height: max(theme.tab_height, 25.)

        scroll_bars: ScrollBarsTabs {
            show_scroll_x: true
            show_scroll_y: false
            scroll_bar_x +: {
                bar_size: 4
                use_vertical_finger_scroll: true
            }
        }
    }

    mod.widgets.RobrixDock = Dock {
        flow: Down

        round_corner +: {
            color: COLOR_SECONDARY
        }

        padding: Inset{left: theme.dock_border_size, top: 0, right: theme.dock_border_size, bottom: theme.dock_border_size}
        drag_target_preview +: {
            draw_depth: 10.0
            color: mix(COLOR_ACTIVE_PRIMARY, #FFFFFF00, 0.5)
        }
        tab_bar: mod.widgets.RobrixTabBar {}
        splitter: mod.widgets.RobrixSplitter {}
    }
}

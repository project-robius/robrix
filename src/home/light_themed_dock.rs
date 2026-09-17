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
        height: 30.0
        width: 30.0
        margin: Inset{left: -34, right: 4}
        draw_button +: {
            color: #0
            color_active: COLOR_PRIMARY

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(1.0, 1.0, self.rect_size.x - 2.0, self.rect_size.y - 2.0, 4.0)
                sdf.fill(mix(#0000001f, #ffffff33, self.active) * self.hover)

                let mid = self.rect_size * 0.5
                let radius = 4.0
                sdf.move_to(mid.x - radius, mid.y - radius)
                sdf.line_to(mid.x + radius, mid.y + radius)
                sdf.move_to(mid.x - radius, mid.y + radius)
                sdf.line_to(mid.x + radius, mid.y - radius)
                return sdf.stroke(mix(self.color, self.color_active, self.active), 1.5)
            }
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
        width: Fit{max: FitBound.Abs(260)}
        height: Fill

        align: Align{x: 0.0, y: 0.5}
        // This padding accounts for the close button on the left of the tab
        // such that the room name label is centered.
        padding: Inset{left: 38, right: 9}
        margin: 0

        close_button: mod.widgets.RobrixTabCloseButton {}
        draw_text +: {
            text_style: theme.font_regular {}
            max_lines: 1
            text_overflow: TextOverflow.Ellipsis

            color: #000
            color_active: COLOR_PRIMARY
            get_color: fn() {
                return self.color.mix(self.color_active, self.active)
            }
        }

        draw_bg +: {
            // Light blue-ish color, de-saturated from COLOR_ACTIVE_PRIMARY
            color: #E1EEFA
            color_2: #E1EEFA
            // A slightly darker shade of the tab color for hover visibility
            color_hover: #C8DDEF
            color_2_hover: #C8DDEF
            // Active (selected) tabs are a deeper blue, with a vertical gradient
            // to a slightly lighter blue.
            color_active: #0660FE
            color_2_active: #398CFE
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

        PermanentTab := mod.widgets.RobrixTab {closeable: false, width: Fit, padding: 9}

        RoomTab := mod.widgets.RobrixTab {
            closeable: true
            // Keep the native Tab widget's label, close button, and drag handling,
            // but reserve space within the tab for action buttons to be overlaid.
            padding: Inset {left: 38, right: 37}
        }

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

    let RoomTabActionButton = mod.widgets.RoomActionButton {
        width: 30
        height: 30
        padding: 7
        icon_walk: Walk {width: 15, height: 15}
    }
    mod.widgets.RoomTabActions = View {
        width: 30
        height: Fill
        flow: Right
        align: Align {y: 0.5}
        expand_room_actions_button := RoomTabActionButton {
            draw_icon.svg: mod.widgets.ICON_CHEVRON_DOWN
        }
        collapse_room_actions_button := RoomTabActionButton {
            visible: false
            draw_icon.svg: mod.widgets.ICON_CHEVRON_UP
        }
    }

    mod.widgets.RoomTabs = mod.widgets.RoomTabsBase {
        room_tab_actions: mod.widgets.RoomTabActions {}
        tab_title_measure: mod.widgets.RobrixTab.draw_text
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

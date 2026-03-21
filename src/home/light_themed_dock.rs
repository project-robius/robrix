use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    COLOR_TAB_BAR = (COLOR_PRIMARY * 0.96)
    COLOR_DOCK_TAB = #E1EEFA // a light blue-ish color, de-saturated from `COLOR_ACTIVE_PRIMARY`
    COLOR_DRAG_TARGET = (COLOR_ACTIVE_PRIMARY)

    pub Splitter = <SplitterBase> {
        draw_bg: {
            uniform border_radius: 1.0
            uniform splitter_pad: 1.0
            uniform splitter_grabber: 110.0

            instance down: 0.0
            instance hover: 0.0

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                sdf.clear(COLOR_SECONDARY);

                sdf.box(
                    -1.,
                    -1.,
                    self.rect_size.x + 2,
                    self.rect_size.y + 2,
                    2.5
                );
                // if self.is_vertical > 0.5 {
                //     sdf.box(
                //         self.splitter_pad,
                //         self.rect_size.y * 0.5 - self.splitter_grabber * 0.5,
                //         self.rect_size.x - 2.0 * self.splitter_pad,
                //         self.splitter_grabber,
                //         self.border_radius
                //     );
                // }
                // else {
                //     sdf.box(
                //         self.rect_size.x * 0.5 - self.splitter_grabber * 0.5,
                //         self.splitter_pad,
                //         self.splitter_grabber,
                //         self.rect_size.y - 2.0 * self.splitter_pad,
                //         self.border_radius
                //     );
                // }

                return sdf.fill_keep(mix(
                    COLOR_SECONDARY,
                    COLOR_ROBRIX_PURPLE,
                    self.hover
                ));
            }
        }
        size: (THEME_SPLITTER_SIZE)
        min_horizontal: (THEME_SPLITTER_MIN_HORIZONTAL)
        max_horizontal: (THEME_SPLITTER_MAX_HORIZONTAL)
        min_vertical: (THEME_SPLITTER_MIN_VERTICAL)
        max_vertical: (THEME_SPLITTER_MAX_VERTICAL)

        animator: {
            hover = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.1}}
                    apply: {
                        draw_bg: {down: 0.0, hover: 0.0}
                    }
                }

                on = {
                    from: {
                        all: Forward {duration: 0.1}
                        state_down: Forward {duration: 0.01}
                    }
                    apply: {
                        draw_bg: {
                            down: 0.0,
                            hover: [{time: 0.0, value: 1.0}],
                        }
                    }
                }

                drag = {
                    from: { all: Forward { duration: 0.1 }}
                    apply: {
                        draw_bg: {
                            down: [{time: 0.0, value: 1.0}],
                            hover: 1.0,
                        }
                    }
                }
            }
        }
    }

    pub TabCloseButton = <TabCloseButtonBase> {
        height: 10.0, width: 10.0,
        margin: { right: (THEME_SPACE_2), left: -1 },
        draw_button: {

            instance hover: float;
            instance active: float;

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                let mid = self.rect_size / 2.0;
                let size = (self.hover * 0.25 + 0.5) * 0.25 * length(self.rect_size);
                let min = mid - vec2(size);
                let max = mid + vec2(size);
                sdf.move_to(min.x, min.y);
                sdf.line_to(max.x, max.y);
                sdf.move_to(min.x, max.y);
                sdf.line_to(max.x, min.y);
                return sdf.stroke(mix(
                    #0,
                    #fe8610,
                    self.hover
                ), 1.0);
            }
        }

        animator: {
            hover = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.1}}
                    apply: {
                        draw_button: {hover: 0.0}
                    }
                }

                on = {
                    cursor: Hand,
                    from: {all: Snap}
                    apply: {
                        draw_button: {hover: 1.0}
                    }
                }
            }
        }
    }

    pub Tab = <TabBase> {
        width: Fit, height: Fill, //Fixed((THEME_TAB_HEIGHT)),

        align: {x: 0.0, y: 0.5}
        padding: <THEME_MSPACE_3> { }

        close_button: <TabCloseButton> {}
        draw_text: {
            text_style: <THEME_FONT_REGULAR> {}
            instance hover: 0.0
            instance active: 0.0
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        #x0, // THEME_COLOR_TEXT_INACTIVE,
                        #xf, // THEME_COLOR_TEXT_ACTIVE,
                        self.active
                    ),
                    #fe8610,
                    self.hover
                )
            }
        }

        draw_bg: {
            instance hover: float
            instance active: float

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                sdf.box(
                    -1.,
                    -1.,
                    self.rect_size.x + 2,
                    self.rect_size.y + 2,
                    1.
                );
                sdf.fill_keep(
                    mix(
                        (COLOR_DOCK_TAB),
                        (COLOR_ACTIVE_PRIMARY),
                        self.active
                    )
                );
                return sdf.result
            }
        }

        animator: {
            hover = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.2}}
                    apply: {
                        draw_bg: {hover: 0.0}
                        draw_text: {hover: 0.0}
                    }
                }

                on = {
                    cursor: Hand,
                    from: {all: Forward {duration: 0.1}}
                    apply: {
                        draw_bg: {hover: [{time: 0.0, value: 1.0}]}
                        draw_text: {hover: [{time: 0.0, value: 1.0}]}
                    }
                }
            }

            active = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.3}}
                    apply: {
                        close_button: {draw_button: {active: 0.0}}
                        draw_bg: {active: 0.0}
                        draw_text: {active: 0.0}
                    }
                }

                on = {
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

    pub TabBar = <TabBarBase> {
        CloseableTab = <Tab> {closeable:true}
        PermanentTab = <Tab> {closeable:false}

        draw_drag: {
            draw_depth: 10
            color: #x0
        }
        draw_fill: {
            color: (COLOR_TAB_BAR)
        }

        width: Fill, height: (THEME_TAB_HEIGHT)

        scroll_bars: <ScrollBarsTabs> {
            show_scroll_x: true
            show_scroll_y: false
            scroll_bar_x: {
                draw_bg: {size: 3.0}
                bar_size: 4
                use_vertical_finger_scroll: true
            }
        }
    }

    pub Dock = <DockBase> {
        flow: Down,

        round_corner: {
            border_radius: 20.
            fn pixel(self) -> vec4 {
                let pos = vec2(
                    mix(self.pos.x, 1.0 - self.pos.x, self.flip.x),
                    mix(self.pos.y, 1.0 - self.pos.y, self.flip.y)
                )

                let sdf = Sdf2d::viewport(pos * self.rect_size);
                sdf.rect(-10., -10., self.rect_size.x * 2.0, self.rect_size.y * 2.0);
                sdf.box(
                    0.25,
                    0.25,
                    self.rect_size.x * 2.0,
                    self.rect_size.y * 2.0,
                    4.0
                );

                sdf.subtract()
                sdf.fill(COLOR_SECONDARY)
                return sdf.result
            }
        }

        padding: {left: (THEME_DOCK_BORDER_SIZE), top: 0, right: (THEME_DOCK_BORDER_SIZE), bottom: (THEME_DOCK_BORDER_SIZE)}
        drag_target_preview: {
            draw_depth: 10.0
            color: (mix((COLOR_DRAG_TARGET), #FFFFFF00, pow(0.5, THEME_COLOR_CONTRAST)))
        }
        tab_bar: <TabBar> {}
        splitter: <Splitter> {}
    }
}

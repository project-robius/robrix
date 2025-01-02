use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    pub Splitter = <SplitterBase> {
        draw_splitter: {
            uniform border_radius: 1.0
            uniform splitter_pad: 1.0
            uniform splitter_grabber: 110.0

            instance pressed: 0.0
            instance hover: 0.0

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                sdf.clear(#ef);

                if self.is_vertical > 0.5 {
                    sdf.box(
                        self.splitter_pad,
                        self.rect_size.y * 0.5 - self.splitter_grabber * 0.5,
                        self.rect_size.x - 2.0 * self.splitter_pad,
                        self.splitter_grabber,
                        self.border_radius
                    );
                }
                else {
                    sdf.box(
                        self.rect_size.x * 0.5 - self.splitter_grabber * 0.5,
                        self.splitter_pad,
                        self.splitter_grabber,
                        self.rect_size.y - 2.0 * self.splitter_pad,
                        self.border_radius
                    );
                }
                return sdf.fill_keep(mix(
                    THEME_COLOR_D_HIDDEN,
                    mix(
                        THEME_COLOR_CTRL_SCROLLBAR_HOVER,
                        THEME_COLOR_CTRL_SCROLLBAR_HOVER * 1.2,
                        self.pressed
                    ),
                    self.hover
                ));
            }
        }
        split_bar_size: (THEME_SPLITTER_SIZE)
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
                        draw_splitter: {pressed: 0.0, hover: 0.0}
                    }
                }

                on = {
                    from: {
                        all: Forward {duration: 0.1}
                        state_down: Forward {duration: 0.01}
                    }
                    apply: {
                        draw_splitter: {
                            pressed: 0.0,
                            hover: [{time: 0.0, value: 1.0}],
                        }
                    }
                }

                pressed = {
                    from: { all: Forward { duration: 0.1 }}
                    apply: {
                        draw_splitter: {
                            pressed: [{time: 0.0, value: 1.0}],
                            hover: 1.0,
                        }
                    }
                }
            }
        }
    }

    pub TabCloseButton = <TabCloseButtonBase> {
            // TODO: NEEDS FOCUS STATE
        height: 10.0, width: 10.0,
        margin: { right: (THEME_SPACE_2), left: -3.5 },
        draw_button: {

            instance hover: float;
            instance selected: float;

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
                    #f,
                    #4,
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
        draw_name: {
            text_style: <THEME_FONT_REGULAR> {}
            instance hover: 0.0
            instance selected: 0.0
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        #x0, // THEME_COLOR_TEXT_INACTIVE,
                        #xf, // THEME_COLOR_TEXT_SELECTED,
                        self.selected
                    ),
                    THEME_COLOR_TEXT_HOVER,
                    self.hover
                )
            }
        }

        draw_bg: {
            instance hover: float
            instance selected: float

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                sdf.box(
                    -1.,
                    -1.,
                    self.rect_size.x + 2,
                    self.rect_size.y + 2,
                    1.
                )
                sdf.fill_keep(
                    mix(
                        (COLOR_SECONDARY) * 0.95,
                        (COLOR_SELECTED_PRIMARY),
                        self.selected
                    )
                )
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
                        draw_name: {hover: 0.0}
                    }
                }

                on = {
                    cursor: Hand,
                    from: {all: Forward {duration: 0.1}}
                    apply: {
                        draw_bg: {hover: [{time: 0.0, value: 1.0}]}
                        draw_name: {hover: [{time: 0.0, value: 1.0}]}
                    }
                }
            }

            selected = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.3}}
                    apply: {
                        close_button: {draw_button: {selected: 0.0}}
                        draw_bg: {selected: 0.0}
                        draw_name: {selected: 0.0}
                    }
                }

                on = {
                    from: {all: Snap}
                    apply: {
                        close_button: {draw_button: {selected: 1.0}}
                        draw_bg: {selected: 1.0}
                        draw_name: {selected: 1.0}
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
            color: (COLOR_SECONDARY)
        }

        width: Fill, height: (THEME_TAB_HEIGHT)

        scroll_bars: <ScrollBarsTabs> {
            show_scroll_x: true
            show_scroll_y: false
            scroll_bar_x: {
                draw_bar: {bar_width: 3.0}
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
                // sdf.fill(THEME_COLOR_BG_APP)
                sdf.fill(COLOR_SECONDARY)
                return sdf.result
            }
        }
        border_size: (THEME_DOCK_BORDER_SIZE)

        padding: {left: (THEME_DOCK_BORDER_SIZE), top: 0, right: (THEME_DOCK_BORDER_SIZE), bottom: (THEME_DOCK_BORDER_SIZE)}
        padding_fill: {color: (THEME_COLOR_BG_APP)} // TODO: unclear what this does
        drag_quad: {
            draw_depth: 10.0
            color: (mix((COLOR_SECONDARY), #FFFFFF00, pow(0.25, THEME_COLOR_CONTRAST)))
        }
        tab_bar: <TabBar> {}
        splitter: <Splitter> {}
    }
}

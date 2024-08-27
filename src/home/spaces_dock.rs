use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::adaptive_layout_view::AdaptiveLayoutView;

    ICON_HOME = dep("crate://self/resources/icons/home.svg")
    ICON_SETTINGS = dep("crate://self/resources/icons/settings.svg")

    SpacesDock = <AdaptiveLayoutView> {
        always_visible: true

        show_bg: true
        draw_bg: {
            color: (COLOR_SECONDARY)
        }

        composition: {
            desktop: {
                layout: {
                    flow: Down, spacing: 15
                    align: {x: 0.5}
                    padding: {top: 40., bottom: 20.}
                }
                walk: {
                    width: 68.
                    height: Fill
                }
            },
            mobile: {
                layout: {
                    flow: Right
                    align: {x: 0.5, y: 0.5}
                    padding: {top: 10, right: 100, bottom: 10, left: 100}
                }
                walk: {
                    width: Fill
                    height: Fit
                }
            }
            // @media (width <= 1250px) {
        }
        
        profile = <View> {
            width: Fit, height: Fit
            align: { x: 0.5, y: 0.5 }            

            text_view = <View> {
                width: 45., height: 45.,
                align: { x: 0.5, y: 0.5 }
                show_bg: true,

                draw_bg: {
                    instance background_color: #b8e5cc,
                    fn pixel(self) -> vec4 {
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        let c = self.rect_size * 0.5;
                        sdf.circle(c.x, c.x, c.x)
                        sdf.fill_keep(self.background_color);
                        return sdf.result
                    }
                }

                text = <Label> {
                    width: Fit, height: Fit,
                    padding: { top: 1.0 } // for better vertical alignment
                    draw_text: {
                        text_style: { font_size: 13. }
                        color: #f,
                    }
                    text: "U"
                }
            }
        }

        <AdaptiveLayoutView> {
            composition: {
                mobile: {
                    walk: {
                        width: 0, height: 0 // TODO, make this invisble instead by adding visibility to composition
                    }
                }
                desktop: {
                    walk: {
                        width: Fill, height: Fit
                    }
                }
            }
            <LineH> {
                margin: {left: 15, right: 15}
            }
        }

        // A mobile-only filler
        <AdaptiveLayoutView> {
            composition: {
                desktop: {
                    walk: {
                        height: 0, width: 0
                    }
                }
                mobile: {
                    walk: {
                        height: Fill, width: Fill // TODO, make this invisble instead by adding visibility to composition
                    }
                }
            }
        }
        
        home = <RoundedView> {
            width: Fit, height: Fit
            // FIXME: the extra padding on the right is becase the icon is not correctly centered
            // within its parent
            padding: {top: 8, left: 8, right: 12, bottom: 8}
            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY_DARKER)
                radius: 4
                border_color: (COLOR_SELECTED_PRIMARY)
                border_width: 1.5
            }
            align: {x: 0.5, y: 0.5}
            <Icon> {
                draw_icon: {
                    svg_file: (ICON_HOME),
                    fn get_color(self) -> vec4 {
                        return #1C274C;
                    }
                }
                icon_walk: {width: 25, height: Fit}
            }
        }

        filler_y = <View> {
            height: Fill,
            width: Fill,
        }

        settings = <View> {
            width: Fit, height: Fit
            // FIXME: the extra padding on the right is becase the icon is not correctly centered
            // within its parent
            padding: {top: 8, left: 8, right: 12, bottom: 8}
            align: {x: 0.5, y: 0.5}
            <Icon> {
                draw_icon: {
                    svg_file: (ICON_SETTINGS),
                    fn get_color(self) -> vec4 {
                        return #1C274C;
                    }
                }
                icon_walk: {width: 25, height: Fit}
            }
        }
    }
}

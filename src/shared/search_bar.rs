use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;

    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")

    SearchBar = <RoundedView> {
        width: Fill,
        height: Fit,

        show_bg: true,
        draw_bg: {
            color: #fff
        }

        padding: {top: 3, bottom: 3, left: 20, right: 20}
        spacing: 4,
        align: {x: 0.0, y: 0.5},

        draw_bg: {
            radius: 6.0,
            border_color: #D0D5DD,
            border_width: 1.0,
        }

        <Icon> {
            draw_icon: {
                svg_file: (ICON_SEARCH),
                fn get_color(self) -> vec4 {
                    return #666;
                }
            }
            icon_walk: {width: 14, height: Fit}
        }

        input = <TextInput> {
            width: Fill,
            height: Fit,

            empty_message: "Filter files"

            draw_text: {
                text_style: { font_size: 10 },
                fn get_color(self) -> vec4 {
                    return #555
                }
            }
    
            // TODO find a way to override colors
            draw_cursor: {
                instance focus: 0.0
                uniform border_radius: 0.5
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    sdf.box(
                        0.,
                        0.,
                        self.rect_size.x,
                        self.rect_size.y,
                        self.border_radius
                    )
                    sdf.fill(mix(#fff, #bbb, self.focus));
                    return sdf.result
                }
            }
    
            // TODO find a way to override colors
            draw_select: {
                instance hover: 0.0
                instance focus: 0.0
                uniform border_radius: 2.0
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    sdf.box(
                        0.,
                        0.,
                        self.rect_size.x,
                        self.rect_size.y,
                        self.border_radius
                    )
                    sdf.fill(mix(#eee, #ddd, self.focus)); // Pad color
                    return sdf.result
                }
            }
    
            draw_bg: {
                color: #fff
                instance radius: 2.0
                instance border_width: 0.0
                instance border_color: #3
                instance inset: vec4(0.0, 0.0, 0.0, 0.0)
    
                fn get_color(self) -> vec4 {
                    return self.color
                }
    
                fn get_border_color(self) -> vec4 {
                    return self.border_color
                }
    
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                    sdf.box(
                        self.inset.x + self.border_width,
                        self.inset.y + self.border_width,
                        self.rect_size.x - (self.inset.x + self.inset.z + self.border_width * 2.0),
                        self.rect_size.y - (self.inset.y + self.inset.w + self.border_width * 2.0),
                        max(1.0, self.radius)
                    )
                    sdf.fill_keep(self.get_color())
                    if self.border_width > 0.0 {
                        sdf.stroke(self.get_border_color(), self.border_width)
                    }
                    return sdf.result;
                }
            }
        }
    }
}

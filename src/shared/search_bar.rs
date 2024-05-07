use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;

    SearchBar = <View> {
        width: Fill, height: Fit
        show_bg: true
        draw_bg: {
            color: #EEEEEE,
        }

        input = <TextInput> {
            width: Fill, height: Fit, margin: {left: 5.0, right: 5.0, top: 5.0, bottom: 15.0}
            clip_x: true,
            clip_y: true,
            align: {y: 0.5}
            empty_message: "Search..."
            draw_bg: {
                color: #F9F9F9
            }
            draw_text: {
                color: (MESSAGE_TEXT_COLOR),
                text_style: <MESSAGE_TEXT_STYLE>{},

                fn get_color(self) -> vec4 {
                    return mix(
                        mix(
                            mix(
                                #xFFFFFF55,
                                #xFFFFFF88,
                                self.hover
                            ),
                            self.color,
                            self.focus
                        ),
                        #BBBBBB,
                        self.is_empty
                    )
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
                    sdf.fill(mix(#0f0, #0b0, self.focus));
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
                    sdf.fill(mix(#0e0, #0d0, self.focus)); // Pad color
                    return sdf.result
                }
            }
        }
    }
}

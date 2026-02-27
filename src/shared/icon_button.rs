use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.COLOR_BRAND = #x5
    mod.widgets.COLOR_BRAND_HOVER = #x3
    mod.widgets.COLOR_META_TEXT = #xaaa

    mod.widgets.IconButton = Button {
        padding: 9.0
        draw_text +: {
            text_style: theme.font_regular {
                font_size: 11.0
            }
            get_color: fn() -> vec4 {
                return mix(
                    mix(
                        (mod.widgets.COLOR_META_TEXT),
                        (mod.widgets.COLOR_BRAND),
                        self.hover
                    ),
                    (mod.widgets.COLOR_BRAND_HOVER),
                    self.down
                )
            }
        }
        draw_icon +: {
            get_color: fn() -> vec4 {
                return mix(
                    mix(
                        (mod.widgets.COLOR_META),
                        (mod.widgets.COLOR_BRAND),
                        self.hover
                    ),
                    (mod.widgets.COLOR_BRAND_HOVER),
                    self.down
                )
            }
        }
        icon_walk: Walk{width: 7.5, height: Fit, margin: Inset{left: 5.0}}
        draw_bg +: {
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size);
                return sdf.result
            }
        }
        text: ""
    }


    // Customized button widget, based on the RoundedView shaders with some modifications
    // which is a better fit with our application UI design
    mod.widgets.RobrixIconButton = Button {
        width: Fit,
        height: Fit,
        spacing: 10,
        padding: 10,
        align: Align{x: 0, y: 0.5}

        draw_bg +: {
            hover: instance(0.0)
            color: uniform((mod.widgets.COLOR_PRIMARY))
            // We set a mid-gray hover color, which gets mixed with the bg color itself
            // in order to create a "lightening" effect upon hover.
            color_hover: uniform(#A)
            border_size: uniform(0.0)
            border_color: uniform(#D0D5DD)
            border_radius: uniform(4.0)

            get_color: fn() -> vec4 {
                return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
            }

            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(
                    self.border_size,
                    self.border_size,
                    self.rect_size.x - (self.border_size * 2.0),
                    self.rect_size.y - (self.border_size * 2.0),
                    max(1.0, self.border_radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_size > 0.0 {
                    sdf.stroke(self.border_color, self.border_size)
                }
                return sdf.result;
            }
        }

        draw_icon +: {
            hover: instance(0.0)
            color: #000
            color_hover: uniform(#000)
            get_color: fn() -> vec4 {
                return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
            }
        }
        icon_walk: Walk{width: 16, height: 16}

        draw_text +: {
            hover: instance(0.0)
            color: #000
            color_hover: uniform(#000)
            get_color: fn() -> vec4 {
                return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
            }
            text_style: mod.widgets.REGULAR_TEXT {font_size: 10},
        }
        text: ""
    }
}

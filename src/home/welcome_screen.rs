use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;

    WELCOME_TEXT_COLOR: #x4

    WelcomeScreen = <View> {
        width: Fill, height: Fill
        align: {x: 0.0, y: 0.5}
        welcome_message = <RoundedView> {
            padding: 40.
            width: Fill, height: Fit
            flow: Down, spacing: 20

            title = <Label> {
                text: "Welcome to Robrix!",
                draw_text: {
                    color: (WELCOME_TEXT_COLOR),
                    text_style: <THEME_FONT_BOLD> {
                        font_size: 22.0
                    }
                }
            }

            subtitle = <Label> {
                text: "Our Matrix client is under heavy development.\nFor now, you can access the rooms you've joined in other clients.\nBut don't worry, we're working on expanding its features.",
                draw_text: {
                    color: (WELCOME_TEXT_COLOR),
                    text_style: {
                        font_size: 14.
                    }
                }
            }

            // Using the HTML widget to taking advantage of embedding a link within text with proper vertical alignment
            <Html> {
                padding: {top: 12, left: 0.}
                font_size: 14.
                draw_normal: {
                    color: (WELCOME_TEXT_COLOR)
                }
                a = {
                    padding: {left: 8., right: 8., top: 4., bottom: 5.},
                    draw_text: {
                        text_style: <THEME_FONT_BOLD> {top_drop: 1.2, font_size: 11. },
                        color: #f,
                        color_pressed: #f00,
                        color_hover: #0f0,
                    }
                    draw_bg: {
                        instance border_width: 0.0
                        instance border_color: #0000
                        instance inset: vec4(0.0, 0.0, 0.0, 0.0)
                        instance radius: 2.
                        instance color: #x0
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
                body:"
                Look out for the latest announcements in our Matrix channel: <a href=\"https://matrix.to/#/#robius-robrix:matrix.org\">[m] Robrix</a><br/>
                "
            }
        }
    }
}

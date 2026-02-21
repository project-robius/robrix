use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.WELCOME_TEXT_COLOR = #x4

    mod.widgets.WelcomeScreen = ScrollYView {
        width: Fill, height: Fill
        align: Align{x: 0.0, y: 0.5}

        show_bg: true,
        draw_bg +: {
            color: (COLOR_PRIMARY),
        }

        welcome_message := RoundedView {
            padding: 40.
            width: Fill, height: Fit
            flow: Down, spacing: 20

            title := Label {
                text: "Welcome to Robrix!",
                draw_text +: {
                    color: (mod.widgets.WELCOME_TEXT_COLOR),
                    text_style: theme.font_bold {
                        font_size: 22.0
                    }
                }
            }

            // Using the HTML widget to taking advantage of embedding a link within text with proper vertical alignment
            MessageHtml {
                padding: Inset{top: 12, left: 0.}
                font_size: 14.
                font_color: (mod.widgets.WELCOME_TEXT_COLOR)
                text_style_normal: theme.font_regular { font_size: 14.0 }
                a: {
                    padding: Inset{left: 8., right: 8., top: 4., bottom: 5.},
                    // draw_text +: {
                    //     text_style: theme.font_bold {top_drop: 1.2, font_size: 11. },
                    //     color: #f,
                    //     color_pressed: #f00,
                    //     color_hover: #0f0,
                    // }
                }
                body:"<p>Our Matrix client is under heavy development. Currently, you can access the rooms and spaces that you've joined in other clients.</p>
                <p><br></p>
                <p>But don't worry, we're constantly expanding the featureset of Robrix!</p>
                <p><br></p>
                <p>Look for the latest announcements in our Matrix channel:</p>
                <p><b>#robrix:matrix.org</b></p>
                "
            }
        }
    }
}

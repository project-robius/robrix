use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::html_or_plaintext::*;

    WELCOME_TEXT_COLOR: #x4

    pub WelcomeScreen = <ScrollYView> {
        width: Fill, height: Fill
        align: {x: 0.0, y: 0.5}

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY),
        }

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

            // Using the HTML widget to taking advantage of embedding a link within text with proper vertical alignment
            <MessageHtml> {
                padding: {top: 12, left: 0.}
                font_size: 14.
                font_color: (WELCOME_TEXT_COLOR)
                draw_normal: {
                    color: (WELCOME_TEXT_COLOR)
                }
                a = {
                    padding: {left: 8., right: 8., top: 4., bottom: 5.},
                    // draw_text: {
                    //     text_style: <THEME_FONT_BOLD> {top_drop: 1.2, font_size: 11. },
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

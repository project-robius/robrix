use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::html_or_plaintext::*;

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
                text: "
                    Our Matrix client is under heavy development.\nFor now, you can access the rooms you've joined in other clients.\nBut don't worry, we're working on expanding its features.\n
                    Tip: to load older messages, click on a selected room (repeatedly).
                    ",
                draw_text: {
                    color: (WELCOME_TEXT_COLOR),
                    text_style: {
                        font_size: 14.
                    }
                }
            }

            // Using the HTML widget to taking advantage of embedding a link within text with proper vertical alignment
            <RobrixHtml> {
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
                body:"
                Look out for the latest announcements in our Matrix channel: <a href=\"https://matrix.to/#/#robius-robrix:matrix.org\">[m] Robrix</a><br/>
                "
            }
        }
    }
}

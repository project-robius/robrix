use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;

    VERIFICATION_YES = dep("crate://self/resources/icons/verification_yes.svg")
    VERIFICATION_NO = dep("crate://self/resources/icons/verification_no.svg")
    VERIFICATION_UNK = dep("crate://self/resources/icons/verification_unk.svg")

    VerificationIcon = <Icon> {
        icon_walk: { width: 23 }
    }
    IconYes = <View> {
        visible: false
        width: 31, height: 31
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_YES),
                fn get_color(self) -> vec4 {
                    return #x00BF00;
                }
            }
        }
    }
    IconNo = <View> {
        visible: false
        width: 31, height: 31
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_NO),
                fn get_color(self) -> vec4 {
                    return #xBF0000;
                }
            }
        }
    }
    IconUnk = <View> {
        visible: false
        width: 31, height: 31
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_UNK),
                fn get_color(self) -> vec4 {
                    return #x333333;
                }
            }
        }
    }

    VerificationNoticeDesktop = <TooltipBase> {
        width: Fill, height: Fill,
        flow: Overlay

        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(0., 0., 0., 0.0)
            }
        }

        content: <View> {
            flow: Overlay

            //The 'Fill' allows it shows anywhere we want over the app screen,
            //our goal is to set the global relative position to make it an illusion of following the cursor.
            width: Fill, height: Fill

            <RoundedView> {
                width: Fit, height: Fit,
                padding: 7,

                draw_bg: {
                    color: (COLOR_TOOLTIP_BG),
                    border_width: 1.0,
                    border_color: #000000,
                    radius: 2.5
                }

                tooltip_label = <Label> {
                    width: 230
                    draw_text: {
                        text_style: <THEME_FONT_REGULAR>{font_size: SMALL_STATE_FONT_SIZE},
                        text_wrap: Word,
                        color: #000
                    }
                }
            }
        }
    }
    VerificationNoticeMobile = <TooltipBase> {
        width: Fill, height: Fill,
        flow: Overlay

        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(0., 0., 0., 0.0)
            }
        }

        content: <View> {
            flow: Overlay

            //Same as `VerificationNoticeDesktop`
            width: Fill, height: Fill

            // Via setting suitable align & padding,
            // we can simulate a relative position to make the tootip follow widget `Profile (U)`,
            // this is not a perfect solution.
            // TODO: Find a way to follow widget `Profile (U)` more precisely.
            align: { x: 0.43, y: 1. }
            padding: { left: 30., bottom: 31. }

            <RoundedView> {
                width: Fit, height: Fit,
                padding: 7,

                draw_bg: {
                    color: (COLOR_TOOLTIP_BG),
                    border_width: 1.0,
                    border_color: #000000,
                    radius: 2.5
                }

                tooltip_label = <Label> {
                    width: 230
                    draw_text: {
                        text_style: <THEME_FONT_REGULAR>{font_size: SMALL_STATE_FONT_SIZE},
                        text_wrap: Word,
                        color: #000
                    }
                }
            }
        }
    }
}

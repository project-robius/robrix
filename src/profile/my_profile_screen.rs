use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::helpers::FillerX;
    import crate::shared::helpers::Divider;
    import crate::shared::search_bar::SearchBar;
    import crate::shared::styles::*;

    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")
    IMG_QR = dep("crate://self/resources/img/qr_icon.png")

    ActionIcon = <Label> {
        width: Fit, height: Fit
        text: ">"
        draw_text: {
            color: #b4,
            text_style: <REGULAR_TEXT>{font_size: 16},
        }
    }

    OptionsItem = <View> {
        width: Fill, height: Fit
        padding: {left: 10., top: 10., right: 10. bottom: 2.}, spacing: 8., flow: Down
        show_bg: true
        draw_bg: {
            color: #fff
        }

        content = <View> {
            width: Fill, height: 36.
            padding: 0, align: {x: 0.0, y: 0.5}, spacing: 10., flow: Right

            label = <Label> {
                width: Fit, height: Fit
                draw_text: {
                    color: #000,
                    text_style: <REGULAR_TEXT>{},
                },
            }

            <FillerX> {}

            item_data = <View> {
                width: 0., height: 0.
            }

            action_icon = <ActionIcon> {}
        }

        divider = <Divider> {}
    }

    Options = <View> {
        width: Fill, height: Fit
        padding: 0, spacing: 0., flow: Down
    }

    MyProfileScreen = <View> {
        width: Fill, height: Fill
        flow: Down, spacing: 10.
        show_bg: true,
        draw_bg: {
            color: #eee
        }

        <Options> {
            <OptionsItem> {
                content = {
                    width: Fill, height: Fit
                    label = {
                        text: "Profile Photo"
                    }
                    item_data = <Image> {
                        source: (IMG_DEFAULT_AVATAR),
                        width: 60., height: 60.
                    }
                }
            }

            <OptionsItem> {
                content = {
                    label = {
                        text: "Name"
                    }
                    item_data = <Label> {
                        width: Fit, height: Fit
                        draw_text:{
                            color: #6
                            text_style: <REGULAR_TEXT>{},
                        }
                        text: "facu"
                    }
                }
            }

            <OptionsItem> {
                content = {
                    label = {
                        text: "Tickle"
                    }
                }
            }

            <OptionsItem> {
                content = {
                    label = {
                        text: "WeChat ID"
                    }
                    item_data = <Label> {
                        width: Fit, height: Fit
                        draw_text: {
                            color: #6
                            text_style: <REGULAR_TEXT>{},
                        }
                        text:"wxid_123n43kjl123hjg"
                    }
                }
            }

            <OptionsItem> {
                content = {
                    label = {
                        text: "My QR Code"
                    }
                    item_data = <Image> {
                        source: (IMG_QR),
                        width: 20., height: 20.
                    }
                }
            }

            <OptionsItem> {
                content = {
                    label = {
                        text: "More Info"
                    }
                }
                divider = <View> {}
            }
        }

        <Options> {
            <OptionsItem> {
                content = {
                    label = {
                        text: "Ringtone for Incoming Calls"
                    }
                }
                divider = <View> {}
            }
        }

        <Options> {
            <OptionsItem> {
                content = {
                    label = {
                        text: "WeBeans"
                    }
                }
                divider = <View> {}
            }
        }
    }
}

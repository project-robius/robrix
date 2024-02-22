use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    // import crate::shared::helpers::FillerX;
    // import crate::shared::helpers::Divider;
    import crate::shared::search_bar::SearchBar;
    import crate::shared::styles::*;

    OptionsItem = <View> {
        width: Fill, height: Fit
        padding: <MSPACE_H_2> {}
        flow: Down,

        show_bg: true,
        draw_bg: { color: (COLOR_D_0) }

        content = <View> {
            flow: Right,
            width: Fill, height: Fit,
            align: {x: 0.0, y: 0.5}
            padding: <MSPACE_2> {}, 

            label = <H4> {}
            <Filler> {}
            item_data = <View> { }
            action_icon = <ActionIcon> {}
        }

        divider = <DividerH> {}
    }

    Options = <View> {
        width: Fill, height: Fit
        flow: Down
        padding: <MSPACE_0> {},
        spacing: (SPACE_0)
        show_bg: true,
        draw_bg: { color: (COLOR_D_0) }
    }

    MyProfileScreen = <View> {
        width: Fill, height: Fill
        flow: Down,
        spacing: (SPACE_2)

        show_bg: true,
        draw_bg: { color: (COLOR_D_1) }

        <Options> {
            width: Fill,
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
                    item_data = <P> {
                        margin: <MSPACE_H_1> {}
                        width: Fit, height: Fit
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
                    item_data = <P> {
                        margin: <MSPACE_H_1> {}
                        width: Fit, height: Fit
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
                        margin: <MSPACE_H_2> {}
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
                divider = <DividerH> {}
            }

            <OptionsItem> {
                content = {
                    width: Fill,
                    label = {
                        text: "Ringtone for Incoming Calls"
                    }
                    item_data = <View> { width: 0.0 }
                }
                divider = <DividerH> {}
            }
        }

        <Options> {
            <OptionsItem> {
                width: Fill,
                content = {
                    label = {
                        text: "WeBeans"
                    }
                }
            }
        }
    }
}

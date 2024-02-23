use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    // import crate::shared::helpers::FillerX;
    // import crate::shared::helpers::Divider;
    // import crate::shared::search_bar::SearchBar;
    import crate::shared::styles::*;

    MyProfileScreen = <View> {
        width: Fill, height: Fill
        padding: <MSPACE_2> {}, margin: <MSPACE_0> {}, 
        flow: Down,
        spacing: (SPACE_2)

        show_bg: true,
        draw_bg: { color: (COLOR_D_1) }

        <Options> {
            width: Fill,
            <OptionsItem> {
                content = {
                    label = {
                        text: "Profile Photo"
                    }
                    item_data = <Image> {
                        source: (IMG_DEFAULT_AVATAR),
                        width: 60., height: 60.
                    }
                }
            }

            <DividerH> {}

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

            <DividerH> {}

            <OptionsItem> {
                content = {
                    label = {
                        text: "Tickle"
                    }
                }
            }

            <DividerH> {}

            <OptionsItem> {
                content = {
                    label = {
                        text: "WeChat ID"
                    }
                    item_data = <P> {
                        margin: <MSPACE_H_1> {}
                        text:"wxid_123n43kjl123hjg"
                    }
                }
            }

            <DividerH> {}

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

            <DividerH> {}

            <OptionsItem> {
                content = {
                    label = {
                        text: "More Info"
                    }
                }
            }

            <DividerH> {}

            <OptionsItem> {
                content = {
                    label = {
                        text: "Ringtone for Incoming Calls"
                    }
                    item_data = <View> { }
                }
            }

            <DividerH> {}

            <Options> {
                <OptionsItem> {
                    content = {
                        label = {
                            text: "WeBeans"
                        }
                    }
                }
            }
        }

    }
}

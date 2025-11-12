use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    // import crate::shared::search_bar::SearchBar;
    // import crate::shared::helpers::*;
    import crate::shared::styles::*;


    OptionsItem = <View> {
        width: Fill, height: Fit,
        flow: Down
        padding: <MSPACE_1> {}
        spacing: (SPACE_2),

        content = <View> {
            width: Fill, height: Fit,
            flow: Right
            align: {x: 0.0, y: 0.0},
            margin: <MSPACE_1> {}, padding: <MSPACE_0> {},
            spacing: (SPACE_2),

            icon = <Image> {
                width: 24., height: 24.,
                margin: <MSPACE_2> {}
            }

            labels = <View> {
                width: Fill, height: Fit,
                flow: Down,
                padding: <MSPACE_0> {},
                spacing: (SPACE_0),

                main = <H4> {
                    width: Fill, height: Fit,
                    margin: { bottom: (SPACE_1) }
                }
                secondary = <P> { }
            }

            action_icon = <ActionIcon> {
                margin: {right: (SPACE_1)}
            }
        }

        show_bg: true
        draw_bg: { color: (COLOR_U) }
        divider = <DividerH> {}
    }

    AddContactScreen = <View> {
        width: Fill, height: Fill,
        flow: Down,
        spacing: (SPACE_2) 

        show_bg: true
        draw_bg: { color: (COLOR_D_1) }

        <View> {
            flow: Down,
            width: Fill, height: Fit,
            align: {x: 0.0, y: 0.5},
            margin: <MSPACE_0> {}, padding: <MSPACE_2> {},
            spacing: (SPACE_2),

            <SearchBar> { input = { empty_message: "Account/Mobile" } }
            <View> {
                flow: Right,
                width: Fill, height: Fit
                padding: <MSPACE_0> {}, margin: <MSPACE_2> {},
                spacing: (SPACE_2),
                <H4> { text: "My WeChat ID: wxid_123n43kjl123hjg" }
                <Image> {
                    width: 20., height: 20.
                    source: (IMG_QR),
                }
            }
        }

        <Options> {
            <OptionsItem> {
                show_bg: true,
                draw_bg: { color: (COLOR_D_0) },
                content = {
                    icon = {
                        source: (IMG_INVITE_FRIENDS)
                    }

                    labels = {
                        main = { text: "Invite Friends" }
                        secondary = { text: "Invite friends to chat using the app!" }
                    }
                }
                divider = <DividerH> {}
            }

            <OptionsItem> {
                show_bg: true,
                draw_bg: { color: (COLOR_D_0) },
                content = {
                    icon = {
                        source: (IMG_FRIEND_RADAR)
                    }

                    labels = {
                        main = { text: "Friend Radar" }
                        secondary = { text: "Quickly add friends nearly" }
                    }
                }
                divider = <DividerH> {}
            }

            <OptionsItem> {
                show_bg: true,
                draw_bg: { color: (COLOR_D_0) },
                content = {
                    icon = {
                        source: (IMG_GROUP_CHATS)
                    }

                    labels = {
                        main = { text: "Join Private Group" }
                        secondary = { text: "Join a group with friends nearby" }
                    }
                }
                divider = <DividerH> {}
            }

            <OptionsItem> {
                show_bg: true,
                draw_bg: { color: (COLOR_D_0) },
                content = {
                    icon = {
                        source: (IMG_SCAN_QR)
                    }

                    labels = {
                        main = { text: "Scan QR Code" }
                        secondary = { text: "Scan a friend's QR code" }
                    }
                }
                divider = <DividerH> {}
            }

            <OptionsItem> {
                show_bg: true,
                draw_bg: { color: (COLOR_D_0) },
                content = {
                    icon = {
                        source: (IMG_MOBILE_CONTACTS)
                    }

                    labels = {
                        main = { text: "Mobile Contacts" }
                        secondary = { text: "Add from your mobile address book" }
                    }
                }
                divider = <DividerH> {}
            }

            <OptionsItem> {
                show_bg: true,
                draw_bg: { color: (COLOR_D_0) },
                content = {
                    icon = {
                        source: (IMG_OFFICIAL_ACCOUNTS)
                    }

                    labels = {
                        main = { text: "Official Accounts" }
                        secondary = { text: "Get more services and information" }
                    }
                }
                divider = <DividerH> {}
            }

            <OptionsItem> {
                show_bg: true,
                draw_bg: { color: (COLOR_D_0) },
                content = {
                    icon = {
                        source: (IMG_WECOM_CONTACTS)
                    }

                    labels = {
                        main = { text: "WeCom Contacts" }
                        secondary = { text: "Find WeCom user by phone number" }
                    }
                }

                divider = <DividerH> {}
            }
        }
    }
}

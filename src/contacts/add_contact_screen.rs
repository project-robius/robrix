use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::search_bar::SearchBar;
    import crate::shared::helpers::*;
    import crate::shared::styles::*;

    IMG_QR = dep("crate://self/resources/img/qr_green.png")
    IMG_INVITE_FRIENDS = dep("crate://self/resources/img/invite_friends.png")
    IMG_FRIEND_RADAR = dep("crate://self/resources/img/friend_radar.png")
    IMG_SCAN_QR = dep("crate://self/resources/img/scan_qr.png")
    IMG_GROUP_CHATS = dep("crate://self/resources/img/group_chats.png")
    IMG_MOBILE_CONTACTS = dep("crate://self/resources/img/mobile_contacts.png")
    IMG_OFFICIAL_ACCOUNTS = dep("crate://self/resources/img/official_accounts.png")
    IMG_WECOM_CONTACTS = dep("crate://self/resources/img/wecom_contacts.png")

    ActionIcon = <Label> {
        width: Fit,
        height: Fit,
        text: ">"
        draw_text: {
            color: #b4
            text_style: <REGULAR_TEXT>{font_size: 16},
        }
    }

    OptionsItem = <View> {
        width: Fill,
        height: Fit,
        padding: {left: 10., top: 10., right: 0, bottom: 2.}, spacing: 8., flow: Down

        content = <View> {
            width: Fill,
            height: Fit,
            margin: {left: 5., top: 6., bottom: 6., right: 0}
            padding: 0, align: {x: 0.0, y: 0.0}, spacing: 10., flow: Right

            icon = <Image> {
                width: 24.,
                height: 24.,
                margin: {right: 10.}
            }

            labels = <View> {
                width: Fill,
                height: Fit,
                padding: 0, spacing: 10., flow: Down

                main = <Label> {
                    width: Fill
                    height: Fit,
                    draw_text: {
                        color: #000,
                        text_style: <REGULAR_TEXT>{},
                    },
                }

                secondary = <Label> {
                    width: Fill,
                    height: Fit,
                    draw_text: {
                        color: #9c9c9c,
                        text_style: <REGULAR_TEXT>{font_size: 10.},
                    },
                }
            }

            action_icon = <ActionIcon> {
                margin: {right: 20.}
            }
        }

        show_bg: true
        draw_bg: {
            color: #fff
        }
        divider = <Divider> {
            margin: {left: 42.0}
        }
    }

    Options = <View> {
        width: Fill,
        height: Fit,
        padding: 0, spacing: 0., flow: Down
    }

    AddContactScreen = <View> {
        width: Fill,
        height: Fill,
        flow: Down, spacing: 10.

        show_bg: true
        draw_bg: {
            color: #ddd
        }

        <SearchBar> {
            input = {
                empty_message: "Account/Mobile"
            }
        }

        <View> {
            width: Fill,
            height: Fit,
            margin: {bottom: 20.}
            align: {x: 0.5, y: 0.5}, spacing: 10.

            <Label> {
                draw_text: {
                    color: #000,
                    text_style: <REGULAR_TEXT>{font_size: 11.},
                }
                text: "My WeChat ID: wxid_123n43kjl123hjg"
            }

            <Image> {
                source: (IMG_QR),
                width: 20.,
                height: 20.
            }
        }

        <Options> {
            <OptionsItem> {
                content = {
                    icon = {
                        source: (IMG_INVITE_FRIENDS)
                    }

                    labels = {
                        main = { text: "Invite Friends" }
                        secondary = { text: "Invite friends to chat using the app!" }
                    }
                }
            }

            <OptionsItem> {
                content = {
                    icon = {
                        source: (IMG_FRIEND_RADAR)
                    }

                    labels = {
                        main = { text: "Friend Radar" }
                        secondary = { text: "Quickly add friends nearly" }
                    }
                }
            }

            <OptionsItem> {
                content = {
                    icon = {
                        source: (IMG_GROUP_CHATS)
                    }

                    labels = {
                        main = { text: "Join Private Group" }
                        secondary = { text: "Join a group with friends nearby" }
                    }
                }
            }

            <OptionsItem> {
                content = {
                    icon = {
                        source: (IMG_SCAN_QR)
                    }

                    labels = {
                        main = { text: "Scan QR Code" }
                        secondary = { text: "Scan a friend's QR code" }
                    }
                }
            }

            <OptionsItem> {
                content = {
                    icon = {
                        source: (IMG_MOBILE_CONTACTS)
                    }

                    labels = {
                        main = { text: "Mobile Contacts" }
                        secondary = { text: "Add from your mobile address book" }
                    }
                }
            }

            <OptionsItem> {
                content = {
                    icon = {
                        source: (IMG_OFFICIAL_ACCOUNTS)
                    }

                    labels = {
                        main = { text: "Official Accounts" }
                        secondary = { text: "Get more services and information" }
                    }
                }
            }

            <OptionsItem> {
                content = {
                    icon = {
                        source: (IMG_WECOM_CONTACTS)
                    }

                    labels = {
                        main = { text: "WeCom Contacts" }
                        secondary = { text: "Find WeCom user by phone number" }
                    }
                }

                divider = <View> {}
            }
        }
    }
}

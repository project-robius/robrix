use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_draw::shader::std::*;
    import makepad_widgets::theme_desktop_dark::*;

    ICON_ARR_R = dep("crate://self/resources/icons/tri_r.svg")
    ICON_CHAT = dep("crate://self/resources/icons/chat.svg")
    ICON_CONTACTS = dep("crate://self/resources/icons/contacts.svg")
    ICON_CREATE = dep("crate://self/resources/icons/plus.svg")
    ICON_EMOJI = dep("crate://self/resources/icons/emoji.svg")
    ICON_HOME = dep("crate://self/resources/icons/home.svg")
    ICON_ME = dep("crate://self/resources/icons/user.svg")
    ICON_SCAN = dep("crate://self/resources/icons/scan.svg")
    ICON_SEARCH = dep("crate://self/resources/icons/search.svg")
    ICON_SETTINGS = dep("crate://self/resources/icons/cog.svg")
    ICON_USERS = dep("crate://self/resources/icons/users.svg")
    
    IMG_BANNER = dep("crate://self/resources/img/hero.jpg")
    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")
    IMG_FAVORITES = dep("crate://self/resources/img/favorites.png")
    IMG_FILE_TRANSFER_AVATAR = dep("crate://self/resources/img/file_transfer_avatar.png")
    IMG_FRIEND_RADAR = dep("crate://self/resources/img/friend_radar.png")
    IMG_GROUP_CHATS = dep("crate://self/resources/img/group_chats.png")
    IMG_INVITE_FRIENDS = dep("crate://self/resources/img/invite_friends.png")
    IMG_LOADING = dep("crate://self/resources/img/loading.png")
    IMG_MINI_PROGRAMS = dep("crate://self/resources/img/mini_programs.png")
    IMG_MOBILE_CONTACTS = dep("crate://self/resources/img/mobile_contacts.png")
    IMG_MOMENTS = dep("crate://self/resources/img/moments.png")
    IMG_MY_POSTS = dep("crate://self/resources/img/my-posts.png")
    IMG_NEW_FRIENDS = dep("crate://self/resources/img/new_friends.png")
    IMG_OFFICIAL_ACCOUNTS = dep("crate://self/resources/img/official_accounts.png")
    IMG_PEOPLE_NEARBY = dep("crate://self/resources/img/people_nearby.png")
    IMG_POST1 = dep("crate://self/resources/img/post1.jpg")
    IMG_POST2 = dep("crate://self/resources/img/post2.jpg")
    IMG_QR = dep("crate://self/resources/img/qr_icon.png")
    IMG_SCAN = dep("crate://self/resources/img/scan.png")
    IMG_SCAN_QR = dep("crate://self/resources/img/scan_qr.png")
    IMG_SEARCH = dep("crate://self/resources/img/search.png")
    IMG_SETTINGS = dep("crate://self/resources/img/settings.png")
    IMG_SHAKE = dep("crate://self/resources/img/shake.png")
    IMG_STICKER_GALLERY = dep("crate://self/resources/img/sticker-gallery.png")
    IMG_TAGS = dep("crate://self/resources/img/tags.png")
    IMG_TESTUSER = dep("crate://self/resources/img/profile_1.jpg")
    IMG_WECHAT_AVATAR = dep("crate://self/resources/img/wechat_avatar.png")
    IMG_WECOM_CONTACTS = dep("crate://self/resources/img/wecom_contacts.png")
    
    OS_SPACE_TOP = 25.
    OS_SPACE_BOTTOM = 50.
    
    SPACE_FACTOR = 10.0 // Increase for a less dense layout
    SPACE_0 = 0.0
    SPACE_1 = (0.5 * (SPACE_FACTOR))
    SPACE_2 = (1 * (SPACE_FACTOR))
    SPACE_3 = (2 * (SPACE_FACTOR))

    MSPACE_0 = {top: (SPACE_0), right: (SPACE_0), bottom: (SPACE_0), left: (SPACE_0)}
    MSPACE_1 = {top: (SPACE_1), right: (SPACE_1), bottom: (SPACE_1), left: (SPACE_1)}
    MSPACE_H_1 = {top: (SPACE_0), right: (SPACE_1), bottom: (SPACE_0), left: (SPACE_1)}
    MSPACE_V_1 = {top: (SPACE_1), right: (SPACE_0), bottom: (SPACE_1), left: (SPACE_0)}
    MSPACE_2 = {top: (SPACE_2), right: (SPACE_2), bottom: (SPACE_2), left: (SPACE_2)}
    MSPACE_H_2 = {top: (SPACE_0), right: (SPACE_2), bottom: (SPACE_0), left: (SPACE_2)}
    MSPACE_V_2 = {top: (SPACE_2), right: (SPACE_0), bottom: (SPACE_2), left: (SPACE_0)}

    COLOR_BG = #xF0F0F0

    COLOR_U = #xFFFFFFFF
    COLOR_U_0 = #xFFFFFF00
    COLOR_U_2 = #xFFFFFF33

    COLOR_D = #x000000FF
    COLOR_D_0 = #x00000000
    COLOR_D_1 = #x00000011
    COLOR_D_2 = #x00000028
    COLOR_D_3 = #x00000033
    COLOR_D_4 = #x00000044
    COLOR_D_5 = #x00000066
    COLOR_D_6 = #x00000088
    COLOR_D_7 = #x000000AA

    COLOR_ACCENT = #08F
    COLOR_SELECT = (COLOR_D_3)
    COLOR_HL = (COLOR_D_7)
    COLOR_TEXT = (COLOR_D_6)
    COLOR_META = (COLOR_D_4)

    FONT_SIZE_BASE = 8.5
    FONT_SIZE_CONTRAST = 2.5 // Greater values = greater font-size steps between font-formats (i.e. from H3 to H2)

    FONT_SIZE_1 = (FONT_SIZE_BASE + 16 * FONT_SIZE_CONTRAST)
    FONT_SIZE_2 = (FONT_SIZE_BASE + 8 * FONT_SIZE_CONTRAST)
    FONT_SIZE_3 = (FONT_SIZE_BASE + 4 * FONT_SIZE_CONTRAST)
    FONT_SIZE_4 = (FONT_SIZE_BASE + 2 * FONT_SIZE_CONTRAST)
    FONT_SIZE_P = (FONT_SIZE_BASE + 1 * FONT_SIZE_CONTRAST)
    FONT_SIZE_META = (FONT_SIZE_BASE + 0.5 * FONT_SIZE_CONTRAST)

    FONT_REGULAR = { font: { path: dep("crate://self/resources/fonts/Inter-Regular.ttf") } }
    FONT_ITALIC = { font: { path: dep("crate://self/resources/fonts/Inter-Italic.ttf") } }
    FONT_BOLD = { font: { path: dep("crate://self/resources/fonts/Inter-Bold.ttf") } }
    FONT_BOLD_ITALIC = { font: { path: dep("crate://self/resources/fonts/Inter-BoldItalic.ttf") } }

    H1 = <Label> {
        draw_text: {
            text_style: <FONT_BOLD> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_1)
            }
            color: (COLOR_TEXT)
        }
        text: "Headline H1"
    }
    H1italic = <Label> {
        draw_text: {
            text_style: <FONT_BOLD_ITALIC> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_1)
            }
            color: (COLOR_TEXT)
        }
        text: "Headline H1"
    }
    H2 = <Label> {
        draw_text: {
            text_style: <FONT_BOLD> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_2)
            }
            color: (COLOR_TEXT)
        }
        text: "Headline H2"
    }
    H2italic = <Label> {
        draw_text: {
            text_style: <FONT_BOLD_ITALIC> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_2)
            }
            color: (COLOR_TEXT)
        }
        text: "Headline H2"
    }
    H3 = <Label> {
        draw_text: {
            text_style: <FONT_BOLD> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_3)
            }
            color: (COLOR_TEXT)
        }
        text: "Headline H3"
    }
    H3italic = <Label> {
        draw_text: {
            text_style: <FONT_BOLD_ITALIC> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_3)
            }
            color: (COLOR_TEXT)
        }
        text: "Headline H3"
    }
    H4 = <Label> {
        draw_text: {
            text_style: <FONT_BOLD> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_4)
            }
            color: (COLOR_TEXT)
        }
        text: "Headline H4"
    }
    H4italic = <Label> {
        draw_text: {
            text_style: <FONT_BOLD_ITALIC> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_4)
            }
            color: (COLOR_TEXT)
        }
        text: "Headline H4"
    }

    P = <Label> {
        draw_text: {
            text_style: <FONT_REGULAR> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_P)
            }
            color: (COLOR_TEXT)
        }
        text: "Paragraph"
    }
    Meta = <Label> {
        draw_text: {
            text_style: <FONT_REGULAR> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_META)
            }
            color: (COLOR_META)
        }
        text: "Meta data"
    }
    Pbold = <Label> {
        draw_text: {
            text_style: <FONT_BOLD> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_P)
            }
            color: (COLOR_TEXT)
        }
        text: "Paragraph"
    }
    Pitalic = <Label> {
        draw_text: {
            text_style: <FONT_ITALIC> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_P)
            }
            color: (COLOR_TEXT)
        }
        text: "Paragraph"
    }
    Pbolditalic = <Label> {
        draw_text: {
            text_style: <FONT_BOLD_ITALIC> {
                line_spacing: 1.5,
                font_size: (FONT_SIZE_P)
            }
            color: (COLOR_TEXT)
        }
        text: "Paragraph"
    }

    Timestamp = <Label> { width: Fit, align: {x: 1.0, y: 0.5}}

    // COMPONENTS        
    Filler = <View> {
        width: Fill, height: Fill,
        draw_bg: {color: (COLOR_U_0)}
    }

    OsHeader = <View> {
        width: Fill, height: 25.0,
        flow: Down
        margin: <MSPACE_0> {}, padding: <MSPACE_0> {},
        spacing: (SPACE_0),
    }
    OsFooter = <View> {
        width: Fill, height: 50.0,
        flow: Right
        margin: <MSPACE_0> {}, padding: <MSPACE_0> {},
        spacing: (SPACE_0),
    }

    Divider = <RectView> {
        margin: <MSPACE_0> {}, padding: <MSPACE_0> {},
        show_bg: true,
        draw_bg: {
            color: (COLOR_D_1),
            border_color: #0000,
            inset: vec4(0.0, 0.0, 0.0, 0.0)
        }
    }
    DividerH = <Divider> { height: 3.0, width: Fill, }
    DividerV = <Divider> { height: Fill, width: 3.0, }

    ActionIcon = <Button> {
        width: 8., height: Fit,
        align: { x: 0.5, y: 0.5 },
        margin: 5
        padding: <MSPACE_0> {},
        draw_icon: {
            svg_file: (ICON_ARR_R),
            fn get_color(self) -> vec4 { return (COLOR_D_3) }
        }
        icon_walk: {
            width: 8., height: Fit
        }
        draw_bg: {
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                return sdf.result
            }
        }
        text: ""
    }

    IconButton = <Button> {
        width: Fit,
        align: { x: 0.5, y: 0.5 },
        draw_icon: {
            svg_file: (ICON_CREATE),
            fn get_color(self) -> vec4 { return (COLOR_D_5) }
        }
        icon_walk: {width: 15.0, height: Fit}
        draw_bg: {
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                return sdf.result
            }
        }
        padding: <MSPACE_0> {},
        text: ""
    }

    OptionsItem = <View> {
        flow: Down,
        width: Fill, height: Fit

        content = <View> {
            flow: Right,
            width: Fill, height: Fit,
            spacing: (SPACE_2),
            margin: <MSPACE_2> {}, padding: <MSPACE_2> {}
            align: { x: 0.0, y: 0.5},

            icon = <Image> {
                width: 20., height: 20.
            }

            label = <H4> {
                width: Fit, height: Fit,
            }
            <Filler> {}
            action_icon = <ActionIcon> {}
        }
    }

    Options = <View> {
        width: Fill, height: Fit,
        flow: Down
        margin: <MSPACE_V_2> {}, padding: {bottom: (SPACE_2)},
        spacing: 0.,
        show_bg: false,
    }

    SearchBar = <View> {
        width: Fill, height: Fit
        show_bg: false,

            input = <TextInput> {
                width: Fill, height: Fit,
                align: {y: 0.0},

                cursor_margin_bottom: 3.0,
                cursor_margin_top: 4.0,
                select_pad_edges: 3.0
                cursor_size: 2.0,
                on_focus_select_all: false,
                empty_message: "Search ...",
                margin: <MSPACE_H_2> {},

                draw_bg: {
                    instance hover: 0.0
                    instance focus: 0.0
                    border_width: 1.0
                    fn get_color(self) -> vec4 {
                        return mix( (COLOR_D_3), (COLOR_D_0), self.pos.y + mix(0.0, 0.5, self.focus) )
                    }

                    fn get_border_color(self) -> vec4 {
                        return mix(
                            mix((COLOR_U_0), (COLOR_U), self.pos.y),
                            mix((COLOR_U_0),(COLOR_U), self.pos.y),
                            self.focus)
                    }

                }

                draw_text: {
                    instance focus: 0.0

                    text_style: {
                        // font_size: (FONT_SIZE_BASE + 2 * (FONT_SIZE_CONTRAST))
                        font_size: (FONT_SIZE_P) 
                        font: {path: dep("crate://self/resources/fonts/Inter-Bold.ttf")}
                    },

                    fn get_color(self) -> vec4 {
                        return
                            mix(
                                (COLOR_D_6),
                                (COLOR_D_4),
                                self.is_empty
                            )
                    }
                }

                draw_cursor: {
                    instance focus: 0.0
                    uniform border_radius: 0.5
                    fn pixel(self) -> vec4 {
                        // let sdf = sdf2d::viewport(self.pos * self.rect_size);
                        // sdf.fill(mix(#ccc, #f, self.focus));
                        // return sdf.result
                        return mix((COLOR_D_0), (COLOR_ACCENT), self.focus)
                    }
                }

                draw_select: {
                    instance hover: 0.0
                    instance focus: 0.0
                    uniform border_radius: 2.0
                    fn pixel(self) -> vec4 {
                        //return mix(#f00,#0f0,self.pos.y)
                        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                        sdf.box(
                            0.,
                            0.,
                            self.rect_size.x,
                            self.rect_size.y,
                            self.border_radius
                        )
                        sdf.fill(mix((COLOR_U_0), (COLOR_SELECT), self.focus)); // Pad color
                        return sdf.result
                    }
                }

                animator: {
                    hover = {
                        default: off
                        off = {
                            from: {all: Forward {duration: 0.25}}
                            apply: {
                                draw_select: {hover: 0.0}
                                draw_text: {hover: 0.0}
                                draw_bg: {hover: 0.0}
                            }
                        }
                        on = {
                            from: {all: Forward {duration: 0.1}}
                            apply: {
                                draw_select: {hover: 1.0}
                                draw_text: {hover: 1.0}
                                draw_bg: {hover: 1.0}
                            }
                        }
                    }

                // text_style: { font_size: (FONT_SIZE_BASE + 5 * (FONT_SIZE_CONTRAST)) },

                    focus = {
                        default: off
                        off = {
                            redraw: true
                            from: {all: Forward {duration: 1.}}
                            ease: OutElastic
                            apply: {
                                draw_cursor: {focus: 0.0},
                                draw_select: {focus: 0.0}
                                draw_text: {
                                    text_style: {
                                        font_size: 10.0
                                    } 
                                }
                                draw_bg: {focus: 0.0}
                            }
                        }
                        on = {
                            redraw: true
                            from: {all: Forward {duration: 1.}}
                            ease: OutElastic
                            apply: {
                                draw_cursor: {focus: 1.0},
                                draw_select: {focus: 1.0}
                                draw_text: {
                                    text_style: {
                                        font_size: 15.0
                                    } 
                                }
                                draw_bg: {focus: 1.0}
                            }
                        }
                    }
                }

            }
        }
}

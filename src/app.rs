use crate::home::rooms_list::RoomListAction;
use crate::home::room_screen::*;
use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_draw::shader::std::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::avatar::Avatar; // TODO: REMOVE?
    import crate::home::home_screen::HomeScreen
    import crate::home::room_screen::RoomScreen
    import crate::contacts::contacts_screen::ContactsScreen
    import crate::contacts::add_contact_screen::AddContactScreen
    import crate::discover::discover_screen::DiscoverScreen
    import crate::discover::moments_screen::MomentsScreen
    import crate::profile::profile_screen::ProfileScreen
    import crate::profile::my_profile_screen::MyProfileScreen

    import crate::shared::clickable_view::ClickableView
    import crate::shared::stack_navigation::*; // TODO: REMOVE?

    ICON_CHAT = dep("crate://self/resources/icons/chat.svg")
    ICON_CONTACTS = dep("crate://self/resources/icons/contacts.svg")
    ICON_DISCOVER = dep("crate://self/resources/icons/discover.svg")
    ICON_ME = dep("crate://self/resources/icons/me.svg")

    H3_TEXT_REGULAR = {
        font_size: 9.0,
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }

    AppTab = <RadioButton> {
        width: Fit,
        height: Fill,
        align: {x: 0.0, y: 0.0}
        draw_radio: {
            radio_type: Tab,
            color_active: #fff,
            color_inactive: #fff,
        }
        draw_text: {
            color_selected: #0b0,
            color_unselected: #000,
            color_unselected_hover: #111,
            text_style: <H3_TEXT_REGULAR> {}
        }
    }

    Legacy = <View> {
        navigation = <StackNavigation> {
            root_view = {
                width: Fill,
                height: Fill,
                padding: 0, align: {x: 0.0, y: 0.0}, spacing: 0., flow: Down

                application_pages = <View> {
                    width: Fill,
                    margin: 0.0,
                    padding: 0.0

                    tab1_frame = <HomeScreen> {visible: true}
                    tab2_frame = <ContactsScreen> {visible: false}
                    tab3_frame = <DiscoverScreen> {visible: false}
                    tab4_frame = <ProfileScreen> {visible: false}
                }

                mobile_menu = <RoundedView> {
                    width: Fill,
                    height: 80,
                    flow: Right, spacing: 6.0, padding: 10
                    draw_bg: {
                        instance radius: 0.0,
                        instance border_width: 1.0,
                        instance border_color: #aaa,
                        color: #fff
                    }

                    mobile_modes = <View> {
                        tab1 = <AppTab> {
                            animator: {selected = {default: on}}
                            label: "Rooms"
                            draw_icon: {
                                svg_file: (ICON_CHAT),
                                fn get_color(self) -> vec4 {
                                    return mix(
                                        #000,
                                        #0b0,
                                        self.selected
                                    )
                                }
                            }
                            width: Fill,
                            icon_walk: {width: 20, height: 20}
                            flow: Down, spacing: 5.0, align: {x: 0.5, y: 0.5}
                        }
                        tab2 = <AppTab> {
                            label: "DMs",
                            draw_icon: {
                                svg_file: (ICON_CONTACTS),
                                fn get_color(self) -> vec4 {
                                    return mix(
                                        #000,
                                        #0b0,
                                        self.selected
                                    )
                                }
                            }
                            width: Fill
                            icon_walk: {width: 20, height: 20}
                            flow: Down, spacing: 5.0, align: {x: 0.5, y: 0.5}
                        }
                        tab3 = <AppTab> {
                            label: "Spaces",
                            draw_icon: {
                                svg_file: (ICON_DISCOVER),
                                fn get_color(self) -> vec4 {
                                    return mix(
                                        #000,
                                        #0b0,
                                        self.selected
                                    )
                                }
                            }
                            width: Fill
                            icon_walk: {width: 20, height: 20}
                            flow: Down, spacing: 5.0, align: {x: 0.5, y: 0.5}
                        }
                        tab4 = <AppTab> {
                            label: "Profile",
                            draw_icon: {
                                svg_file: (ICON_ME),
                                fn get_color(self) -> vec4 {
                                    return mix(
                                        #000,
                                        #0b0,
                                        self.selected
                                    )
                                }
                            }
                            width: Fill
                            icon_walk: {width: 20, height: 20}
                            flow: Down, spacing: 5.0, align: {x: 0.5, y: 0.5}
                        }
                    }
                }
            }

            moments_stack_view = <StackNavigationView> {
                header = {
                    content = {
                        title_container = {
                            title = {
                                text: "Moments"
                            }
                        }
                    }
                }
                body = {
                    <MomentsScreen> {}
                }
            }

            add_contact_stack_view = <StackNavigationView> {
                header = {
                    content = {
                        title_container = {
                            title = {
                                text: "Add Contact"
                            }
                        }
                    }
                }
                body = {
                    <AddContactScreen> {}
                }
            }

            my_profile_stack_view = <StackNavigationView> {
                header = {
                    content = {
                        title_container = {
                            title = {
                                text: "My Profile"
                            }
                        }
                    }
                }
                body = {
                    <MyProfileScreen> {}
                }
            }

            rooms_stack_view = <StackNavigationView> {
                header = {
                    content = {
                        title_container = {
                            title = {
                                text: "Loading room..."
                            }
                        }
                    }
                }
                body = {
                    room_screen = <RoomScreen> {}
                }
            }
        }
    }

    SpacesSidebar = <Rows> {
    margin: <MSPACE_1> {},
    spacing: (SPACE_1),
    align: {x: 0.5, y: 0.0}
    width: 35.0,

    <Avatar> {}

    <CachedRoundedView> {
        draw_bg:{radius:10.0}
        width: 35, height: 35,
        <Image> {
            source: dep("crate://self/resources/img/profile_1.jpg"),
            width: Fill, height: Fill,
            margin: 0,
        }
    }

    <Filler> {}

    <CachedRoundedView> {
        draw_bg:{radius:10.0}
        width: 35, height: 35,
        <Image> {
            source: dep("crate://self/resources/img/profile_1.jpg"),
            width: Fill, height: Fill,
            margin: 0,
        }
    }

    <CachedRoundedView> {
        draw_bg:{radius:10.0}
        width: 35, height: 35,
        <Image> {
            source: dep("crate://self/resources/img/profile_1.jpg"),
            width: Fill, height: Fill,
            margin: 0,
        }
    }

    <CachedRoundedView> {
        draw_bg:{radius:10.0}
        width: 35, height: 35,
        <Image> {
            source: dep("crate://self/resources/img/profile_1.jpg"),
            width: Fill, height: Fill,
            margin: 0,
        }
    }

    <DividerH> {}

    <IconButton> {
        draw_icon: {svg_file: (ICO_DM)},
        icon_walk: {width: 18.0, height: Fit}
        margin: <MSPACE_2> {}
    }

    <IconButton> {
        draw_icon: {svg_file: (ICO_HOME)},
        icon_walk: {width: 22.5, height: Fit}
        margin: <MSPACE_2> {}
    }


}

TypographyDemo = <View> {
    flow: Down,
    width: Fill, height: Fit,
    padding: <MSPACE_0> {},
    draw_bg: {color: (COLOR_D_0)}
    <H1> { text: "Headline H1"}
    <H1italic> { text: "Headline H1"}
    <H2> { text: "Headline H2"}
    <H2italic> { text: "Headline H2"}
    <H3> { text: "Headline H3"}
    <H3italic> { text: "Headline H3"}
    <H4> { text: "Headline H4"}
    <H4italic> { text: "Headline H4"}
    <P> {
        text: "Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum"
    }
    <Pitalic> {
        text: "Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum Lorem Ipsum"
    }
}

ChannelPreview = <View> {
    flow: Down,
    width: Fill, height: Fit,
    spacing: (SPACE_1 / 2)
    margin: <MSPACE_V_1> {},
    <View> {
        flow: Right,
        width: Fill, height: Fit,
        <Pbold> { text: "Channel Name" }
        <Filler> {}
        <Meta> { width: Fit, text: "12:15" }
    }
    <Meta> { text: "Message preview"}
}

SubSpace = <View> {
    flow: Down,
    width: Fill, height: Fit,
    spacing: (SPACE_1),
    padding: <MSPACE_V_1> {},
    <RoundedView> {
        flow: Down,
        width: Fill, height: Fit,
        draw_bg: { color: (COLOR_D_1)},
        padding: <MSPACE_2> {},
        <H4> { text: "Test" } 
    }
    <View> {
        flow: Down,
        width: Fill, height: Fit,
        spacing: (SPACE_1),
        padding: <MSPACE_H_2> {},
        <ChannelPreview> {}
        <ChannelPreview> {}
        <ChannelPreview> {}
        <ChannelPreview> {}
        <ChannelPreview> {}
    }
}

Space = <Rows> {
    margin: <MSPACE_1> {},
    spacing: (SPACE_1)
    <H2> { text: "Home"}
    <DividerH> {}
    <View> {
        scroll_bars: <ScrollBars> {show_scroll_x: false, show_scroll_y: true}
        flow: Down,
        margin: {top: (SPACE_1 * -1)}
        <Filler> {}
        <View> {
            flow: Down,
            spacing: (SPACE_1),
            width: Fill, height: Fit,
            <SubSpace> {}
            <SubSpace> {}
            <SubSpace> {}
        }
    }
}

SearchBar = <View> {
    width: Fill, height: Fit
    show_bg: false,
    draw_bg: { color: (COLOR_D_0) }

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
                    return mix( #A, (COLOR_U), self.pos.y + mix(0.0, 0.5, self.focus) )
                }

                fn get_border_color(self) -> vec4 {
                    return mix(
                        mix((COLOR_U_0), (COLOR_U), self.pos.y),
                        mix((COLOR_D_2),(COLOR_U_0), self.pos.y),
                        self.focus)
                }

            }

            draw_text: {
                instance focus: 0.0

                text_style: {
                    // font_size: (FONT_SIZE_BASE + 2 * (FONT_SIZE_CONTRAST))
                    font_size: 10.0
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
                                    font_size: 20.0
                                } 
                            }
                            draw_bg: {focus: 1.0}
                        }
                    }
                }
            }

        }
    }

    SpaceSearch = <Rows> {
        height: Fit,
        <SearchBar> {
            draw_bg: {color: (COLOR_U_0)}
        }
    }



    ReworkedUI = <View> {
        width: Fill, height: Fill,
        margin: <MSPACE_0> {}, padding: <MSPACE_1> {},
        spacing: (SPACE_1), flow: Down,

        <OsHeader> {}

        <Columns> {
            <SpacesSidebar> {}
            <Space> {}
        }
        <SpaceSearch> {}
        <OsFooter> {}
    } 


    App = {{App}} {
        ui: <Window> {
            window: {inner_size: vec2(400, 800)},
            pass: {clear_color: (COLOR_BG) }

            body = {
                // <ReworkedUI> {}
                <Legacy> {}
            }
        }
    }
}

app_main!(App);

#[derive(Live)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        makepad_widgets::live_design(cx);

        // shared
        crate::shared::styles::live_design(cx);
        crate::shared::helpers::live_design(cx);
        crate::shared::header::live_design(cx);
        crate::shared::search_bar::live_design(cx);
        crate::shared::popup_menu::live_design(cx);
        crate::shared::dropdown_menu::live_design(cx);
        crate::shared::clickable_view::live_design(cx);
        crate::shared::avatar::live_design(cx);

        // home - chats
        crate::home::home_screen::live_design(cx);
        crate::home::rooms_list::live_design(cx);
        crate::home::room_screen::live_design(cx);

        // contacts
        crate::contacts::contacts_screen::live_design(cx);
        crate::contacts::contacts_group::live_design(cx);
        crate::contacts::contacts_list::live_design(cx);
        crate::contacts::add_contact_screen::live_design(cx);

        // discover
        crate::discover::discover_screen::live_design(cx);
        crate::discover::moment_list::live_design(cx);
        crate::discover::moments_screen::live_design(cx);

        // profile
        crate::profile::profile_screen::live_design(cx);
        crate::profile::my_profile_screen::live_design(cx);
    }
}
impl LiveHook for App {
    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        log!("after_new_from_doc(): starting matrix sdk loop");
        crate::sliding_sync::start_matrix_tokio().unwrap();
    }
}

impl MatchEvent for App {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        self.ui.radio_button_set(ids!(
            mobile_modes.tab1,
            mobile_modes.tab2,
            mobile_modes.tab3,
            mobile_modes.tab4,
        ))
        .selected_to_visible(
            cx,
            &self.ui,
            &actions,
            ids!(
                application_pages.tab1_frame,
                application_pages.tab2_frame,
                application_pages.tab3_frame,
                application_pages.tab4_frame,
            ),
        );

        self.handle_rooms_list_action(&actions);

        let mut navigation = self.ui.stack_navigation(id!(navigation));
        navigation.handle_stack_view_actions(cx, &actions);
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        // Forward events to the MatchEvent trait impl, and then to the App's UI element.
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

impl App {
    fn handle_rooms_list_action(&mut self, actions: &Actions) {
        for action in actions {
            // Handle the user selecting a room to view (a RoomPreview in the RoomsList).
            if let RoomListAction::Selected { room_index: _, room_id, room_name } = action.as_widget_action().cast() {
                let stack_navigation = self.ui.stack_navigation(id!(navigation));
                
                // Set the title of the RoomScreen's header to the room name.
                stack_navigation.set_title(
                    live_id!(rooms_stack_view),
                    &room_name.unwrap_or_else(|| format!("Room {}", &room_id)),
                );

                // Get a reference to the `RoomScreen` widget and tell it which room's data to show.
                stack_navigation
                    .room_screen(id!(rooms_stack_view.room_screen))
                    .set_displayed_room(room_id);
            }
        }
    }
}

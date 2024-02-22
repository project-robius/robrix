use crate::home::rooms_list::RoomListAction;
use crate::home::room_screen::*;
use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_draw::shader::std::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::avatar::Avatar;
    import crate::home::home_screen::HomeScreen
    import crate::home::room_screen::RoomScreen
    import crate::contacts::contacts_screen::ContactsScreen
    import crate::contacts::add_contact_screen::AddContactScreen
    import crate::discover::discover_screen::DiscoverScreen
    import crate::discover::moments_screen::MomentsScreen
    import crate::profile::profile_screen::ProfileScreen
    import crate::profile::my_profile_screen::MyProfileScreen

    import crate::shared::clickable_view::ClickableView
    // import crate::shared::stack_navigation::*;

    SpaceAvatar = <Avatar> {
        width: 32.5, height: 32.5,

        // the text_view and img_view are overlaid on top of each other.
        flow: Overlay
        // centered horizontally and vertically.
        align: { x: 0.5, y: 0.5 }

        text_view = <View> {
            visible: true,
            align: { x: 0.5, y: 0.5 }
            show_bg: true,
            draw_bg: {
                instance background_color: (COLOR_D_7),
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    let c = self.rect_size * 0.5;
                    sdf.circle(c.x, c.x, c.x)
                    sdf.fill_keep(self.background_color);
                    return sdf.result
                }
            }
            
            text = <Pbold> {
                width: Fit, height: Fit,
                draw_text: {
                    text_style: {
                        font_size: 12.5,
                        top_drop: 1.1
                    },
                    color: (COLOR_U)
                }
                text: "B"
            }
        }
    }

    ChanAvatar = <Avatar> {
        width: 30.0, height: 30.0,

        // the text_view and img_view are overlaid on top of each other.
        flow: Overlay

        // centered horizontally and vertically.
        align: { x: 0.5, y: 0.5 }

        text_view = <View> {
            visible: true,
            align: { x: 0.5, y: 0.5 }
            show_bg: true,
            draw_bg: {
                instance background_color: (COLOR_U),
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    let c = self.rect_size * 0.5;
                    sdf.circle(c.x, c.x, c.x)
                    sdf.fill_keep(self.background_color);
                    return sdf.result
                }
            }
            
            text = <Pbold> {
                width: Fit, height: Fit,

                draw_text: {
                    text_style: {
                        font_size: 12.5,
                        top_drop: 1.1
                    },
                    color: (COLOR_D_6)
                }
                text: "B"
            }
        }
    }

    AppTab = <RadioButton> {
        width: 30, height: 30,
        flow: Down,
        icon_walk: { width: 30.0 }
        draw_icon: {
            svg_file: (ICON_ME),
            fn get_color(self) -> vec4 {
                return mix(
                    (COLOR_D_5),
                    (COLOR_ACCENT),
                    self.selected
                )
            }
        }
        draw_radio: {
            radio_type: Tab,
            color_active: (COLOR_U_0),
            color_inactive: (COLOR_U_0),
        }
        label: "",
    }

    DummyCommunityTMP = <CachedRoundedView> {
        width: 32.5, height: 32.5,
        draw_bg:{radius:10.0}
        <Image> {
            width: Fill, height: Fill,
            source: (IMG_TESTUSER),
            margin: <MSPACE_0> {},
        }
    }

    HomeView = <View> {
        width: Fill, height: Fill,
        margin: <MSPACE_0> {}, padding: <MSPACE_1> {},
        flow: Down,
        spacing: (SPACE_2),
        <HomeScreen> {visible: true}
    } 

    App = {{App}} {
        ui: <Window> {
            window: {inner_size: vec2(400, 800)},
            pass: {clear_color: (COLOR_BG) }

            body = {
                flow: Down,
                <OsHeader> {}
                
                navigation = <StackNavigation> {
                    root_view = {
                        width: Fill, height: Fill,
                        flow: Right
                        align: {x: 0.0, y: 0.0},
                        padding: <MSPACE_0> {},
                        spacing: (SPACE_0),

                        mobile_menu = <View> {
                            width: 60, height: Fill,
                            flow: Right,
                            align: { x: 0.5, y: 0.5 }
                            
                            margin: <MSPACE_0> {}, padding: <MSPACE_0> {},
                            mobile_modes = <View> {
                                flow: Down,
                                align: { x: 0.5, y: 0.5 },
                                margin: <MSPACE_2> {}, spacing: (SPACE_3)

                                tab1 = <AppTab> { draw_icon: { svg_file: (ICON_ME) } }
                                tab2 = <AppTab> { draw_icon: { svg_file: (ICON_SETTINGS) } }
                                <Filler> {}
                                <IconButton> {
                                    draw_icon: { svg_file: (ICON_CREATE) },
                                    icon_walk: { width: 25.0 }
                                }
                                <DividerH> {}
                                <SpaceAvatar> {}
                                <DummyCommunityTMP> {}
                                <DummyCommunityTMP> {}
                                <DummyCommunityTMP> {}
                                <DummyCommunityTMP> {}
                                <DividerH> {}
                                tab3 = <AppTab> { draw_icon: { svg_file: (ICON_CHAT) } }
                                tab4 = <AppTab> {
                                    animator: { selected = {default: on} }
                                    draw_icon: { svg_file: (ICON_HOME) }
                                }
                            }
                        }

                        application_pages = <View> {
                            width: Fill,
                            margin: 0.0, padding: 0.0,

                            // tab1_frame = <HomeScreen> {visible: true}
                            tab1_frame = <ProfileScreen> {visible: false}
                            tab2_frame = <ContactsScreen> {visible: false}
                            tab3_frame = <DiscoverScreen> {visible: false}
                            tab4_frame = <HomeView> { visible: true}
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
                            align: { x: 0.5, y: 0.5 },
                            padding: <MSPACE_V_2> {},
                            show_bg: true,
                            content = {
                                title_container = {
                                    align: {x: 0.5, y: 0.5}
                                    margin: { left: 35.0, }
                                    spacing: (SPACE_2)
                                    title = {
                                        draw_text: {
                                            text_style: { 
                                                font: {path: dep("crate://self/resources/fonts/Inter-Bold.ttf")},
                                                font_size: (FONT_SIZE_4)
                                            }
                                            color: (COLOR_TEXT)
                                        }
                                        text: "Loading Room..."
                                    }
                                    <ChanAvatar> { width: 25., height: 25.}
                                    <Filler> {}
                                    <IconButton> {
                                        draw_icon: { svg_file: (ICON_USERS) }
                                        icon_walk: { width: 20.0, height: Fit }
                                    }
                                    <IconButton> {
                                        margin: { right: (SPACE_2 * 1.5) }
                                        draw_icon: { svg_file: (ICON_SETTINGS) }
                                        icon_walk: { width: 16.0, height: Fit }
                                    }
                                }
                            }
                        }

                        body = {
                            height: Fill,
                            room_screen = <RoomScreen> {}
                        }
                    }
                }
                <OsFooter> {}
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

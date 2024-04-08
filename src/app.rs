use crate::home::rooms_list::RoomListAction;
use crate::home::room_screen::*;
use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::home::home_screen::HomeScreen
    import crate::home::room_screen::RoomScreen
    import crate::contacts::contacts_screen::ContactsScreen
    import crate::contacts::add_contact_screen::AddContactScreen
    import crate::discover::discover_screen::DiscoverScreen
    import crate::discover::moments_screen::MomentsScreen
    import crate::profile::profile_screen::ProfileScreen
    import crate::profile::my_profile_screen::MyProfileScreen

    import crate::shared::clickable_view::ClickableView

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

    App = {{App}} {
        ui: <Window> {
            window: {inner_size: vec2(400, 800)},
            pass: {clear_color: #2A}

            body = {
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
        crate::shared::text_or_image::live_design(cx);
        crate::shared::html_or_plaintext::live_design(cx);

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

impl LiveHook for App { }

impl MatchEvent for App {
    fn handle_startup(&mut self, _cx: &mut Cx) {
        log!("App::handle_startup(): starting matrix sdk loop");
        crate::sliding_sync::start_matrix_tokio().unwrap();
    }
    fn handle_shutdown(&mut self, _cx: &mut Cx) {
        log!("App::handle_shutdown()");
    }
    fn handle_foreground(&mut self, _cx: &mut Cx) {
        log!("App::handle_foreground()");
    }
    fn handle_background(&mut self, _cx: &mut Cx) {
        log!("App::handle_background()");
    }
    fn handle_pause(&mut self, _cx: &mut Cx) {
        log!("App::handle_pause()");
    }
    fn handle_resume(&mut self, _cx: &mut Cx) {
        log!("App::handle_resume()");
    }
    fn handle_app_got_focus(&mut self, _cx: &mut Cx) {
        log!("App::handle_app_got_focus()");
    }
    fn handle_app_lost_focus(&mut self, _cx: &mut Cx) {
        log!("App::handle_app_lost_focus()");
    }
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

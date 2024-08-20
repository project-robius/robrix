use crate::home::rooms_list::RoomListAction;
use crate::home::room_screen::*;
use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::clickable_view::ClickableView
    import crate::shared::portal::*;
    import crate::home::home_screen::HomeScreen
    import crate::home::room_screen::RoomScreen
    import crate::contacts::contacts_screen::ContactsScreen
    import crate::contacts::add_contact_screen::AddContactScreen
    import crate::discover::discover_screen::DiscoverScreen
    import crate::discover::moments_screen::MomentsScreen
    import crate::profile::profile_screen::ProfileScreen
    import crate::profile::my_profile_screen::MyProfileScreen

    ICON_CHAT = dep("crate://self/resources/icons/chat.svg")
    ICON_CONTACTS = dep("crate://self/resources/icons/contacts.svg")
    ICON_DISCOVER = dep("crate://self/resources/icons/discover.svg")
    ICON_ME = dep("crate://self/resources/icons/me.svg")


    APP_TAB_COLOR = #344054
    APP_TAB_COLOR_HOVER = #636e82
    APP_TAB_COLOR_SELECTED = #091

    AppTab = <RadioButton> {
        width: Fit,
        height: Fill,
        flow: Down,
        align: {x: 0.5, y: 0.5},

        icon_walk: {width: 20, height: 20, margin: 0.0}
        label_walk: {margin: 0.0}

        draw_radio: {
            radio_type: Tab,

            // Draws a horizontal line under the tab when selected or hovered.
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                sdf.box(
                    20.0,
                    self.rect_size.y - 2.5,
                    self.rect_size.x - 40,
                    self.rect_size.y - 4,
                    0.5
                );
                sdf.fill(
                    mix(
                        mix(
                            #0000,
                            (APP_TAB_COLOR_HOVER),
                            self.hover
                        ),
                        (APP_TAB_COLOR_SELECTED),
                        self.selected
                    )
                );
                return sdf.result;
            }
        }

        draw_text: {
            color_unselected: (APP_TAB_COLOR)
            color_unselected_hover: (APP_TAB_COLOR_HOVER)
            color_selected: (APP_TAB_COLOR_SELECTED)

            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color_unselected,
                        self.color_unselected_hover,
                        self.hover
                    ),
                    self.color_selected,
                    self.selected
                )
            }
        }

        draw_icon: {
            instance color_unselected: (APP_TAB_COLOR)
            instance color_unselected_hover: (APP_TAB_COLOR_HOVER)
            instance color_selected: (APP_TAB_COLOR_SELECTED)
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color_unselected,
                        self.color_unselected_hover,
                        self.hover
                    ),
                    self.color_selected,
                    self.selected
                )
            }
        }
    }

    App = {{App}} {
        ui: <Window> {
            window: {inner_size: vec2(1280, 800)},
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
                            height: 75,
                            flow: Right,
                            // spacing: 6.0,
                            padding: {bottom: 5.0},
                            draw_bg: {
                                instance radius: 0.0,
                                instance border_width: 0.0,
                                instance border_color: #aaa,
                                color: #fff
                            }

                            mobile_modes = <View> {
                                tab1 = <AppTab> {
                                    animator: {selected = {default: on}}
                                    text: "Rooms"
                                    draw_icon: {
                                        svg_file: (ICON_CHAT),
                                    }
                                    width: Fill,
                                    flow: Down,
                                    spacing: 5.0,
                                    align: {x: 0.5, y: 0.5}
                                }
                                tab2 = <AppTab> {
                                    text: "DMs",
                                    draw_icon: {
                                        svg_file: (ICON_CONTACTS),
                                    }
                                    width: Fill
                                    flow: Down,
                                    spacing: 5.0,
                                    align: {x: 0.5, y: 0.5}
                                }
                                tab3 = <AppTab> {
                                    text: "Spaces",
                                    draw_icon: {
                                        svg_file: (ICON_DISCOVER),
                                    }
                                    width: Fill
                                    flow: Down,
                                    spacing: 5.0,
                                    align: {x: 0.5, y: 0.5}
                                }
                                tab4 = <AppTab> {
                                    text: "Profile",
                                    draw_icon: {
                                        svg_file: (ICON_ME),
                                    }
                                    width: Fill
                                    flow: Down,
                                    spacing: 5.0,
                                    align: {x: 0.5, y: 0.5}
                                }
                            }
                        }
                    }

                    rooms_stack_view = <StackNavigationView> {
                        header = {
                            content = {
                                title_container = {
                                    padding: { bottom: 0 }
                                    title = {
                                        width: Fit,
                                        text: "Loading room..."
                                        draw_text: {
                                            color: (MESSAGE_TEXT_COLOR),
                                        }
                                    }
                                }
                                button_container = { left_button = {
                                    icon_walk: {width: 12, height: 68},
                                } }
                            }
                        }
                        body = {
                            room_screen = <RoomScreen> {}
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

                } // end of StackNavigation

            } // end of body
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
        // Order matters here, as some widget definitions depend on others.
        // `makepad_widgets` must be registered first,
        // then `shared`` widgets (in which styles are defined),
        // then other modules widgets.
        makepad_widgets::live_design(cx);
        crate::shared::live_design(cx);
        crate::home::live_design(cx);
        crate::contacts::live_design(cx);
        crate::discover::live_design(cx);
        crate::profile::live_design(cx);
    }
}

impl LiveHook for App { }

impl MatchEvent for App {
    fn handle_startup(&mut self, _cx: &mut Cx) {
        log!("App::handle_startup(): starting matrix sdk loop");
        crate::sliding_sync::start_matrix_tokio().unwrap();
    }
    /*
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
    */

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        // Handle the user selecting a tab in the mobile menu on bottom.
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

        // Handle stack navigation actions, e.g., navigating to/from a room screen
        let mut stack_navigation = self.ui.stack_navigation(id!(navigation));
        stack_navigation.handle_stack_view_actions(cx, &actions);


        for action in actions {
            // Handle the user selecting a room to view (a RoomPreview in the RoomsList).
            if let RoomListAction::Selected { room_index: _, room_id, room_name } = action.as_widget_action().cast() {                
                // Set the title of the RoomScreen's header to the room name.
                let displayed_room_name = room_name.unwrap_or_else(|| format!("Room ID {}", &room_id));
                stack_navigation.set_title(live_id!(rooms_stack_view), &displayed_room_name);
                // Get a reference to the `RoomScreen` widget and tell it which room's data to show.
                stack_navigation
                    .room_screen(id!(rooms_stack_view.room_screen))
                    .set_displayed_room(displayed_room_name, room_id);
            }
        }
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        // Forward events to the MatchEvent trait impl, and then to the App's UI element.
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

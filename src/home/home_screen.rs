use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::home::main_content::MainContent;
    import crate::home::rooms_sidebar::RoomsSideBar;
    import crate::home::spaces_dock::SpacesDock;
    import crate::shared::styles::*;
    import crate::shared::adaptive_layout_view::AdaptiveLayoutView;

    HomeScreen = <AdaptiveLayoutView> {
        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY)
        }
        composition: {
            desktop: {
                layout: {
                    flow: Right
                },
                // walk: {}
                // navigation: None -> TODO: this does not work, user must not use None, we might instead remove the wrapping Option
            },
            mobile: {
                layout: {
                    flow: Down
                    padding: {top: 40.}
                },
                navigation: {
                    mode: Stack,
                    items: [rooms_sidebar, main_content]
                }
                child_order: [rooms_sidebar, spaces]
            }
            // @media (width <= 1250px) {
        }

        spaces = <SpacesDock> {}
        rooms_sidebar = <RoomsSideBar> {}
        main_content = <AdaptiveLayoutView> {
            composition: {
                desktop: {
                    view_presence: Visible
                },
                mobile: {
                    view_presence: NavigationItem
                }
            }
            <MainContent> {}
        }
    }
}

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
                flow: Right
            },
            mobile: {
                flow: Down
                padding: {top: 40.}
                navigation: {
                    mode: Stack,
                    items: [rooms_sidebar, main_content]
                }
                child_order: [rooms_sidebar, spaces]
            }
        }

        spaces = <SpacesDock> {}
        rooms_sidebar = <RoomsSideBar> {}
        main_content = <AdaptiveLayoutView> {
            composition: {
                desktop: {
                    visibility: Visible
                },
                mobile: {
                    visibility: NavigationItem
                }
            }
            <MainContent> {}
        }
    }
}

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
        // flow: Right
        composition: {
            desktop: { // AdaptiveProps. 
                layout: { // AdaptiveLayout TODO: Flatten
                    flow: Right
                }
            },
            mobile: {
                layout: {
                    flow: Stacked
                    padding: {top: 40.}
                }
            }
            // @media (width <= 1250px) {
        }

        spaces = <SpacesDock> {}
        rooms = <RoomsSideBar> {}
        main_content = <AdaptiveLayoutView> {
            <MainContent> {}
        }

        // Section { // StackableSection
        //     flow: Right
        //     <A> {}
        //     <B> {}
        // }
    }
}

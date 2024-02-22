use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::styles::*;
    

    import crate::shared::header::HeaderDropDownMenu;
    import crate::home::rooms_list::RoomsList;

    HomeScreen = <View> {
        width: Fill, height: Fill
        flow: Down
        show_bg: true,
        draw_bg: { color: (COLOR_U) }
        // <HeaderDropDownMenu> {}
        <RoomsList> {}
    }
}

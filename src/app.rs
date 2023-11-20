use makepad_widgets::*;
   
live_design!{
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::timeline::timeline_view::Timeline;

    IMG_A = dep("crate://self/resources/neom-THlO6Mkf5uI-unsplash.jpg")
    IMG_PROFILE_A = dep("crate://self/resources/profile_1.jpg")
    IMG_PROFILE_B = dep("crate://self/resources/profile_2.jpg")
    ICO_FAV = dep("crate://self/resources/icon_favorite.svg")
    ICO_COMMENT = dep("crate://self/resources/icon_comment.svg")
    ICO_REPLY = dep("crate://self/resources/icon_reply.svg")
    ICO_HOME = dep("crate://self/resources/icon_home.svg")
    ICO_FIND = dep("crate://self/resources/icon_find.svg")
    ICO_LIKES = dep("crate://self/resources/icon_likes.svg")
    ICO_USER = dep("crate://self/resources/icon_user.svg")
    ICO_ADD = dep("crate://self/resources/icon_add.svg")

    MENU_BAR_HEIGHT = 80.0
    
    FONT_SIZE_SUB = 9.5
    FONT_SIZE_P = 12.5
    
    TEXT_SUB = {
        font_size: (FONT_SIZE_SUB),
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }
    
    TEXT_P = {
        font_size: (FONT_SIZE_P),
        height_factor: 1.65,
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }
    
    COLOR_BG = #xfff8ee
    COLOR_BRAND = #xf88
    COLOR_BRAND_HOVER = #xf66
    COLOR_META_TEXT = #xaaa
    COLOR_META = #xccc
    COLOR_META_INV = #xfffa
    COLOR_OVERLAY_BG = #x000000d8
    COLOR_DIVIDER = #x00000018
    COLOR_DIVIDER_DARK = #x00000044
    COLOR_PROFILE_CIRCLE = #xfff8ee
    COLOR_P = #x999
    
    FillerY = <View> {width: Fill}
    
    FillerX = <View> {height: Fill}
    
    Logo = <Button> {
        draw_bg: {
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                return sdf.result
            }
        }
        padding: 9.0
        text: "Testing: For testing Robius app"
    }
    
    IconButton = <Button> {
        draw_text: {
            instance hover: 0.0
            instance pressed: 0.0
            text_style: {
                font_size: 11.0
            }
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        (COLOR_META_TEXT),
                        (COLOR_BRAND),
                        self.hover
                    ),
                    (COLOR_BRAND_HOVER),
                    self.pressed
                )
            }
        }
        draw_icon: {
            svg_file: (ICO_FAV),
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        (COLOR_META),
                        (COLOR_BRAND),
                        self.hover
                    ),
                    (COLOR_BRAND_HOVER),
                    self.pressed
                )
            }
        }
        icon_walk: {width: 7.5, height: Fit, margin: {left: 5.0}}
        draw_bg: {
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                return sdf.result
            }
        }
        padding: 9.0
        text: "1"
    }
    
    // The header bar at the top of each room window.
    RoomHeader = <RoundedYView> {
        width: Fill,
        height: 70
        flow: Right,
        padding: 10.0,
        spacing: 10.0
        draw_bg: {color: (COLOR_OVERLAY_BG), inset: vec4(-0.5, -0.5, -1.0, 0.0), radius: vec2(0.5, 4.5)}
        
        <Logo> {
            height: Fit,
            width: Fill,
            margin: {top: 0.0}
        }
        
    }
    
    // The MenuBar bar at the bottom of the window.
    MenuBar = <RoundedYView> {
        width: Fill,
        height: (MENU_BAR_HEIGHT),
        flow: Right,
        padding: 10.0,
        spacing: 10.0
        draw_bg: {color: (COLOR_OVERLAY_BG), inset: vec4(-0.5, 0.0, -1.0, -1.0), radius: vec2(4.5, 0.5)}
        
        <View> {
            width: Fill,
            height: Fit,
            margin: 0.0
            flow: Right,
            padding: 0.0,
            spacing: 25.0,
            align: {x: 0.5, y: 0.5}
            
            <IconButton> {draw_icon: {svg_file: (ICO_HOME)} icon_walk: {width: 30.0, height: Fit}, text: ""}
            <IconButton> {draw_icon: {svg_file: (ICO_FIND)} icon_walk: {width: 18.0, height: Fit}, text: ""}
            <IconButton> {draw_icon: {svg_file: (ICO_ADD)} icon_walk: {width: 40.0, height: Fit}, text: ""}
            <IconButton> {draw_icon: {svg_file: (ICO_LIKES)} icon_walk: {width: 20.0, height: Fit}, text: ""}
            <IconButton> {draw_icon: {svg_file: (ICO_USER)} icon_walk: {width: 15.0, height: Fit}, text: ""}
        }
    }
    
    LineH = <RoundedView> {
        width: Fill,
        height: 2,
        margin: 0.0
        padding: 0.0,
        spacing: 0.0
        draw_bg: {color: (COLOR_DIVIDER)}
    }

    Timestamp = <Label> {
        padding: { top: 10.0, bottom: 0.0, left: 0.0, right: 0.0 }
        draw_text: {
            text_style: <TEXT_SUB> {},
            color: (COLOR_META_TEXT)
        }
        text: " "
    }    
    
    App = {{App}} {
        ui: <Window> {
            window: {inner_size: vec2(428, 926), dpi_override: 2},
            show_bg: true
            
            draw_bg: {
                fn pixel(self) -> vec4 {
                    return (COLOR_BG);
                }
            }
            body = {
                flow: Overlay,
                padding: 0.0
                spacing: 0,
                align: {
                    x: 0.0,
                    y: 0.0
                },

                timeline = <Timeline> {
                    // just default content for now
                }
                
                <View> {
                    height: Fill,
                    width: Fill
                    flow: Down
                    
                    <RoomHeader> {}
                    <FillerY> {}
                    <MenuBar> {}
                }
            }
        }
    }
}

app_main!(App);

#[derive(Live)]
pub struct App {
    #[live] ui: WidgetRef,
}

impl LiveHook for App {
    fn before_live_design(cx: &mut Cx) {
        crate::makepad_widgets::live_design(cx);
        crate::timeline::timeline_view::live_design(cx);
    } 
}


impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {

        // TODO: not sure if this is the correct place to do this.
        if matches!(event, Event::Construct) {
            println!("Construct: starting matrix sdk loop");
            // crate::matrix::start_matrix_tokio().unwrap();
            crate::sliding_sync::start_matrix_tokio().unwrap();
            return;
        }

        if let Event::Draw(event) = event {
            self.ui.draw_widget_all(&mut Cx2d::new(cx, event));
            return;
        }
        
        let _actions = self.ui.handle_widget_event(cx, event);
        
        // for (item_id, item) in message_list.items_with_actions(&actions) {
        //     if item.button(id!(likes)).clicked(&actions) {
        //         log!("hello {}", item_id);
        //     }
        // }
    }
}

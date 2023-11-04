use makepad_widgets::*;
   
live_design!{
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;
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
        text: "HH:MMpm"
        
    }
    
    PostMenu = <View> {
        width: Fill,
        height: Fit,
        margin: 0.0
        flow: Down,
        padding: 0.0,
        spacing: 0.0
        
            <View> {
            width: Fill,
            height: Fit,
            margin: 0.0
            flow: Right,
            padding: 0.0,
            spacing: 10.0
            
            likes = <IconButton> {draw_icon: {svg_file: (ICO_FAV)} icon_walk: {width: 15.0, height: Fit}}
            comments = <IconButton> {draw_icon: {svg_file: (ICO_COMMENT)} icon_walk: {width: 15.0, height: Fit}, text: "7"}
            
            <FillerX> {}
            reply = <IconButton> {draw_icon: {svg_file: (ICO_REPLY)} icon_walk: {width: 15.0, height: Fit}, text: ""}
        }
    }
    
    Post = <View> {
        width: Fill,
        height: Fit,
        margin: 0.0
        flow: Down,
        padding: 0.0,
        spacing: 0.0
        
        body = <View> {
            width: Fill,
            height: Fit
            flow: Right,
            padding: 10.0,
            spacing: 10.0
            
            profile = <View> {
                align: {x: 0.5, y: 0.0} // centered horizontally, top aligned
                width: Fit,
                height: Fit,
                margin: {top: 7.5}
                flow: Down,
                padding: 0.0
                profile_img = <Image> {
                    source: (IMG_PROFILE_A)
                    margin: 0,
                    width: 50.,
                    height: 50.
                    draw_bg: {
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            let c = self.rect_size * 0.5;
                            sdf.circle(c.x, c.y, c.x - 2.)
                            sdf.fill_keep(self.get_color());
                            sdf.stroke((COLOR_PROFILE_CIRCLE), 1);
                            return sdf.result
                        }
                    }
                }
                timestamp = <Timestamp> { }
            }
            content = <View> {
                width: Fill,
                height: Fit
                flow: Down,
                padding: 0.0
                
                username = <Label> {
                    margin: {bottom: 10.0, top: 10.0}
                    draw_text: {
                        text_style: <TEXT_SUB> {},
                        color: (COLOR_META_TEXT)
                    }
                    text: "<username>"
                }
                message = <Label> {
                    width: Fill,
                    height: Fit
                    draw_text: {
                        wrap: Word,
                        text_style: <TEXT_P> {},
                        color: (COLOR_P)
                    }
                    text: ""
                }
                
                <LineH> {
                    margin: {top: 10.0, bottom: 5.0}
                }
                
                <PostMenu> {}
            }
        }
        
        <LineH> {
            draw_bg: {color: (COLOR_DIVIDER_DARK)}
        }
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

                message_list = <PortalList> {
                    auto_tail: false, // set to `true` to lock the view to the last item.
                    height: Fill,
                    width: Fill
                    flow: Down
                    TopSpace = <View> {height: 80}
                    Post = <Post> {}
                    BottomSpace = <View> {height: (MENU_BAR_HEIGHT)}
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
    } 
}

static MESSAGES: [(u64, &str, &str); 15] = [
    (0, "Kevin Boos", "First message"),
    (1, "Kevin Boos", "Second message"),
    (2, "Yue Chen", "got it"),
    (3, "Robius Test", "Send 7:14 PDT"),
    (4, "Kevin Boos", "Message 4"),
    (5, "Kevin Boos", "Message 5"),
    (6, "Kevin Boos", "Message 6"),
    (7, "Kevin Boos", "Message 7"),
    (8, "Kevin Boos", "Message 8"),
    (9, "Kevin Boos", "Message 9"),
    (10, "Kevin Boos", "Message 10"),
    (11, "Kevin Boos", "Message 11"),
    (12, "Kevin Boos", "Message 12"),
    (13, "Kevin Boos", "Message 13"),
    (14, "Kevin Boos", "Message 14"),
];

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if matches!(event, Event::Construct) {
            println!("Construct: starting matrix sdk loop");
            crate::matrix::start_matrix_tokio().unwrap();

            /*
            let message_list = self.ui.portal_list(id!(message_list));
            
            (cx, MESSAGES.len() as u64);

            if let Some(mut list) = message_list.has_widget(&next).borrow_mut() {

                let last_item_id = MESSAGES.len() as u64;
                // Set the range of all items that exist in the list.
                list.set_item_range(cx, 0, last_item_id);
            }
            */
            return
        }

        let message_list = self.ui.portal_list_set(ids!(message_list));

        if let Event::Draw(event) = event {
            let cx = &mut Cx2d::new(cx, event);
            while let Some(next) = self.ui.draw_widget(cx).hook_widget() {
                if let Some(mut list) = message_list.has_widget(&next).borrow_mut() {

                    let last_item_id = MESSAGES.len() as u64 + 1; // + 1 because we use 0 for the TopSpace
                    // Set the range of all items that exist in the list.
                    // + 1 again because we use the last item for the BottomSpace.
                    list.set_item_range(cx, 0, last_item_id + 1);
                    
                    println!("-------- Starting next visible item loop --------");
                    while let Some(item_id) = list.next_visible_item(cx) {
                        println!("Drawing item {}", item_id);
                        let item = if item_id == 0 {
                            let template = live_id!(TopSpace);
                            list.item(cx, item_id, template).unwrap()
                        } else if item_id >= last_item_id {
                            let template = live_id!(BottomSpace);
                            list.item(cx, item_id, template).unwrap()
                        } else {
                            let template = live_id!(Post);
                            let item = list.item(cx, item_id, template).unwrap();
                            if let Some((msg_id, un, msg)) = MESSAGES.get((item_id - 1) as usize) {
                                item.label(id!(content.username)).set_text(un);
                                item.label(id!(profile.timestamp)).set_text(&format!("id: {msg_id}"));
                                item.label(id!(content.message)).set_text(msg);
                                item.button(id!(likes)).set_text(&format!("{msg_id}"));
                                item.button(id!(comments)).set_text(&format!("{msg_id}"));
                            } else {
                                println!("\tSkipping setting content for item_id {item_id}");
                            }
                            item
                        };

                        item.draw_widget_all(cx);
                    }
                }
            }
            return
        }
        
        let actions = self.ui.handle_widget_event(cx, event);
        
        for (item_id, item) in message_list.items_with_actions(&actions) {
            if item.button(id!(likes)).clicked(&actions) {
                log!("hello {}", item_id);
            }
        }
    }
}
use chrono::NaiveDateTime;
use makepad_widgets::*;
use matrix_sdk::ruma::MilliSecondsSinceUnixEpoch;
use matrix_sdk_ui::timeline::{TimelineItemKind, VirtualTimelineItem, TimelineDetails, TimelineItemContent, AnyOtherFullStateEventContent};

use crate::sliding_sync::CHOSEN_ROOM;
   
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
    
    MessageMenu = <View> {
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
    
    Message = <View> {
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
                datestamp = <Timestamp> {
                    padding: { top: 5.0 }
                }
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
                
                <MessageMenu> {}
            }
        }
        
        // <LineH> {
        //     draw_bg: {color: (COLOR_DIVIDER_DARK)}
        // }
    }


    DayDivider = <View> {
        width: Fill,
        height: Fit,
        margin: 0.0,
        flow: Right,
        padding: 0.0,
        spacing: 0.0,
        align: {x: 0.5, y: 0.5} // center horizontally and vertically


        left_line = <LineH> {
            margin: {top: 10.0, bottom: 10.0}
            draw_bg: {color: (COLOR_DIVIDER_DARK)}
        }

        date = <Label> {
            padding: {left: 8.0, right: 8.0}
            margin: {bottom: 10.0, top: 10.0}
            draw_text: {
                text_style: <TEXT_SUB> {},
                color: (COLOR_DIVIDER_DARK)
            }
            text: "<date>"
        }

        right_line = <LineH> {
            margin: {top: 10.0, bottom: 10.0}
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

                    // Below, we must place all of the possible views that can be used in the portal list.
                    TopSpace = <View> {height: 80}
                    Message = <Message> {}
                    DayDivider = <DayDivider> {}
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


impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if matches!(event, Event::Construct) {
            println!("Construct: starting matrix sdk loop");
            // crate::matrix::start_matrix_tokio().unwrap();
            crate::sliding_sync::start_matrix_tokio().unwrap();

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

                    let timeline_items_owned = if let Some(r) = CHOSEN_ROOM.get() {
                        crate::sliding_sync::get_timeline_items(r)
                    } else {
                        None
                    };
                    let timeline_items = timeline_items_owned.as_ref();
                    let last_item_id = timeline_items
                        .map(|(_tl, items)| items.len() as u64)
                        .unwrap_or(0);
                    let last_item_id = last_item_id + 1; // Add 1 for the TopSpace.

                    // Set the range of all items that exist in the list.
                    // + 1 again because we use the last item for the BottomSpace.
                    list.set_item_range(cx, 0, last_item_id + 1);
                    
                    // println!("-------- Starting next visible item loop, last_item: {last_item_id} --------");
                    while let Some(item_id) = list.next_visible_item(cx) {
                        // println!("Drawing item {}", item_id);
                        let item = if item_id == 0 {
                            list.item(cx, item_id, live_id!(TopSpace)).unwrap()
                        } else if item_id >= last_item_id {
                            list.item(cx, item_id, live_id!(BottomSpace)).unwrap()
                        } else {
                            let tl_idx = (item_id - 1) as usize;
                            if let Some(timeline_item) = timeline_items.and_then(|(_tl, items)| items.get(tl_idx)) {
                                match timeline_item.kind() {
                                    TimelineItemKind::Event(tl_event) => {
                                        let item = list.item(cx, item_id, live_id!(Message)).unwrap();

                                        // Set sender to the display name if available, otherwise the user id.
                                        let sender = match tl_event.sender_profile() {
                                            TimelineDetails::Ready(profile) => profile.display_name.as_deref(),
                                            _ => None,
                                        }.unwrap_or_else(|| tl_event.sender().as_str());
                                        item.label(id!(content.username)).set_text(sender);

                                        // Set the timestamp.
                                        let ts_millis = tl_event.timestamp();
                                        if let Some(dt) = unix_time_millis_to_datetime(&ts_millis) {
                                            // format as AM/PM 12-hour time
                                            item.label(id!(profile.timestamp)).set_text(
                                                &format!("{}", dt.time().format("%-I:%M %p"))
                                            );
                                            item.label(id!(profile.datestamp)).set_text(
                                                &format!("{}", dt.date())
                                            );
                                        } else {
                                            item.label(id!(profile.timestamp)).set_text(
                                                &format!("{}", ts_millis.get())
                                            );
                                        }

                                        // Set the content.
                                        match tl_event.content() {
                                            TimelineItemContent::Message(message) => {
                                                item.label(id!(content.message)).set_text(message.body());
                                            }
                                            TimelineItemContent::OtherState(other) => {
                                                let text = match other.content() {
                                                    AnyOtherFullStateEventContent::PolicyRuleRoom(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::PolicyRuleServer(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::PolicyRuleUser(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomAliases(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomAvatar(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomCanonicalAlias(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomCreate(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomEncryption(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomGuestAccess(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomHistoryVisibility(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomJoinRules(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomName(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomPinnedEvents(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomPowerLevels(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomServerAcl(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomThirdPartyInvite(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomTombstone(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::RoomTopic(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::SpaceChild(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::SpaceParent(fs_content) => format!("{:?}", fs_content),
                                                    AnyOtherFullStateEventContent::_Custom { event_type } => format!("{:?}", event_type),
                                                };
                                                item.label(id!(content.message)).set_text(&text);
                                            }
                                            other => {
                                                item.label(id!(content.message)).set_text(&format!("{:?}", other));
                                            }
                                        }

                                        // Temp filler: set the likes and comments count to the item id, just for now.
                                        item.button(id!(likes)).set_text(&format!("{item_id}"));
                                        item.button(id!(comments)).set_text(&format!("{item_id}"));
                                        item
                                    }
                                    TimelineItemKind::Virtual(VirtualTimelineItem::DayDivider(millis)) => {
                                        let item = list.item(cx, item_id, live_id!(DayDivider)).unwrap();
                                        let text = unix_time_millis_to_datetime(millis)
                                            // format the time as a shortened date (Sat, Sept 5, 2021)
                                            .map(|dt| format!("{}", dt.date().format("%a %b %-d, %Y")))
                                            .unwrap_or_else(|| format!("{:?}", millis));
                                        item.label(id!(date)).set_text(&text);
                                        item
                                    }
                                    TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker) => {
                                        // reuse the DayDivider view for user read markers.
                                        let item = list.item(cx, item_id, live_id!(DayDivider)).unwrap();
                                        item.label(id!(date)).set_text(&format!("Read marker, {}", timeline_item.unique_id()));
                                        item
                                    }
                                }
                            } else {
                                println!("\tSkipping setting content for item_id {item_id}");
                                // This should never happen, so just use a blank Message.
                                list.item(cx, item_id, live_id!(Message)).unwrap()
                            }
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


fn unix_time_millis_to_datetime(millis: &MilliSecondsSinceUnixEpoch) -> Option<NaiveDateTime> {
    let millis: i64 = millis.get().into();
    NaiveDateTime::from_timestamp_millis(millis)
}
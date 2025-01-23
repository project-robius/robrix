use imbl::HashMap;
use makepad_widgets::*;
use matrix_sdk::ruma::OwnedUserId;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;
    use crate::shared::search_bar::SearchBar;

    FadeView = <CachedView> {
        draw_bg: {
            instance opacity: 1.0

            fn pixel(self) -> vec4 {
                let color = sample2d_rt(self.image, self.pos * self.scale + self.shift) + vec4(self.marked, 0.0, 0.0, 0.0);
                return Pal::premul(vec4(color.xyz, color.w * self.opacity))
            }
        }
    }

    RoomMemberPreview = <View> {
        width: Fill,
        height: 40,

        left = <View> {
            avatar = <Avatar> {}
            member_name =  <Label> {
                text: "Tyrese Luo",
            }
        }

        <Filler> {}

        power_level = <Label> {
            text: "Admin",
        }
    }

    pub RoomMembersList = {{RoomMembersList}}<ScrollXYView> {
        width: Fill,
        height: Fill,
        spacing: 20,
        flow: Down,
        margin: { top: 10 }

        list = <PortalList> {
            keep_invisible: false
            auto_tail: false
            width: Fill, height: Fill
            flow: Down, spacing: 0.0

            room_preview = <RoomMemberPreview> {}
            empty = <Empty> {}
        }
    }

    pub RoomMembersSlidingPane = {{RoomMembersSlidingPane}} {
        flow: Overlay,
        width: Fill,
        height: Fill,
        align: {x: 1.0, y: 0}

        bg_view = <View> {
            width: Fill
            height: Fill
            // visible: false,
            show_bg: true
            draw_bg: {
                fn pixel(self) -> vec4 {
                    return vec4(0., 0., 0., 0.7)
                }
            }
        }

        main_content = <FadeView> {
            width: 360,
            height: Fill
            flow: Overlay,
            room_members_view = <View> {
                width: Fill,
                height: Fill
                flow: Down,
                show_bg: true,
                draw_bg: {
                    color: (COLOR_PRIMARY)
                }
                padding: {left: 10, right: 10, top: 10, bottom: 10}

                top = <View> {
                    width: Fill,
                    height: Fit,
                    align: { y: 0.5 },
                    title = <Label> {
                        text: "Room Members",
                        draw_text: {
                            color: #000,
                            text_style: {
                                font_size: 10.5,
                            },
                        }
                    }
                    <Filler> {}
                    // The "X" close button on the top left
                    close_button = <RobrixIconButton> {
                        width: Fit,
                        height: Fit,
                        align: {x: 0.0, y: 0.0},
                        draw_icon: {
                            svg_file: (ICON_CLOSE),
                            fn get_color(self) -> vec4 {
                                return #x0;
                            }
                        }
                        icon_walk: {width: 10, height: 10}
                    }
                }

                search_members = <SearchBar> {
                    margin: { top: 10 },
                    input = {
                        empty_message: "Search room members...",
                    }
                    draw_bg: {
                        radius: 2.0,
                        border_color: #000,
                    }
                }

                members_list = <RoomMembersList> {}
            }
        }

        animator: {
            panel = {
                default: hide,
                show = {
                    redraw: true,
                    from: {all: Forward {duration: 0.4}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {main_content = { width: 300, draw_bg: {opacity: 1.0} }}
                }
                hide = {
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {main_content = { width: 0, draw_bg: {opacity: 0.0} }}
                }
            }
        }
    }

}

#[derive(Live, LiveHook, Widget)]
pub struct RoomMembersSlidingPane {
    #[deref] view: View,
    #[animator] animator: Animator,
}

impl Widget for RoomMembersSlidingPane {

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}


#[derive(Debug)]
pub struct RoomMemberEntry {
    pub user_id: OwnedUserId,
    pub display_name: Option<String>,
    pub avatar: RoomMemberPreviewAvatar,
    pub power_level: i64,
}

#[derive(Debug)]
pub enum RoomMemberPreviewAvatar {
    Text(String),
    Image(Vec<u8>),
}

impl Default for RoomMemberPreviewAvatar {
    fn default() -> Self {
        RoomMemberPreviewAvatar::Text(String::new())
    }
}


#[derive(Live, LiveHook, Widget)]
pub struct RoomMembersList {
    #[deref] view: View,

    #[rust] all_members: HashMap<OwnedUserId, >
}

impl Widget for RoomMembersList {

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
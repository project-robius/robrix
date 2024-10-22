use crossbeam_queue::SegQueue;
use makepad_widgets::*;

static POPUP_UPDATES: SegQueue<PopupUpdate> = SegQueue::new();

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::view::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::adaptive_view::AdaptiveView;
    ICON_SETTINGS = dep("crate://self/resources/icons/settings.svg")
    Popup = {{Popup}} {
        width: Fit,
        height: Fit,

        notification = <PopupNotification> {
            margin: {top: 25,right: 0},
            content: {
                height: Fit,
                width: Fit,

                padding: 10,

                <RoundedView> {
                    height: Fit,
                    width: 240,

                    padding: 30,
                    show_bg: true,
                    draw_bg: {
                        color: #FFFFFF
                        instance border_width: 0.8
                        instance border_color: #D0D5DD
                        radius: 3.0
                    }
                    room_status_label = <Label> {
                        width: 170
                        text: "......"
                        draw_text: {
                            color: #000
                        }
                    }
                    close_popup_button = <Button> {
                        width: Fit,
                        height: Fit,

                        margin: {top: -20 },

                        draw_icon: {
                            svg_file: dep("crate://self/resources/close.svg"),
                            fn get_color(self) -> vec4 {
                                return #000;
                            }
                        }
                        icon_walk: {width: 10, height: 10}
                    }
                }
            }
        }

    }


}
pub enum PopupUpdate {
    RoomListStatus { status: String },
}

pub fn enqueue_popup_update(update: PopupUpdate) {
    POPUP_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}
#[derive(Live, LiveHook, Widget)]
pub struct Popup {
    #[deref]
    view: View,
}
impl Widget for Popup {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        while let Some(update) = POPUP_UPDATES.pop() {
            match update {
                PopupUpdate::RoomListStatus { status } => {
                    self.view.label(id!(room_status_label)).set_text(&status);
                    self.view.popup_notification(id!(notification)).open(cx);
                }
            }
        }
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for Popup {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(id!(close_popup_button)).clicked(actions) {
            self.view.popup_notification(id!(notification)).close(cx);
        }
    }
}

use std::time::Instant;

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    Progress = <View> {
        width: 20,
        height: Fill,
        flow: Overlay,

        <RoundedView> {
            width: Fill,
            height: Fill,
            draw_bg: {
                color: #639b0d,
                radius: 4.0,
            }
        }

        progress_bar = <RoundedView> {
            height: Fill,
            width: Fill,
            draw_bg: {
                color: #42660a,
                radius: 4.0,
            }
        }
    }

    PopupDialog = <RoundedView> {
        width: 275,
        height: Fill,

        <Progress> {}
    }


    pub RobrixPopupNotification = {{RobrixPopupNotification}} {
        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(193,255,193,1.0);
            }
        }
        content: {
            <PopupDialog> {}
        }
        
        animator: {
            mode = {
                default: close,
                open = {
                    redraw: true,
                    from: {}
                    ease: 
                    apply: {}
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct RobrixPopupNotification {
    #[deref]
    view: PopupNotification,

    #[live]
    duration: f64,

    #[rust]
    timer: Timer,

    #[rust]
    start_time: Option<Instant>,

    #[rust]
    opened: bool,

    #[animator]
    animator: Animator,
}

impl Widget for RobrixPopupNotification {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {

        // loop alaways trigger 
        if self.opened {
            // self.timer = cx.start_timeout(interval);
            self.start_time = Some(Instant::now());
        }

        if self.timer.is_event(event).is_some() {
            self.view.close(cx);
        }
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if let Some(start_time) = self.start_time {
            let elapsed = start_time.elapsed().as_secs_f64();
            let progress = (elapsed / self.duration).min(1.0);
            let progress_bar_height = 100.0 * (1.0 - progress);

            self.view.view(id!(progress_bar)).apply_over(
                cx,
                live! {
                    height: (progress_bar_height)
                } 
            );
            if elapsed >= self.duration {
                self.view.close(cx);
                return DrawStep::done();
            }
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for RobrixPopupNotification {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        
    }
}

impl RobrixPopupNotification {
    // pub fn set_time(&mut self, duration: f64) {
    //     // self.timer = cx.start_timeout(duration);
    //     self.duration = duration;
    // }
}


impl RobrixPopupNotificationRef {
    // pub fn set_time(&mut self, duration: f64) {
    //     if let Some(mut inner) = self.borrow_mut() {
    //         inner.set_time(duration);
    //     }
    // }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum PopupNotificationAction {
    None,
    Close,
    Open
}
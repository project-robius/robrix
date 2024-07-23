use makepad_widgets::*;

use super::portal::PortalAction;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::portal::*;

    Modal = {{Modal}} {
        width: Fill
        height: Fill
        flow: Overlay
        align: {x: 0.5, y: 0.5}

        bg_view = <View> {
            width: Fill
            height: Fill
            show_bg: true
            draw_bg: {
                fn pixel(self) -> vec4 {
                    return vec4(0., 0., 0., 0.7)
                }
            }
        }

        content = <View> {
            flow: Overlay
            width: Fit
            height: Fit
        }
    }
}

#[derive(Live, LiveHook, LiveRegisterWidget, WidgetRef)]
pub struct Modal {
    #[deref]
    view: View,
}

impl Widget for Modal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let widget_uid = self.widget_uid();

        self.view(id!(content)).handle_event(cx, event, scope);

        let content_rect = self.view(id!(content)).area().rect(cx);

        // Check if there was a click outside of the content (bg), then close if true.
        if let Hit::FingerUp(fe) = event.hits_with_capture_overload(cx, self.view.area(), true) {
            if !content_rect.contains(fe.abs) {
                cx.widget_action(widget_uid, &scope.path, PortalAction::Close);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view
            .draw_walk(cx, scope, walk.with_abs_pos(DVec2 { x: 0., y: 0. }))
    }
}

impl WidgetNode for Modal {
    fn walk(&mut self, cx: &mut Cx) -> Walk {
        self.view.walk(cx)
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.view.redraw(cx);
    }

    fn find_widgets(&self, path: &[LiveId], cached: WidgetCache, results: &mut WidgetSet) {
        self.view.find_widgets(path, cached, results);
    }
}

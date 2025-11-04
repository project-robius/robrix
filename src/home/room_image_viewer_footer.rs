//! Room image viewer footer widget that contains loading spinner, error icon, and status label.

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    pub RoomImageViewerFooter = {{RoomImageViewerFooter}} {
        width: Fill, height: 50,
        flow: Right
        padding: 10
        align: {x: 0.5, y: 0.8}
        spacing: 10

        image_viewer_loading_spinner_view = <View> {
            width: Fit, height: Fit
            loading_spinner = <LoadingSpinner> {
                width: 40, height: 40,
                draw_bg: {
                    color: (COLOR_PRIMARY)
                    border_size: 3.0,
                }
            }
        }
        image_viewer_forbidden_view = <View> {
            width: Fit, height: Fit
            visible: false
            <Icon> {
                draw_icon: {
                    svg_file: (ICON_FORBIDDEN),
                    color: #ffffff,
                }
                icon_walk: { width: 30, height: 30 }
            }
        }
        image_viewer_status_label = <Label> {
            width: Fit, height: 30,
            text: "Loading image...",
            draw_text: {
                text_style: <REGULAR_TEXT>{font_size: 14},
                color: (COLOR_PRIMARY)
            }
        }
    }
}

/// A room image viewer footer widget that contains loading spinner, error icon, and status label.
#[derive(Live, LiveHook, Widget)]
pub struct RoomImageViewerFooter {
    #[deref]
    view: View,
}

impl RoomImageViewerFooter {
    /// Shows a loading message in the footer.
    ///
    /// The loading spinner is shown, the error icon is hidden, and the
    /// status label is set to "Loading...".
    pub fn show_loading(&mut self, cx: &mut Cx) {
        self.view.view(ids!(image_viewer_loading_spinner_view)).set_visible(cx, true);
        self.view.label(ids!(image_viewer_status_label)).set_text(cx, "Loading...");
        self.view.view(ids!(image_viewer_forbidden_view)).set_visible(cx, false);
        self.view.view(ids!(footer)).apply_over(cx, live!{
            height: 50
        });
    }

    /// Shows an error message in the footer.
    ///
    /// The loading spinner is hidden, the error icon is shown, and the
    /// status label is set to the error message provided.
    pub fn show_error(&mut self, cx: &mut Cx, error: &str) {
        self.view.view(ids!(image_viewer_loading_spinner_view)).set_visible(cx, false);
        self.view.view(ids!(image_viewer_forbidden_view)).set_visible(cx, true);
        self.view.label(ids!(image_viewer_status_label)).set_text(cx, error);
        
    }

    /// Hides all the elements in the footer.
    pub fn hide(&mut self, cx: &mut Cx) {
        self.view.view(ids!(image_viewer_loading_spinner_view)).set_visible(cx, false);
        self.view.view(ids!(image_viewer_forbidden_view)).set_visible(cx, false);
        self.view.label(ids!(image_viewer_status_label)).set_text(cx, "");
    }
}

impl Widget for RoomImageViewerFooter {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomImageViewerFooterRef {

    /// See [`RoomImageViewerFooter::show_loading()`].
    pub fn show_loading(&mut self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_loading(cx);
        }
    }

    /// See [`RoomImageViewerFooter::show_error()`].
    pub fn show_error(&mut self, cx: &mut Cx, error: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_error(cx, error);
        }
    }

    /// See [`RoomImageViewerFooter::hide()`].
    pub fn hide(&mut self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide(cx);
        }
    }
}
use std::sync::Arc;
use crate::utils;

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub ImageViewer = {{ImageViewer}} {
        visible: false
        width: Fill, height: Fill
        align: {x: 0.5, y: 0.5}
        spacing: 12
        flow: Overlay
        show_bg: true
        draw_bg: {
            color: (COLOR_IMAGE_VIEWER_BG)
        }

        <View> {
            align: {x: 1.0, y: 0.0}
            width: Fill, height: Fill
            close_button = <RobrixIconButton> {
                padding: {left: 15, right: 15}
                draw_icon: {
                    svg_file: (ICON_CLOSE)
                    color: (COLOR_CLOSE),
                }
                icon_walk: {width: 25, height: 25, margin: {left: -1, right: -1} }

                draw_bg: {
                    border_color: (COLOR_CLOSE_BG),
                    color: (COLOR_CLOSE_BG) // light red
                }
            }
        }

        image_view = <View> {
            padding: {top: 40, bottom: 30, left: 20, right: 20}
            flow: Overlay
            align: {x: 0.5, y: 0.5}
            width: Fill, height: Fill,
            image = <Image> {
                width: Fill, height: Fill,
                fit: Smallest,
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ImageViewer {
    #[deref]
    view: View,
}

#[derive(Clone, Debug, DefaultNone)]
pub enum ImageViewerAction {
    // We need to distinct these two similar actions because a potential bug.
    // if we combine these 2 actions into 1, will appear a rare bug-condition:
    //
    // User click some image and **quickly close** the image viewer,
    // firstly, the `_ & File` version image has not been fetched, `try_get_image_or_fetch` would return the matching thumbnail to as temporariness,
    // Once the `_ & File` version image is beening fetched, it would autoly open image viewer and show the image,
    // but image viewer was already closed by user.
    // That is, image viewer will show up for no reason in users' view.
    // feel free to modify comments.
    Show(Arc<[u8]>),
    MakeItClearer(Arc<[u8]>),
    None,
}

impl Widget for ImageViewer {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
impl MatchEvent for ImageViewer {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.view.button(id!(close_button)).clicked(actions) {
            self.close(cx);
        }

        for action in actions {
            if let Some(ImageViewerAction::Show(data)) = action.downcast_ref() {
                self.open(cx);
                self.load_with_data(cx, data);
            }
            if let Some(ImageViewerAction::MakeItClearer(data)) = action.downcast_ref() {
                // Handled the potential bug here
                // feel free to modify comments.
                if self.visible {
                    self.load_with_data(cx, data);
                }
            }
        }
    }
}

impl ImageViewer {
    fn open(&mut self, cx: &mut Cx) {
        self.visible = true;
        self.redraw(cx);
    }
    fn close(&mut self, cx: &mut Cx) {
        self.visible = false;
        self.view.image(id!(image_view.image)).set_texture(cx, None);
        self.redraw(cx);
    }
    fn load_with_data(&mut self, cx: &mut Cx, data: &[u8]) {
        let image = self.view.image(id!(image_view.image));

        if let Err(e) = utils::load_png_or_jpg(&image, cx, data) {
            log!("Error to load image: {e}");
        } else {
            self.view.redraw(cx);
        }
    }
}

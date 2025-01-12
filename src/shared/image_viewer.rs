use makepad_widgets::*;

use matrix_sdk::ruma::events::room::{ImageInfo, MediaSource};

use crate::{media_cache::{MediaCache, MediaCacheEntry}, utils};

#[derive(Clone, DefaultNone, Debug)]
pub enum ImageViewerAction {
    Open,
    None
}

live_design! {
    use link::theme::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;

    pub ImageViewer = {{ImageViewer}} {
        width: Fit, height: Fit
        visible: false

        flow: Overlay

        close_button = <RobrixIconButton> {
            align: {x: 1., y: 0.}
            enabled: false,
            padding: {top: 0, right: 0}
            draw_icon: {
                svg_file: (ICON_CLOSE)
                color: (COLOR_ACCEPT_GREEN),
            }
            icon_walk: {width: 16, height: 16, margin: {left: -1, right: -1} }

            draw_bg: {
                border_color: (COLOR_ACCEPT_GREEN),
                color: #f0fff0 // light green
            }
        }
        image = <Image> {
            fit: Stretch,
            width: Fill, height: Fill,
            fit: Largest,
            // source: (IMG_DEFAULT_AVATAR),
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ImageViewer {
    #[deref] view: View,
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
        if self.button(id!(close_button)).clicked(actions) {
            log!("")
        }

        for action in actions {
            match action.downcast_ref() {
                Some(ImageViewerAction::Open) => {
                    self.visible = true;
                    self.redraw(cx);
                },
                Some(ImageViewerAction::None) | None => {
                }
            }
        }
    }
}

impl ImageViewer {
    /// We fetch thumbnail of the image in `populate_image_message_content` in `room_screen.rs`.
    ///
    /// We fetch origin of the image and show it here.
    pub fn fetch_and_show_image<F, E> (
        &mut self,
        cx: &mut Cx,
        image_info_source: Option<(Option<ImageInfo>, MediaSource)>,
        media_cache: &mut MediaCache
    )
    {
        let image_ref = self.view.image(id!(image));

        match image_info_source.map(|(_, media_source)| media_source ) {
                Some(MediaSource::Plain(mxc_uri)) => {
                    // Now that we've obtained thumbnail of the image URI and its metadata.
                    // Let's try to fetch it.
                    match media_cache.try_get_media_or_fetch(mxc_uri.clone(), None) {
                        MediaCacheEntry::Loaded(data) => {
                            let load_image_task = utils::load_png_or_jpg(&image_ref, cx, &data)
                                            .map(|()| image_ref.size_in_pixels(cx).unwrap_or_default());

                            if let Err(e) = load_image_task {
                                log!("Image loading error: {e}")
                            }

                        }
                        MediaCacheEntry::Requested | MediaCacheEntry::Failed=> {
                            // TODO: Show loading spinner.
                        }
                    }
                }
                Some(MediaSource::Encrypted(_encrypted)) => {
                }
                None => {
                }
        }
    }
}
